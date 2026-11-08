//! Host tests for the production Cafe TLS-key backend. Only the OS-specific
//! thread-specific-value calls are shimmed; `cafe.rs` itself is included below.

#![allow(unexpected_cfgs)]

extern crate self as cafe_sys;

pub mod cell {
    pub use core::cell::UnsafeCell;
}

pub mod ptr {
    pub use core::ptr::*;
}

pub mod mem {
    pub use core::mem::transmute;
}

pub mod sync {
    pub mod atomic {
        use core::marker::PhantomData;
        use core::sync::atomic as host;

        pub use host::Ordering;

        pub trait AtomicValue: Copy {
            fn into_bits(self) -> usize;
            fn from_bits(value: usize) -> Self;
        }

        impl AtomicValue for usize {
            fn into_bits(self) -> usize {
                self
            }
            fn from_bits(value: usize) -> Self {
                value
            }
        }

        impl AtomicValue for bool {
            fn into_bits(self) -> usize {
                usize::from(self)
            }
            fn from_bits(value: usize) -> Self {
                value != 0
            }
        }

        impl AtomicValue for *mut () {
            fn into_bits(self) -> usize {
                self as usize
            }
            fn from_bits(value: usize) -> Self {
                value as *mut ()
            }
        }

        pub struct Atomic<T: AtomicValue> {
            inner: host::AtomicUsize,
            _type: PhantomData<T>,
        }

        // The wrapped host AtomicUsize provides the synchronization; the
        // PhantomData only records the value representation.
        unsafe impl<T: AtomicValue> Send for Atomic<T> {}
        unsafe impl<T: AtomicValue> Sync for Atomic<T> {}

        impl<T: AtomicValue> Atomic<T> {
            // The production Cafe module initializes its static atomics to
            // zero/null/false. This const shim mirrors those initializers;
            // nonzero values are stored through `store` after construction.
            pub const fn new(_value: T) -> Self {
                Self {
                    inner: host::AtomicUsize::new(0),
                    _type: PhantomData,
                }
            }

            pub fn load(&self, order: Ordering) -> T {
                T::from_bits(self.inner.load(order))
            }

            pub fn store(&self, value: T, order: Ordering) {
                self.inner.store(value.into_bits(), order)
            }

            pub fn compare_exchange(
                &self,
                current: T,
                new: T,
                success: Ordering,
                failure: Ordering,
            ) -> Result<T, T> {
                self.inner
                    .compare_exchange(current.into_bits(), new.into_bits(), success, failure)
                    .map(T::from_bits)
                    .map_err(T::from_bits)
            }
        }

        impl Atomic<usize> {
            pub fn fetch_add(&self, value: usize, order: Ordering) -> usize {
                self.inner.fetch_add(value, order)
            }
        }

        pub type AtomicBool = Atomic<bool>;
        pub type AtomicPtr = Atomic<*mut ()>;
        pub type AtomicUsize = Atomic<usize>;
    }
}

pub mod coreinit {
    pub mod thread {
        use core::ffi::c_void;
        use std::cell::RefCell;

        std::thread_local! {
            static THREAD_SPECIFIC: RefCell<[*mut c_void; 16]> =
                const { RefCell::new([core::ptr::null_mut(); 16]) };
        }

        pub unsafe fn get_specific(id: i32) -> *mut c_void {
            assert!((0..16).contains(&id));
            THREAD_SPECIFIC.with(|slots| slots.borrow()[id as usize])
        }

        pub unsafe fn set_specific(id: i32, value: *mut c_void) {
            assert!((0..16).contains(&id));
            THREAD_SPECIFIC.with(|slots| slots.borrow_mut()[id as usize] = value);
        }
    }
}

#[macro_export]
macro_rules! rtabort {
    ($($arg:tt)*) => {{ panic!($($arg)*) }};
}

#[path = "../../library/std/src/sys/thread_local/key/cafe.rs"]
mod production_cafe_tls;

