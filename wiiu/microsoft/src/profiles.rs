use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use horizon_wiiu_security::{Protection, Secret, SecretStore, StorageMode, Vault};
use serde::{Deserialize, Serialize};

use crate::protocol::valid_player;
use crate::{Error, Identity, ProfileId, Provider, Verified, Work};

const INDEX: &str = "profiles";
const REFRESH: &str = "refresh_token";
const MAX_PROFILES: usize = 48;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(with = "uuid")]
    pub id: ProfileId,
    pub name: String,
}

mod uuid {
    use super::*;

    pub fn serialize<S: serde::Serializer>(id: &ProfileId, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{:032x}", id.0))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ProfileId, D::Error> {
        ProfileId::parse(&String::deserialize(d)?)
            .ok_or_else(|| serde::de::Error::custom("invalid profile UUID"))
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicIndex {
    profiles: Vec<Profile>,
    selected: Option<Profile>,
}

impl PublicIndex {
    fn valid(&self) -> bool {
        self.profiles.len() <= MAX_PROFILES
            && self.profiles.iter().enumerate().all(|(i, profile)| {
                valid_player(&profile.name)
                    && self.profiles[..i]
                        .iter()
                        .all(|other| other.id != profile.id)
            })
            && self
                .selected
                .as_ref()
                .is_none_or(|selected| self.profiles.iter().any(|entry| entry == selected))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileState {
    Locked,
    Saved(StorageMode),
    SignedIn(StorageMode),
    Reauthenticate,
    Unavailable,
}

pub struct Accounts {
    root: PathBuf,
    index: Vault,
    public: PublicIndex,
    vaults: BTreeMap<ProfileId, Result<Vault, Error>>,
    session_profiles: BTreeMap<ProfileId, Profile>,
    identities: BTreeMap<ProfileId, Identity>,
    failures: BTreeMap<ProfileId, Error>,
    selected: Option<ProfileId>,
}

impl Accounts {
    /// The index stores only public profile information. It lives outside
    /// launcher settings and each refresh credential's independent domain.
    pub fn open(private_root: &Path) -> Result<Self, Error> {
        let mut index = Vault::open(private_root, "microsoft_index")?;
        let public: PublicIndex = if index.status().initialized {
            if index.status().mode != StorageMode::Unprotected {
                return Err(Error::Storage);
            }
            match index.get(INDEX)? {
                Some(data) => serde_json::from_slice(data.expose()).map_err(|_| Error::Storage)?,
                None => PublicIndex::default(),
            }
        } else {
            PublicIndex::default()
        };
        if !public.valid() {
            return Err(Error::Storage);
        }
        let selected = public.selected.as_ref().map(|profile| profile.id);
        let vaults = public
            .profiles
            .iter()
            .map(|profile| (profile.id, open_vault(private_root, profile.id)))
            .collect();
        Ok(Self {
            index,
            root: private_root.to_owned(),
            public,
            vaults,
            session_profiles: BTreeMap::new(),
            identities: BTreeMap::new(),
            failures: BTreeMap::new(),
            selected,
        })
    }

    fn vault(&mut self, id: ProfileId) -> Result<&mut Vault, Error> {
        self.vaults
            .entry(id)
            .or_insert_with(|| open_vault(&self.root, id))
            .as_mut()
            .map_err(|error| *error)
    }

    fn save_index(&mut self, next: PublicIndex) -> Result<(), Error> {
        if !next.valid() {
            return Err(Error::Storage);
        }
        if !self.index.status().initialized {
            self.index
                .set_protection(Protection::ConfirmedUnprotected)?;
        }
        let bytes = serde_json::to_vec(&next).map_err(|_| Error::Storage)?;
        self.index.put(INDEX, Secret::new(bytes))?;
        self.public = next;
        Ok(())
    }

    pub fn profiles(&self) -> impl Iterator<Item = &Profile> {
        self.public
            .profiles
            .iter()
            .chain(self.session_profiles.values())
    }

    pub fn profile(&self, id: ProfileId) -> Option<&Profile> {
        self.profiles().find(|profile| profile.id == id)
    }

    pub fn selected(&self) -> Option<ProfileId> {
        self.selected
    }

    pub fn active(&self) -> Option<&Identity> {
        self.selected
            .and_then(|id| self.identities.get(&id))
            .filter(|identity| identity.is_current())
    }

    pub fn mode(&self, id: ProfileId) -> Option<StorageMode> {
        self.vaults
            .get(&id)
            .and_then(|result| result.as_ref().ok())
            .filter(|vault| vault.status().initialized)
            .map(|vault| vault.status().mode)
    }

    pub fn state(&self, id: ProfileId) -> ProfileState {
        if self.failures.get(&id) == Some(&Error::Reauthenticate) {
            return ProfileState::Reauthenticate;
        }
        match self.vaults.get(&id) {
            Some(Ok(vault)) => {
                let state = vault.status();
                if state.locked {
                    ProfileState::Locked
                } else if self.identities.get(&id).is_some_and(Identity::is_current) {
                    ProfileState::SignedIn(state.mode)
                } else if state.initialized
                    && (state.mode != StorageMode::SessionOnly
                        || self.session_profiles.contains_key(&id))
                {
                    ProfileState::Saved(state.mode)
                } else {
                    ProfileState::Reauthenticate
                }
            }
            _ => ProfileState::Unavailable,
        }
    }

    pub fn unlock(&mut self, id: ProfileId, pin: &[u8]) -> Result<(), Error> {
        Ok(self.vault(id)?.unlock(pin)?)
    }

    pub fn change_protection(
        &mut self,
        id: ProfileId,
        protection: Protection<'_>,
    ) -> Result<bool, Error> {
        // The local edit UI offers PIN/no PIN on saved profiles. Session-only
        // profiles can be persisted when the player signs in again.
        if matches!(protection, Protection::SessionOnly)
            || self.mode(id) == Some(StorageMode::SessionOnly)
        {
            return Err(Error::Storage);
        }
        let vault = self.vault(id)?;
        vault.set_protection(protection)?;
        Ok(vault.status().cleanup_pending)
    }

    pub fn refresh(&mut self, id: ProfileId, provider: &Provider) -> Result<Work, Error> {
        let refresh = self.vault(id)?.get(REFRESH)?.ok_or(Error::Reauthenticate)?;
        provider.renew(refresh)
    }

    pub fn select_current(&mut self, id: ProfileId) -> Result<bool, Error> {
        if !self.identities.get(&id).is_some_and(Identity::is_current) {
            return Ok(false);
        }
        self.select(id)?;
        Ok(true)
    }

    fn select(&mut self, id: ProfileId) -> Result<(), Error> {
        let persistent = self.public.profiles.iter().find(|profile| profile.id == id);
        if self.public.selected.as_ref() != persistent {
            let mut next = self.public.clone();
            next.selected = persistent.cloned();
            self.save_index(next)?;
        }
        self.selected = Some(id);
        Ok(())
    }

    pub fn select_offline(&mut self) -> Result<(), Error> {
        if self.public.selected.is_some() {
            let mut next = self.public.clone();
            next.selected = None;
            self.save_index(next)?;
        }
        self.selected = None;
        Ok(())
    }

    pub fn accept(
        &mut self,
        verified: Verified,
        expected: Option<ProfileId>,
        initial_protection: Option<Protection<'_>>,
    ) -> Result<(), Error> {
        let identity = verified.0.identity;
        let id = identity.id;
        if expected.is_some_and(|expected| expected != id) {
            return Err(Error::IdentityChanged);
        }
        let profile = Profile {
            id,
            name: identity.name.clone(),
        };
        if self.profile(id).is_none() && self.profiles().count() >= MAX_PROFILES {
            return Err(Error::Storage);
        }
        let vault = self.vault(id)?;
        if let Some(protection) = initial_protection {
            vault.set_protection(protection)?;
        }
        let status = vault.status();
        let persistent = status.mode != StorageMode::SessionOnly;
        if !status.initialized || status.locked {
            return Err(horizon_wiiu_security::VaultError::Locked.into());
        }

        // Publish the public profile before the credential. If power fails
        // between writes the visible profile can ask the player to sign in
        // again, and the app never creates an undiscoverable credential.
        if persistent && self.profile(id) != Some(&profile) {
            let mut next = self.public.clone();
            next.profiles.retain(|old| old.id != id);
            next.profiles.push(profile.clone());
            if self.selected == Some(id) && next.selected.is_some() {
                next.selected = Some(profile.clone());
            }
            self.save_index(next)?;
        }
        self.vault(id)?.put(REFRESH, verified.0.refresh)?;
        if persistent {
            self.session_profiles.remove(&id);
        } else {
            self.session_profiles.insert(id, profile);
        }
        self.identities.insert(id, identity);
        self.failures.remove(&id);
        self.select(id)
    }

    pub fn record_failure(&mut self, id: ProfileId, error: Error) {
        if matches!(
            error,
            Error::Reauthenticate | Error::OwnershipMissing | Error::ProfileMissing
        ) {
            self.identities.remove(&id);
            self.failures.insert(id, error);
        }
    }

    /// Drop the in-memory session, replace the saved credential with an
    /// empty generation, and only then retire its public profile entry.
    pub fn sign_out(&mut self, id: ProfileId) -> Result<(), Error> {
        self.identities.remove(&id);
        self.failures.remove(&id);
        let vault = self.vault(id)?;
        vault.erase_domain()?;
        if vault.status().cleanup_pending {
            return Err(Error::Storage);
        }
        if self.public.profiles.iter().any(|entry| entry.id == id) {
            let mut next = self.public.clone();
            next.profiles.retain(|entry| entry.id != id);
            if next.selected.as_ref().is_some_and(|entry| entry.id == id) {
                next.selected = None;
            }
            self.save_index(next)?;
        }
        self.vaults.remove(&id);
        self.session_profiles.remove(&id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        Ok(())
    }
}

fn open_vault(root: &Path, id: ProfileId) -> Result<Vault, Error> {
    Ok(Vault::open(root, &format!("microsoft/{:032x}", id.0))?)
}
