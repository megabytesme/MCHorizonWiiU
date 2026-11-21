//! Microsoft device authorization and the Minecraft Java identity exchange.

#[cfg(any(test, feature = "fixture"))]
mod fixture;
mod profiles;
mod protocol;
mod worker;

pub use profiles::{Accounts, Profile, ProfileState};
pub use protocol::{Error, Identity, ProfileId};
pub use worker::{DevicePrompt, Event, Provider, Verified, Work};

#[cfg(test)]
mod tests;
