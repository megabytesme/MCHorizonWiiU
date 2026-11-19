//! Bounded behavioral probe for the Cafe std TLS-key virtualization patch.

use std::cell::Cell;
use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn horizon_cafe_tls_self_rearm_probe() -> u32;
}

const RACE_THREADS: usize = 16;
const REUSE_THREADS: usize = 300;
const WAIT: Duration = Duration::from_secs(5);
static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
static CURRENT_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
static EARLIER_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
static REARM_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static DROP_MARKER: DropMarker = const { DropMarker };
    static EARLIER_DROP_MARKER: EarlierDropMarker = const { EarlierDropMarker };
    static REARM_MARKER: RearmMarker = const { RearmMarker };
}

struct DropMarker;

impl Drop for DropMarker {
    fn drop(&mut self) {
        let _thread_id = thread::current().id();
        CURRENT_DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        DROP_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

struct EarlierDropMarker;

impl Drop for EarlierDropMarker {
    fn drop(&mut self) {
        EARLIER_DROP_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

struct RearmMarker;

impl Drop for RearmMarker {
    fn drop(&mut self) {
        REARM_DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        EARLIER_DROP_MARKER.with(|_| {});
    }
}

fn finish_bounded<T>(worker: thread::JoinHandle<T>) -> Option<T> {
    let deadline = Instant::now() + WAIT;
    while !worker.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    if worker.is_finished() {
        worker.join().ok()
    } else {
        None
    }
}

macro_rules! tls_keys {
    ($check:ident, $verify:ident; $($key:ident),+ $(,)?) => {
        thread_local! {
            $(static $key: Cell<u32> = const { Cell::new(0) };)+
        }

        fn $check(base: u32) -> bool {
            let mut expected = base;
            let mut okay = true;
            $(
                expected += 1;
                $key.with(|value| {
                    okay &= value.get() == 0;
                    value.set(expected);
                    okay &= value.get() == expected;
                });
            )+
            okay
        }

        fn $verify(base: u32) -> bool {
            let mut expected = base;
            let mut okay = true;
            $(
                expected += 1;
                $key.with(|value| okay &= value.get() == expected);
            )+
            okay
        }
    };
}

tls_keys!(check_race_keys, verify_race_keys;
    TLS_00, TLS_01, TLS_02, TLS_03, TLS_04, TLS_05, TLS_06, TLS_07,
    TLS_08, TLS_09, TLS_10, TLS_11, TLS_12, TLS_13, TLS_14, TLS_15,
    TLS_16, TLS_17, TLS_18, TLS_19, TLS_20, TLS_21, TLS_22, TLS_23,
    TLS_24, TLS_25, TLS_26, TLS_27, TLS_28, TLS_29, TLS_30, TLS_31,
);

tls_keys!(check_parent_keys, verify_parent_keys;
    PARENT_00, PARENT_01, PARENT_02, PARENT_03, PARENT_04, PARENT_05, PARENT_06, PARENT_07,
    PARENT_08, PARENT_09, PARENT_10, PARENT_11, PARENT_12, PARENT_13, PARENT_14, PARENT_15,
    PARENT_16, PARENT_17, PARENT_18, PARENT_19, PARENT_20, PARENT_21, PARENT_22, PARENT_23,
    PARENT_24, PARENT_25, PARENT_26, PARENT_27, PARENT_28, PARENT_29, PARENT_30, PARENT_31,
);

tls_keys!(check_capacity_keys, verify_capacity_keys;
    CAPACITY_000, CAPACITY_001, CAPACITY_002, CAPACITY_003, CAPACITY_004, CAPACITY_005,
    CAPACITY_006, CAPACITY_007, CAPACITY_008, CAPACITY_009, CAPACITY_010, CAPACITY_011,
    CAPACITY_012, CAPACITY_013, CAPACITY_014, CAPACITY_015, CAPACITY_016, CAPACITY_017,
    CAPACITY_018, CAPACITY_019, CAPACITY_020, CAPACITY_021, CAPACITY_022, CAPACITY_023,
    CAPACITY_024, CAPACITY_025, CAPACITY_026, CAPACITY_027, CAPACITY_028, CAPACITY_029,
    CAPACITY_030, CAPACITY_031, CAPACITY_032, CAPACITY_033, CAPACITY_034, CAPACITY_035,
    CAPACITY_036, CAPACITY_037, CAPACITY_038, CAPACITY_039, CAPACITY_040, CAPACITY_041,
    CAPACITY_042, CAPACITY_043, CAPACITY_044, CAPACITY_045, CAPACITY_046, CAPACITY_047,
    CAPACITY_048, CAPACITY_049, CAPACITY_050, CAPACITY_051, CAPACITY_052, CAPACITY_053,
    CAPACITY_054, CAPACITY_055, CAPACITY_056, CAPACITY_057, CAPACITY_058, CAPACITY_059,
    CAPACITY_060, CAPACITY_061, CAPACITY_062, CAPACITY_063, CAPACITY_064, CAPACITY_065,
    CAPACITY_066, CAPACITY_067, CAPACITY_068, CAPACITY_069, CAPACITY_070, CAPACITY_071,
    CAPACITY_072, CAPACITY_073, CAPACITY_074, CAPACITY_075, CAPACITY_076, CAPACITY_077,
    CAPACITY_078, CAPACITY_079, CAPACITY_080, CAPACITY_081, CAPACITY_082, CAPACITY_083,
    CAPACITY_084, CAPACITY_085, CAPACITY_086, CAPACITY_087, CAPACITY_088, CAPACITY_089,
    CAPACITY_090, CAPACITY_091, CAPACITY_092, CAPACITY_093, CAPACITY_094, CAPACITY_095,
);

/// Failure bits: more than 15 keys, per-thread isolation, thread destructor,
/// and reclamation/reuse of a thread table across sequential Rust threads.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_tls_probe() -> u32 {
    const FAIL_KEYS: u32 = 1 << 0;
    const FAIL_ISOLATION: u32 = 1 << 1;
    const FAIL_DTOR: u32 = 1 << 2;
    const FAIL_REUSE: u32 = 1 << 3;
    const FAIL_REARM: u32 = 1 << 4;

    let mut failures = 0;
    DROP_COUNT.store(0, Ordering::Relaxed);
    CURRENT_DROP_COUNT.store(0, Ordering::Relaxed);
    EARLIER_DROP_COUNT.store(0, Ordering::Relaxed);
    REARM_DROP_COUNT.store(0, Ordering::Relaxed);

    if unsafe { horizon_cafe_tls_self_rearm_probe() } != 4 {
        failures |= FAIL_REARM;
    }

    // Initialize an earlier key on the main thread. The spawned thread's
    // REARM_MARKER destructor will initialize its own earlier-key value only
    // after that key's index has already been scanned in destructor round 1.
    EARLIER_DROP_MARKER.with(|_| {});
    if !check_parent_keys(2000) || !check_capacity_keys(4000) {
        failures |= FAIL_ISOLATION;
    }

    // Race first access to 32 LocalKeys. Losing LazyKey candidates must be
    // destroyed and immediately reusable, not consume the logical-key pool.
    let barrier = Arc::new(Barrier::new(RACE_THREADS + 1));
    let mut workers = Vec::with_capacity(RACE_THREADS);
    let mut receivers = Vec::with_capacity(RACE_THREADS);
    for worker_index in 0..RACE_THREADS {
        let barrier = Arc::clone(&barrier);
        let (tx, rx) = mpsc::channel();
        receivers.push(rx);
        workers.push(thread::spawn(move || {
            barrier.wait();
            let base = 100 + worker_index as u32 * 100;
            let okay = check_race_keys(base) && verify_race_keys(base);
            DROP_MARKER.with(|_| {});
            if worker_index == 0 {
                REARM_MARKER.with(|_| {});
            }
            let _ = tx.send(okay);
        }));
    }
    barrier.wait();

    for receiver in receivers {
        if !matches!(receiver.recv_timeout(WAIT), Ok(true)) {
            failures |= FAIL_KEYS;
        }
    }
    for worker in workers {
        if finish_bounded(worker).is_none() {
            failures |= FAIL_DTOR;
        }
    }
    if DROP_COUNT.load(Ordering::Relaxed) != RACE_THREADS
        || CURRENT_DROP_COUNT.load(Ordering::Relaxed) != RACE_THREADS
    {
        failures |= FAIL_DTOR;
    }

    // Parent values must still contain their defaults; verification does not
    // overwrite them, so a shared-table bug cannot mask cross-thread writes.
    if !verify_parent_keys(2000) || !verify_capacity_keys(4000) {
        failures |= FAIL_ISOLATION;
    }

    // The first-key destructor ran before REARM_MARKER and was reached again
    // on the next destructor pass. This checks cross-key re-registration.
    if REARM_DROP_COUNT.load(Ordering::Relaxed) != 1
        || EARLIER_DROP_COUNT.load(Ordering::Relaxed) != 1
    {
        failures |= FAIL_REARM;
    }

    // Reclaim and reuse tables across more sequential Rust threads than the
    // table pool. Completion waits are bounded; a failed case does not join a
    // worker whose shutdown behavior is under test.
    let mut reuse_failed = false;
    for reused in 0..REUSE_THREADS {
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let okay = check_race_keys(3000);
            DROP_MARKER.with(|_| {});
            let _ = tx.send(okay);
        });
        let okay = matches!(rx.recv_timeout(WAIT), Ok(true));
        if !okay {
            reuse_failed = true;
            break;
        }
        if finish_bounded(worker).is_none() {
            reuse_failed = true;
            break;
        }
        if DROP_COUNT.load(Ordering::Relaxed) < RACE_THREADS + reused + 1 {
            reuse_failed = true;
            break;
        }
    }
    if reuse_failed {
        failures |= FAIL_DTOR;
        failures |= FAIL_REUSE;
    }

    failures
}
