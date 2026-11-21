//! Local and application-owned credential storage for Horizon Night.

use std::fmt;

use zeroize::Zeroizing;

mod format;
mod vault;

pub use vault::Vault;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StorageMode {
    #[default]
    PinProtected,
    Unprotected,
    SessionOnly,
}

/// The UI must obtain explicit consent before constructing this variant for
/// either first-time setup or a change from PIN protection.
pub enum Protection<'a> {
    PinProtected { new_pin: &'a [u8] },
    ConfirmedUnprotected,
    SessionOnly,
}

/// Owned opaque bytes. The Debug representation is safe to report.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageStatus {
    pub mode: StorageMode,
    pub initialized: bool,
    pub locked: bool,
    /// The current generation is usable. An older or interrupted write
    /// could not be cleaned up and may still be present on the SD card.
    pub cleanup_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultError {
    SetupRequired,
    Locked,
    InvalidPinLength,
    /// Wrong PIN, or authenticated data that could not be verified.
    AuthenticationFailed,
    Corrupt,
    UnsupportedFormat,
    InvalidName,
    TooLarge,
    EntropyUnavailable,
    StorageUnavailable,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SetupRequired => "Choose how to store credentials first.",
            Self::Locked => "Unlock this vault locally first.",
            Self::InvalidPinLength => "Use a PIN or passphrase of 6 to 128 bytes.",
            Self::AuthenticationFailed => "The PIN or protected vault could not be verified.",
            Self::Corrupt => "The stored vault is damaged.",
            Self::UnsupportedFormat => {
                "This vault format requires a different application version."
            }
            Self::InvalidName => "Invalid vault or credential identifier.",
            Self::TooLarge => "This credential exceeds the vault size limit.",
            Self::EntropyUnavailable => "Secure randomness is unavailable.",
            Self::StorageUnavailable => "The vault could not access application storage.",
        })
    }
}

impl std::error::Error for VaultError {}

/// One store controls one PIN domain. The app prompts locally upon Locked.
/// Services never receive the PIN or a handle for another service's domain.
pub trait SecretStore {
    fn status(&self) -> StorageStatus;
    fn unlock(&mut self, pin: &[u8]) -> Result<(), VaultError>;
    fn lock(&mut self);
    fn get(&mut self, name: &str) -> Result<Option<Secret>, VaultError>;
    fn put(&mut self, name: &str, secret: Secret) -> Result<(), VaultError>;
    fn remove(&mut self, name: &str) -> Result<(), VaultError>;
    /// Initialize a new vault or migrate an unlocked vault. To change the
    /// PIN, unlock first and pass the replacement PIN as the new protection.
    fn set_protection(&mut self, protection: Protection<'_>) -> Result<(), VaultError>;
}
