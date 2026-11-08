use cafe_sys::coreinit::thread;

use crate::ptr;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicPtr, AtomicUsize};

pub type Key = usize;
type Dtor = unsafe extern "C" fn(*mut u8);

// Cafe OS has 16 thread-specific slots per thread. Slot 0 is skipped
// because 0 is the "not yet created" sentinel of `LazyKey`.
const SLOT_COUNT: usize = 16;
static NEXT_KEY: Atomic<usize> = AtomicUsize::new(1);

static DTORS: [Atomic<*mut ()>; SLOT_COUNT] =
    [const { AtomicPtr::new(ptr::null_mut()) }; SLOT_COUNT];

pub fn create(dtor: Option<Dtor>) -> Key {
    let key = NEXT_KEY.fetch_add(1, Relaxed);
    if key >= SLOT_COUNT {
        rtabort!("out of TLS keys");
    }
    if let Some(dtor) = dtor {
        DTORS[key].store(dtor as *mut (), Release);
    }
    key
}

/// Runs the destructors of the calling thread's values, like POSIX does at thread exit.
/// Destructors may store new values, so this repeats for a few rounds.
pub fn run_dtors() {
    for _ in 0..4 {
        let mut ran = false;
        for key in 1..SLOT_COUNT {
            let dtor = DTORS[key].load(Acquire);
            let value = unsafe { get(key) };
            if dtor.is_null() || value.is_null() {
                continue;
            }
            unsafe {
                set(key, ptr::null_mut());
                crate::mem::transmute::<*mut (), Dtor>(dtor)(value);
            }
            ran = true;
        }
        if !ran {
            break;
        }
    }
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    unsafe { thread::set_specific(key as i32, value.cast()) }
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    unsafe { thread::get_specific(key as i32).cast() }
}

#[inline]
pub unsafe fn destroy(_key: Key) {}
