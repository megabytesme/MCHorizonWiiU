//! Cafe adapters for the getrandom versions used by Horizon.
//!
//! Call [`link`] from the Cafe executable before it uses getrandom. The
//! callbacks use the Cafe standard library's own system RNG and initialization.
//! Its current implementation aborts the process if NSS reports an error.

#![cfg_attr(target_os = "cafe", feature(random))]

#[cfg(target_os = "cafe")]
mod cafe {
    use std::random::{Rng, SystemRng};
    use std::sync::Once;

    /// Detect known broken system implementations before they can silently
    /// provide unchanged or obviously constant bytes. This is not an entropy
    /// quality test; the operating system remains responsible for that.
    fn ensure_system_rng() {
        static CHECK: Once = Once::new();
        CHECK.call_once(|| {
            let mut first = [0x55; 64];
            let mut second = [0xaa; 64];
            SystemRng.fill_bytes(&mut first);
            SystemRng.fill_bytes(&mut second);
            assert!(
                !first.iter().all(|&byte| byte == 0x55)
                    && !second.iter().all(|&byte| byte == 0xaa)
                    && !first.iter().all(|&byte| byte == 0)
                    && !second.iter().all(|&byte| byte == 0)
                    && first != second,
                "Cafe system RNG failed its startup check"
            );
        });
    }

    fn fill02(dest: &mut [u8]) -> Result<(), getrandom02::Error> {
        ensure_system_rng();
        SystemRng.fill_bytes(dest);
        Ok(())
    }

    getrandom02::register_custom_getrandom!(fill02);

    // getrandom 0.3/0.4 can pass MaybeUninit storage. Initialize it before
    // forming a normal byte slice for SystemRng. The getrandom caller supplies
    // a writable allocation valid for `len` bytes.
    unsafe fn fill_uninit(dest: *mut u8, len: usize) {
        if len == 0 {
            return;
        }
        ensure_system_rng();
        unsafe { std::ptr::write_bytes(dest, 0, len) };
        let bytes = unsafe { std::slice::from_raw_parts_mut(dest, len) };
        SystemRng.fill_bytes(bytes);
    }

    #[no_mangle]
    pub unsafe extern "Rust" fn __getrandom_v03_custom(
        dest: *mut u8,
        len: usize,
    ) -> Result<(), getrandom03::Error> {
        unsafe { fill_uninit(dest, len) };
        Ok(())
    }

    #[no_mangle]
    pub unsafe extern "Rust" fn __getrandom_v04_custom(
        dest: *mut u8,
        len: usize,
    ) -> Result<(), getrandom04::Error> {
        unsafe { fill_uninit(dest, len) };
        Ok(())
    }

    /// Link all three version-specific hooks and check the Cafe system RNG.
    /// The standard library initializes NSS on first use. Its current Cafe
    /// implementation aborts on NSS error, as does a failed startup check.
    #[inline(never)]
    pub fn link() {
        ensure_system_rng();
        // The 0.2 macro generates this Rust-ABI symbol; referencing it here
        // ensures the link anchor also retains the macro-generated adapter.
        extern "Rust" {
            fn __getrandom_custom(dest: *mut u8, len: usize) -> u32;
        }
        std::hint::black_box(__getrandom_custom as unsafe extern "Rust" fn(*mut u8, usize) -> u32);
        std::hint::black_box(
            __getrandom_v03_custom
                as unsafe extern "Rust" fn(*mut u8, usize) -> Result<(), getrandom03::Error>,
        );
        std::hint::black_box(
            __getrandom_v04_custom
                as unsafe extern "Rust" fn(*mut u8, usize) -> Result<(), getrandom04::Error>,
        );
    }
}

#[cfg(target_os = "cafe")]
pub use cafe::link;
