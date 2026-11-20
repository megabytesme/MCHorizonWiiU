use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::format::{self, Entries, Key, Policy, Record};
use crate::{Protection, Secret, SecretStore, StorageMode, StorageStatus, VaultError};

const SLOTS: [&str; 2] = ["vault.0", "vault.1"];
const STAGED: &str = "vault.pending";

pub struct Vault {
    domain: String,
    directory: PathBuf,
    policy: Option<Policy>,
    active: Option<(usize, u64)>,
    key: Option<Key>,
    entries: Option<Entries>,
    cleanup_pending: bool,
}

impl Vault {
    /// Open a single domain relative to the application's private directory.
    /// Components are lowercase ASCII identifiers, for example "mcp" or
    /// "microsoft/0123456789abcdef0123456789abcdef". All components also bind
    /// the protected record to the caller's domain cryptographically.
    pub fn open(private_directory: &Path, domain: &str) -> Result<Self, VaultError> {
        if domain.len() > 192 || !domain.split('/').all(format::valid_name) {
            return Err(VaultError::InvalidName);
        }
        let directory = private_directory.join(domain);
        let active = read_active(&directory, domain)?;
        let (active, policy, entries) = if let Some((slot, record)) = active {
            let policy = record.policy;
            let entries = if policy.mode == StorageMode::PinProtected {
                None
            } else {
                Some(record.decode(domain, None)?)
            };
            (Some((slot, record.generation)), Some(policy), entries)
        } else {
            (None, None, None)
        };
        let mut vault = Self {
            domain: domain.to_owned(),
            directory,
            policy,
            active,
            key: None,
            entries,
            cleanup_pending: false,
        };
        // Do not retire a protected predecessor until the selected record
        // has authenticated successfully in this process.
        if vault.status().locked {
            vault.cleanup_pending = vault.has_pending();
        } else {
            vault.retire();
        }
        Ok(vault)
    }

    fn has_pending(&self) -> bool {
        let stale = self.active.map(|(slot, _)| SLOTS[slot ^ 1]);
        [Some(STAGED), stale].into_iter().flatten().any(|name| {
            match fs::metadata(self.directory.join(name)) {
                Ok(_) => true,
                Err(err) => err.kind() != io::ErrorKind::NotFound,
            }
        })
    }

    fn retire(&mut self) {
        self.cleanup_pending = self.has_pending();
        if !self.cleanup_pending || !sync_directory(&self.directory) {
            return;
        }
        let staged = remove_if_present(&self.directory.join(STAGED));
        let stale = self
            .active
            .map(|(slot, _)| remove_if_present(&self.directory.join(SLOTS[slot ^ 1])));
        self.cleanup_pending = !sync_directory(&self.directory)
            || staged.is_err()
            || stale.is_some_and(|result| result.is_err());
    }

    fn on_disk(&self) -> Result<Option<Record>, VaultError> {
        let disk = read_active(&self.directory, &self.domain)?;
        if disk.as_ref().map(|(slot, r)| (*slot, r.generation)) != self.active
            || disk
                .as_ref()
                .is_some_and(|(_, r)| self.policy != Some(r.policy))
        {
            return Err(VaultError::StorageUnavailable);
        }
        Ok(disk.map(|(_, record)| record))
    }