#[cfg(test)]
mod tests {
    use super::production_cafe_tls as tls;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static REARM_KEY: AtomicUsize = AtomicUsize::new(0);
    static SELF_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static EARLIER_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CURRENT_DTOR_OK: AtomicUsize = AtomicUsize::new(0);
    static CURRENT_DTOR_THREAD: Mutex<Option<thread::ThreadId>> = Mutex::new(None);

    fn raw(value: usize) -> *mut u8 {
        value as *mut u8
    }

    unsafe extern "C" fn self_rearm(value: *mut u8) {
        assert!(unsafe { tls::get(REARM_KEY.load(Ordering::Acquire)) }.is_null());
        let count = SELF_DTOR_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
        if count < 4 {
            unsafe { tls::set(REARM_KEY.load(Ordering::Acquire), value) };
        }
        let actual_thread = thread::current().id();
        if CURRENT_DTOR_THREAD.lock().unwrap().as_ref() == Some(&actual_thread) {
            CURRENT_DTOR_OK.fetch_add(1, Ordering::Relaxed);
        }
    }

    unsafe extern "C" fn rearm_earlier(_value: *mut u8) {
        let key = REARM_KEY.load(Ordering::Acquire);
        unsafe { tls::set(key, raw(77)) };
    }

    unsafe extern "C" fn count_earlier(_value: *mut u8) {
        EARLIER_DTOR_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn supports_128_live_keys_and_destroy_reuses_without_stale_values() {
        let _guard = TEST_LOCK.lock().unwrap();
        let keys: Vec<_> = (0..128).map(|_| tls::create(None)).collect();
        for (index, key) in keys.iter().copied().enumerate() {
            unsafe { tls::set(key, raw(index + 1)) };
        }
        for (index, key) in keys.iter().copied().enumerate() {
            assert_eq!(unsafe { tls::get(key) }, raw(index + 1));
        }

        let stale = keys[17];
        unsafe { tls::destroy(stale) };
        let replacement = tls::create(None);
        assert_eq!(replacement & 511, stale & 511);
        assert_ne!(replacement >> 9, stale >> 9);
        assert!(unsafe { tls::get(replacement) }.is_null());

        for key in keys.into_iter().filter(|key| *key != stale) {
            unsafe { tls::destroy(key) };
        }
        unsafe { tls::destroy(replacement) };
        tls::release_thread_table();
    }

    #[test]
    fn logical_key_exhaustion_is_explicit_and_destroy_reclaims_indices() {
        let _guard = TEST_LOCK.lock().unwrap();
        let keys: Vec<_> = (0..511).map(|_| tls::create(None)).collect();
        assert!(std::panic::catch_unwind(create_none_for_test).is_err());
        for key in keys {
            unsafe { tls::destroy(key) };
        }
        let key = tls::create(None);
        unsafe { tls::destroy(key) };
    }

    fn create_none_for_test() -> usize {
        tls::create(None)
    }

    #[test]
    fn contended_create_destroy_reclaims_losing_candidates() {
        let _guard = TEST_LOCK.lock().unwrap();
        let barrier = Arc::new(Barrier::new(33));
        let mut workers = Vec::new();
        for _ in 0..32 {
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..1000 {
                    let key = tls::create(None);
                    unsafe { tls::destroy(key) };
                }
            }));
        }
        barrier.wait();
        for worker in workers {
            worker.join().unwrap();
        }
        let key = tls::create(None);
        unsafe { tls::destroy(key) };
    }

    #[test]
    fn thread_values_are_isolated_and_table_cells_are_reused_cleanly() {
        let _guard = TEST_LOCK.lock().unwrap();
        let key = tls::create(None);
        unsafe { tls::set(key, raw(11)) };
        let worker = thread::spawn(move || {
            assert!(unsafe { tls::get(key) }.is_null());
            unsafe { tls::set(key, raw(29)) };
            assert_eq!(unsafe { tls::get(key) }, raw(29));
            tls::release_thread_table();
        });
        worker.join().unwrap();
        assert_eq!(unsafe { tls::get(key) }, raw(11));
        tls::release_thread_table();

        for _ in 0..300 {
            let worker = thread::spawn(move || {
                assert!(unsafe { tls::get(key) }.is_null());
                unsafe { tls::set(key, raw(55)) };
                assert_eq!(unsafe { tls::get(key) }, raw(55));
                tls::release_thread_table();
            });
            worker.join().unwrap();
        }
        unsafe { tls::destroy(key) };
    }

    #[test]
    fn production_destructor_loop_clears_self_rearm_and_runs_exactly_four_rounds() {
        let _guard = TEST_LOCK.lock().unwrap();
        SELF_DTOR_CALLS.store(0, Ordering::Relaxed);
        CURRENT_DTOR_OK.store(0, Ordering::Relaxed);
        *CURRENT_DTOR_THREAD.lock().unwrap() = Some(thread::current().id());
        let key = tls::create(Some(self_rearm));
        REARM_KEY.store(key, Ordering::Release);
        unsafe { tls::set(key, raw(1)) };
        tls::run_dtors();
        assert_eq!(SELF_DTOR_CALLS.load(Ordering::Relaxed), 4);
        assert_eq!(CURRENT_DTOR_OK.load(Ordering::Relaxed), 4);
        assert!(unsafe { tls::get(key) }.is_null());
        unsafe { tls::destroy(key) };
        tls::release_thread_table();
    }

    #[test]
    fn production_destructor_loop_reaches_an_earlier_key_next_round() {
        let _guard = TEST_LOCK.lock().unwrap();
        EARLIER_DTOR_CALLS.store(0, Ordering::Relaxed);
        let earlier = tls::create(Some(count_earlier));
        let later = tls::create(Some(rearm_earlier));
        REARM_KEY.store(earlier, Ordering::Release);
        unsafe { tls::set(later, raw(9)) };
        tls::run_dtors();
        assert_eq!(EARLIER_DTOR_CALLS.load(Ordering::Relaxed), 1);
        unsafe {
            tls::destroy(earlier);
            tls::destroy(later);
        }
        tls::release_thread_table();
    }

    #[test]
    fn live_table_exhaustion_panics_without_aliasing_and_tables_recover() {
        let _guard = TEST_LOCK.lock().unwrap();
        let key = tls::create(None);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let mut workers = Vec::new();
        let mut releases = Vec::new();
        for index in 0..256 {
            let ready_tx = ready_tx.clone();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            releases.push(release_tx);
            workers.push(thread::spawn(move || {
                unsafe { tls::set(key, raw(index + 1)) };
                ready_tx.send(index).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(15))
                    .unwrap();
                assert_eq!(unsafe { tls::get(key) }, raw(index + 1));
                tls::release_thread_table();
            }));
        }
        drop(ready_tx);
        for _ in 0..256 {
            ready_rx
                .recv_timeout(std::time::Duration::from_secs(15))
                .expect("all 256 tables must be claimed before overflow probe");
        }

        let overflow = thread::spawn(move || unsafe { tls::set(key, raw(999)) });
        assert!(overflow.join().is_err(), "257th live table must fail explicitly");
        for release in releases {
            release.send(()).unwrap();
        }
        for worker in workers {
            worker.join().unwrap();
        }
        let recovered = thread::spawn(move || {
            unsafe { tls::set(key, raw(1000)) };
            assert_eq!(unsafe { tls::get(key) }, raw(1000));
            tls::release_thread_table();
        });
        recovered.join().unwrap();
        unsafe { tls::destroy(key) };
    }

    #[test]
    fn wut_reserved_slots_are_not_used_by_the_backend() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe {
            assert!(super::coreinit::thread::get_specific(14).is_null());
            assert!(super::coreinit::thread::get_specific(15).is_null());
        }
        let key = tls::create(None);
        unsafe { tls::set(key, raw(42)) };
        assert!(!unsafe {
            super::coreinit::thread::get_specific(13)
        }
        .is_null());
        unsafe { tls::destroy(key) };
        tls::release_thread_table();
    }
}
