//! Opt-in diagnostic for the Cafe standard library's operating-system RNG.
#![feature(random)]

use std::random::{Rng, SystemRng};

// These flags report obvious failures, not a statistical or cryptographic test.
fn inspect(a: &[u8], b: &[u8], sentinel: &[u8]) -> u32 {
    let mut failures = 0;
    if a.iter().all(|&byte| byte == 0) {
        failures |= 1;
    }
    if b.iter().all(|&byte| byte == 0) {
        failures |= 2;
    }
    if a == b {
        failures |= 4;
    }
    if sentinel.iter().all(|&byte| byte == 0x55) {
        failures |= 8;
    }
    failures
}

/// Called from the WUT diagnostic entry point. Entropy is never logged.
#[no_mangle]
pub extern "C" fn horizon_entropy_probe() -> u32 {
    let mut first = [0; 256];
    let mut second = [0; 256];
    let mut sentinel = [0x55; 256];
    SystemRng.fill_bytes(&mut first);
    SystemRng.fill_bytes(&mut second);
    SystemRng.fill_bytes(&mut sentinel);
    inspect(&first, &second, &sentinel)
}

#[cfg(test)]
mod tests {
    use super::inspect;

    #[test]
    fn identifies_unchanged_and_repeated_buffers() {
        let zero = [0; 4];
        let sentinel = [0x55; 4];
        assert_eq!(inspect(&zero, &zero, &sentinel), 15);
        assert_eq!(inspect(&[1, 2, 3, 4], &[4, 3, 2, 1], &[0x55, 0, 1, 2]), 0);
        assert_eq!(inspect(&[1; 4], &[1; 4], &[2; 4]), 4);
    }
}