    fn ready(&mut self) -> Result<(), VaultError> {
        match self.policy.ok_or(VaultError::SetupRequired)?.mode {
            StorageMode::PinProtected if self.key.is_none() => Err(VaultError::Locked),
            StorageMode::Unprotected if self.entries.is_none() => {
                let record = self.on_disk()?.ok_or(VaultError::StorageUnavailable)?;
                self.entries = Some(record.decode(&self.domain, None)?);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn commit(&mut self, policy: Policy, replacement_key: Option<&Key>) -> Result<(), VaultError> {
        // Validate the committed source before taking a step that changes it.
        if let Some(record) = self.on_disk()? {
            record.decode(&self.domain, self.key.as_ref())?;
        }
        self.write_generation(policy, replacement_key)
    }

    /// Forget every credential in this domain after explicit local consent.
    /// This also works on a locked domain. It never reads back its contents.
    /// Consumers sharing a domain should remove their individual entries instead.
    pub fn erase_domain(&mut self) -> Result<(), VaultError> {
        self.on_disk()?;
        let empty = Policy::new(StorageMode::SessionOnly)?;
        self.write_generation(empty, None)?;
        self.policy = Some(empty);
        self.lock();
        Ok(())
    }

    fn write_generation(
        &mut self,
        policy: Policy,
        replacement_key: Option<&Key>,
    ) -> Result<(), VaultError> {
        if policy.mode == StorageMode::SessionOnly && self.active.is_none() {
            return Ok(());
        }

        let (next_slot, generation) = match self.active {
            Some((slot, generation)) => (
                slot ^ 1,
                generation.checked_add(1).ok_or(VaultError::TooLarge)?,
            ),
            None => (0, 1),
        };
        let empty = Entries::new();
        let entries = if policy.mode == StorageMode::SessionOnly {
            &empty
        } else {
            self.entries.as_ref().unwrap_or(&empty)
        };
        let key = replacement_key.or(self.key.as_ref());
        let bytes = format::encode(&self.domain, generation, policy, key, entries)?;
        let dest = self.directory.join(SLOTS[next_slot]);
        let staged = self.directory.join(STAGED);

        let stage_and_commit = || -> Result<(), VaultError> {
            fs::create_dir_all(&self.directory).map_err(storage_error)?;
            remove_if_present(&staged).map_err(storage_error)?;
            // Free the inactive slot while the active generation still exists.
            // Cafe FS refuses renaming over an existing destination.
            remove_if_present(&dest).map_err(storage_error)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)
                .map_err(storage_error)?;
            file.write_all(&bytes).map_err(storage_error)?;
            file.sync_all().map_err(storage_error)?;
            drop(file);
            let staged_record =
                read_slot(&staged, &self.domain)?.ok_or(VaultError::StorageUnavailable)?;
            if staged_record.generation != generation || staged_record.policy != policy {
                return Err(VaultError::Corrupt);
            }
            let verified = staged_record.decode(&self.domain, key)?;
            if verified.len() != entries.len()
                || verified.iter().any(|(name, value)| {
                    entries.get(name).map(Secret::expose) != Some(value.expose())
                })
            {
                return Err(VaultError::Corrupt);
            }
            fs::rename(&staged, &dest).map_err(storage_error)
        };

        if let Err(err) = stage_and_commit() {
            self.retire();
            return Err(err);
        }
        self.active = Some((next_slot, generation));
        if !sync_directory(&self.directory) {
            self.cleanup_pending = true;
            return Ok(());
        }
        self.retire();
        Ok(())
    }

    fn update(&mut self, name: &str, value: Option<Secret>) -> Result<(), VaultError> {
        if !format::valid_name(name) {
            return Err(VaultError::InvalidName);
        }
        self.ready()?;
        let entries = self.entries.get_or_insert_with(Entries::new);
        let old = match value {
            Some(value) => entries.insert(name.to_owned(), value),
            None => {
                let Some(old) = entries.remove(name) else {
                    self.retire();
                    return Ok(());
                };
                Some(old)
            }
        };
        let policy = self.policy.ok_or(VaultError::SetupRequired)?;
        if policy.mode != StorageMode::SessionOnly {
            if let Err(err) = self.commit(policy, None) {
                let entries = self.entries.as_mut().unwrap();
                match old {
                    Some(value) => entries.insert(name.to_owned(), value),
                    None => entries.remove(name),
                };
                return Err(err);
            }
        }
        Ok(())
    }
}

impl SecretStore for Vault {
    fn status(&self) -> StorageStatus {
        StorageStatus {
            mode: self.policy.map_or(StorageMode::PinProtected, |p| p.mode),
            initialized: self.policy.is_some(),
            locked: self
                .policy
                .is_some_and(|p| p.mode == StorageMode::PinProtected)
                && self.key.is_none(),
            cleanup_pending: self.cleanup_pending,
        }
    }

    fn unlock(&mut self, pin: &[u8]) -> Result<(), VaultError> {
        let policy = self.policy.ok_or(VaultError::SetupRequired)?;
        if policy.mode != StorageMode::PinProtected || self.key.is_some() {
            return Ok(());
        }
        if !(6..=128).contains(&pin.len()) {
            return Err(VaultError::AuthenticationFailed);
        }
        let record = self.on_disk()?.ok_or(VaultError::StorageUnavailable)?;
        let key = policy.key(pin)?;
        let entries = record.decode(&self.domain, Some(&key))?;
        self.entries = Some(entries);
        self.key = Some(key);
        self.retire();
        Ok(())
    }

    fn lock(&mut self) {
        self.key = None;
        self.entries = None;
    }

    fn get(&mut self, name: &str) -> Result<Option<Secret>, VaultError> {
        if !format::valid_name(name) {
            return Err(VaultError::InvalidName);
        }
        self.ready()?;
        Ok(self
            .entries
            .as_ref()
            .and_then(|entries| entries.get(name))
            .map(|value| Secret::new(value.expose().to_vec())))
    }

    fn put(&mut self, name: &str, secret: Secret) -> Result<(), VaultError> {
        self.update(name, Some(secret))
    }

    fn remove(&mut self, name: &str) -> Result<(), VaultError> {
        self.update(name, None)
    }

    fn set_protection(&mut self, protection: Protection<'_>) -> Result<(), VaultError> {
        if self.policy.is_some() {
            self.ready()?;
        }
        let (policy, key) = match protection {
            Protection::PinProtected { new_pin } => {
                if !(6..=128).contains(&new_pin.len()) {
                    return Err(VaultError::InvalidPinLength);
                }
                let policy = Policy::new(StorageMode::PinProtected)?;
                (policy, Some(policy.key(new_pin)?))
            }
            Protection::ConfirmedUnprotected => (Policy::new(StorageMode::Unprotected)?, None),
            Protection::SessionOnly => (Policy::new(StorageMode::SessionOnly)?, None),
        };
        self.commit(policy, key.as_ref())?;
        self.policy = Some(policy);
        self.key = key;
        Ok(())
    }
}

fn read_active(directory: &Path, domain: &str) -> Result<Option<(usize, Record)>, VaultError> {
    let a = read_slot(&directory.join(SLOTS[0]), domain)?;
    let b = read_slot(&directory.join(SLOTS[1]), domain)?;
    match (a, b) {
        (None, None) => Ok(None),
        (Some(a), None) => Ok(Some((0, a))),
        (None, Some(b)) => Ok(Some((1, b))),
        (Some(a), Some(b)) if a.generation.checked_add(1) == Some(b.generation) => Ok(Some((1, b))),
        (Some(a), Some(b)) if b.generation.checked_add(1) == Some(a.generation) => Ok(Some((0, a))),
        _ => Err(VaultError::Corrupt),
    }
}

fn read_slot(path: &Path, domain: &str) -> Result<Option<Record>, VaultError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(storage_error(err)),
    };
    // Allocate once so plaintext read from unprotected slots is not left
    // behind in buffers freed during growth.
    let mut bytes = Zeroizing::new(Vec::with_capacity(format::MAX_FILE + 1));
    file.take((format::MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(storage_error)?;
    Record::parse(bytes, domain).map(Some)
}

fn storage_error(_: io::Error) -> VaultError {
    VaultError::StorageUnavailable
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> bool {
    File::open(path).and_then(|file| file.sync_all()).is_ok()
}

// Cafe provides FSFlushFile for file data. It has no directory descriptor
// for fsync. Its synchronous rename/remove calls are the metadata boundary.
#[cfg(not(unix))]
fn sync_directory(_: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests;
