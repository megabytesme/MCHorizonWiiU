//! Opt-in Cemu diagnostic guard for process-wide local-world exclusion.
//!
//! This intentionally uses one global port, so it treats every world as the
//! same lock: opening any world conflicts with any other holder. It is a Cemu
//! diagnostic only and has not been validated on Wii U hardware. The caller
//! must opt in with `horizon_wiiu_cemu_global_lock`; failures are returned and
//! must not be treated as permission to continue without a lock.

use std::io;

unsafe extern "C" {
    /// Opens the C bridge's process-wide loopback listener lock.
    ///
    /// Negative results report failure; nonnegative results are owned fds.
    fn horizon_wiiu_cemu_global_lock_open() -> i32;
    /// Closes a listener fd returned by `horizon_wiiu_cemu_global_lock_open`.
    fn horizon_wiiu_cemu_global_lock_close(fd: i32);
}

/// Holds the opt-in Cemu-only global session lock until dropped.
///
/// The lock is global across worlds rather than keyed by world path.
pub struct CemuGlobalSessionLock {
    fd: i32,
}

impl CemuGlobalSessionLock {
    /// Acquires the global lock or returns the bridge's negative failure code.
    pub fn try_acquire() -> io::Result<Self> {
        // SAFETY: The C bridge owns nsysnet initialization and returns an fd
        // whose ownership transfers to this guard on success.
        let fd = unsafe { horizon_wiiu_cemu_global_lock_open() };
        if fd < 0 {
            return Err(io::Error::other(format!(
                "Cafe Cemu global session lock failed ({fd})"
            )));
        }
        Ok(Self { fd })
    }
}

impl Drop for CemuGlobalSessionLock {
    fn drop(&mut self) {
        // SAFETY: This guard uniquely owns the fd returned by the bridge.
        unsafe { horizon_wiiu_cemu_global_lock_close(self.fd) };
    }
}
