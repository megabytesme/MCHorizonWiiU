//! Gameplay and startup, above the reusable client library.
// No `#![recursion_limit]` or other inner attribute horizon-mixins doesn't know here: the
// launcher's horizon-mixins then rejects every native mod targeting this crate (#8169).
#![forbid(unsafe_code)]
#[cfg(not(target_has_atomic = "64"))]
pub(crate) use portable_atomic::{AtomicI64, AtomicU64};
#[cfg(target_has_atomic = "64")]
pub(crate) use std::sync::atomic::{AtomicI64, AtomicU64};
extern crate self as horizon_client;
pub use horizon_client_lib::*;
pub mod entry;
pub mod game;
pub mod mod_api;
