use std::collections::BTreeMap;

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::{aead::AeadInPlace, KeyInit, XChaCha20Poly1305};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Secret, StorageMode, VaultError};

const MAGIC: &[u8; 8] = b"HWVAULT\0";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 70;
const DIGEST_LEN: usize = 32;
const TAG_LEN: usize = 16;
pub(crate) const MAX_PAYLOAD: usize = 128 * 1024;
pub(crate) const MAX_FILE: usize = HEADER_LEN + MAX_PAYLOAD + TAG_LEN + DIGEST_LEN;
const MAX_SECRET: usize = 64 * 1024;
pub(crate) const MEMORY_KIB: u32 = 19 * 1024;
const PASSES: u32 = 2;
const LANES: u32 = 1;

pub(crate) type Key = Zeroizing<[u8; 32]>;
pub(crate) type Entries = BTreeMap<String, Secret>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Policy {
    pub(crate) mode: StorageMode,
    pub(crate) salt: [u8; 16],
}

impl Policy {
    pub(crate) fn new(mode: StorageMode) -> Result<Self, VaultError> {
        let mut salt = [0; 16];
        if mode == StorageMode::PinProtected {
            random(&mut salt)?;
        }
        Ok(Self { mode, salt })
    }

    pub(crate) fn key(self, pin: &[u8]) -> Result<Key, VaultError> {
        let mut key = Zeroizing::new([0; 32]);
        let mut blocks = Zeroizing::new(Vec::new());
        blocks
            .try_reserve_exact(MEMORY_KIB as usize)
            .map_err(|_| VaultError::StorageUnavailable)?;
        blocks.resize(MEMORY_KIB as usize, Block::default());
        let params = Params::new(MEMORY_KIB, PASSES, LANES, Some(key.len()))
            .map_err(|_| VaultError::UnsupportedFormat)?;
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into_with_memory(pin, &self.salt, &mut *key, &mut *blocks)
            .map_err(|_| VaultError::AuthenticationFailed)?;
        Ok(key)
    }
}

fn random(output: &mut [u8]) -> Result<(), VaultError> {
    getrandom::fill(output).map_err(|_| VaultError::EntropyUnavailable)
}

pub(crate) struct Record {
    pub(crate) generation: u64,
    pub(crate) policy: Policy,
    bytes: Zeroizing<Vec<u8>>,
}

impl Record {
    pub(crate) fn parse(bytes: Zeroizing<Vec<u8>>, domain: &str) -> Result<Self, VaultError> {
        if bytes.len() < HEADER_LEN + DIGEST_LEN + 2
            || bytes.len() > MAX_FILE
            || &bytes[..8] != MAGIC
        {
            return Err(VaultError::Corrupt);
        }
        let end = bytes.len() - DIGEST_LEN;
        if checksum(domain, &bytes[..end]).as_slice() != &bytes[end..] {
            return Err(VaultError::Corrupt);
        }
        if bytes[8] != VERSION {
            return Err(VaultError::UnsupportedFormat);
        }
        let mode = match bytes[9] {
            0 => StorageMode::PinProtected,
            1 => StorageMode::Unprotected,
            2 => StorageMode::SessionOnly,
            _ => return Err(VaultError::UnsupportedFormat),
        };
        let generation = u64::from_be_bytes(bytes[10..18].try_into().unwrap());
        if generation == 0 {
            return Err(VaultError::Corrupt);
        }
        let salt = bytes[30..46].try_into().unwrap();
        if mode == StorageMode::PinProtected {
            if bytes[18..22] != MEMORY_KIB.to_be_bytes()
                || bytes[22..26] != PASSES.to_be_bytes()
                || bytes[26..30] != LANES.to_be_bytes()
            {
                return Err(VaultError::UnsupportedFormat);
            }
            if end < HEADER_LEN + TAG_LEN + 2 {
                return Err(VaultError::Corrupt);
            }
        } else if bytes[18..HEADER_LEN].iter().any(|&byte| byte != 0) {
            return Err(VaultError::Corrupt);
        }
        Ok(Self {
            generation,
            policy: Policy { mode, salt },
            bytes,
        })
    }

