#![feature(random)]
use std::random::{Rng, SystemRng};
extern "C" {
    fn NSSInit() -> i32;
    fn NSSGetRandom(buf: *mut u8, len: i32) -> i32;
}
const INVALID: i32 = 0xFFD1FFF7u32 as i32;
const NOT_INITIALIZED: i32 = 0xFFD1FFFBu32 as i32;
#[no_mangle]
pub extern "C" fn horizon_entropy_probe() -> u32 {
    let mut flags = 0u32;
    let mut a = [0u8; 256];
    let mut b = [0u8; 256];
    let mut sentinel = [0x55u8; 256];
    unsafe {
        if NSSGetRandom(a.as_mut_ptr(), a.len() as i32) != NOT_INITIALIZED {
            flags |= 1;
        }
        if NSSGetRandom(core::ptr::null_mut(), 1) != INVALID {
            flags |= 2;
        }
        if NSSInit() != 0 {
            flags |= 4;
        }
        if NSSGetRandom(a.as_mut_ptr(), -1) != INVALID {
            flags |= 8;
        }
        if NSSGetRandom(0xF0000000usize as *mut u8, 16) != INVALID {
            flags |= 16;
        }
    }
    SystemRng.fill_bytes(&mut a);
    SystemRng.fill_bytes(&mut b);
    SystemRng.fill_bytes(&mut sentinel);
    if a.iter().all(|&x| x == 0) {
        flags |= 32;
    }
    if b.iter().all(|&x| x == 0) {
        flags |= 64;
    }
    if a == b {
        flags |= 128;
    }
    if sentinel.iter().all(|&x| x == 0x55) {
        flags |= 256;
    }
    flags
}
