//! Platform-neutral account contracts. The prototype uses only an offline profile.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfflineName<'a>(&'a str);

impl<'a> OfflineName<'a> {
    pub fn parse(name: &'a str) -> Option<Self> {
        (1..=16).contains(&name.len()).then_some(())?;
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            .then_some(Self(name))
    }

    /// Internal names are compiled into the built-in profile list.
    pub(crate) const fn for_builtin(name: &'a str) -> Self {
        Self(name)
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile<'a> {
    Offline(OfflineName<'a>),
    Microsoft(AccountId),
}

pub struct SavedAccount<'a> {
    pub id: AccountId,
    pub player_name: &'a str,
}

/// Only these device-code fields may reach the UI. The private polling
/// credential stays inside the opaque request and expires on cancellation.
pub struct DevicePrompt<'a> {
    pub verification_uri: &'a str,
    pub user_code: &'a str,
    pub expires_in_seconds: u32,
    pub poll_interval_seconds: u32,
}

pub enum DevicePoll<A> {
    Pending,
    SlowDown,
    Authorized(A),
    Denied,
    Expired,
}

pub enum Ownership {
    Owned,
    Missing,
}

/// Credential storage must protect refresh credentials at rest and require
/// authenticated platform access. No Cafe implementation is supplied.
pub trait RefreshVault {
    type Credential;
    type Error;

    fn store(&mut self, id: AccountId, credential: Self::Credential) -> Result<(), Self::Error>;
    fn load(&mut self, id: AccountId) -> Result<Option<Self::Credential>, Self::Error>;
    fn remove(&mut self, id: AccountId) -> Result<(), Self::Error>;
}

/// These operations belong on a cancellable account worker. Provider types holding
/// credentials must avoid Debug/serialization and keep credentials out of launch args.
pub trait MicrosoftAccounts {
    type Request;
    type Authorization;
    type GameIdentity;
    type Error;

    fn begin_device_code(&mut self) -> Result<Self::Request, Self::Error>;
    fn device_prompt(request: &Self::Request) -> DevicePrompt<'_>;
    fn poll(
        &mut self,
        request: &mut Self::Request,
    ) -> Result<DevicePoll<Self::Authorization>, Self::Error>;
    fn cancel(&mut self, request: Self::Request);
    fn ownership(&mut self, authorization: &Self::Authorization) -> Result<Ownership, Self::Error>;
    /// Commit a saved account only after ownership and secure storage succeed.
    fn save(&mut self, authorization: Self::Authorization) -> Result<AccountId, Self::Error>;
    fn saved_accounts(&self) -> &[SavedAccount<'_>];
    /// Refresh as needed and recheck entitlement before returning an in-memory identity.
    fn select(&mut self, id: AccountId) -> Result<Self::GameIdentity, Self::Error>;
    /// Stop in-flight work for this account, discard in-memory identity and
    /// delete its secure refresh credential before reporting success.
    fn sign_out(&mut self, id: AccountId) -> Result<(), Self::Error>;
}