    pub(crate) fn decode(&self, domain: &str, key: Option<&Key>) -> Result<Entries, VaultError> {
        let end = self.bytes.len() - DIGEST_LEN;
        let mut payload = Zeroizing::new(self.bytes[HEADER_LEN..end].to_vec());
        if self.policy.mode == StorageMode::PinProtected {
            let key = key.ok_or(VaultError::Locked)?;
            let tag = end - HEADER_LEN - TAG_LEN;
            let (body, auth_tag) = payload.split_at_mut(tag);
            XChaCha20Poly1305::new((&**key).into())
                .decrypt_in_place_detached(
                    self.bytes[46..HEADER_LEN].into(),
                    &associated(domain, &self.bytes[..HEADER_LEN]),
                    body,
                    (&*auth_tag).into(),
                )
                .map_err(|_| VaultError::AuthenticationFailed)?;
            payload.truncate(tag);
        }
        let entries = decode_entries(&payload)?;
        if self.policy.mode == StorageMode::SessionOnly && !entries.is_empty() {
            return Err(VaultError::Corrupt);
        }
        Ok(entries)
    }
}

pub(crate) fn encode(
    domain: &str,
    generation: u64,
    policy: Policy,
    key: Option<&Key>,
    entries: &Entries,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let mut payload = encode_entries(entries)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(
        HEADER_LEN + payload.len() + TAG_LEN + DIGEST_LEN,
    ));
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.push(match policy.mode {
        StorageMode::PinProtected => 0,
        StorageMode::Unprotected => 1,
        StorageMode::SessionOnly => 2,
    });
    bytes.extend_from_slice(&generation.to_be_bytes());
    if policy.mode == StorageMode::PinProtected {
        let mut nonce = [0; 24];
        random(&mut nonce)?;
        bytes.extend_from_slice(&MEMORY_KIB.to_be_bytes());
        bytes.extend_from_slice(&PASSES.to_be_bytes());
        bytes.extend_from_slice(&LANES.to_be_bytes());
        bytes.extend_from_slice(&policy.salt);
        bytes.extend_from_slice(&nonce);
        let key = key.ok_or(VaultError::Locked)?;
        let tag = XChaCha20Poly1305::new((&**key).into())
            .encrypt_in_place_detached((&nonce).into(), &associated(domain, &bytes), &mut payload)
            .map_err(|_| VaultError::TooLarge)?;
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(&tag);
    } else {
        bytes.resize(HEADER_LEN, 0);
        bytes.extend_from_slice(&payload);
    }
    let digest = checksum(domain, &bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

fn associated(domain: &str, header: &[u8]) -> Vec<u8> {
    [domain.as_bytes(), &[0], header].concat()
}

fn checksum(domain: &str, bytes: &[u8]) -> [u8; DIGEST_LEN] {
    let mut digest = Sha256::new();
    digest.update(domain.as_bytes());
    digest.update([0]);
    digest.update(bytes);
    digest.finalize().into()
}

pub(crate) fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

fn encode_entries(entries: &Entries) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let mut len = 2;
    for (name, value) in entries {
        let size = value.expose().len();
        if size > MAX_SECRET {
            return Err(VaultError::TooLarge);
        }
        len += 1 + name.len() + 4 + size;
        if len > MAX_PAYLOAD {
            return Err(VaultError::TooLarge);
        }
    }
    let count = u16::try_from(entries.len()).map_err(|_| VaultError::TooLarge)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(len));
    bytes.extend_from_slice(&count.to_be_bytes());
    for (name, value) in entries {
        bytes.push(name.len() as u8);
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&(value.expose().len() as u32).to_be_bytes());
        bytes.extend_from_slice(value.expose());
    }
    Ok(bytes)
}

fn decode_entries(mut payload: &[u8]) -> Result<Entries, VaultError> {
    fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], VaultError> {
        let (head, tail) = bytes.split_at_checked(len).ok_or(VaultError::Corrupt)?;
        *bytes = tail;
        Ok(head)
    }

    if payload.len() > MAX_PAYLOAD {
        return Err(VaultError::Corrupt);
    }
    let count = u16::from_be_bytes(take(&mut payload, 2)?.try_into().unwrap());
    let mut entries = Entries::new();
    for _ in 0..count {
        let len = take(&mut payload, 1)?[0] as usize;
        let name = std::str::from_utf8(take(&mut payload, len)?)
            .ok()
            .filter(|name| valid_name(name))
            .ok_or(VaultError::Corrupt)?;
        let len = u32::from_be_bytes(take(&mut payload, 4)?.try_into().unwrap()) as usize;
        if len > MAX_SECRET || entries.contains_key(name) {
            return Err(VaultError::Corrupt);
        }
        entries.insert(
            name.to_owned(),
            Secret::new(take(&mut payload, len)?.to_vec()),
        );
    }
    if !payload.is_empty() {
        return Err(VaultError::Corrupt);
    }
    Ok(entries)
}
