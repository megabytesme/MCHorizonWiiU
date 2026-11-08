use cafe_sys::coreinit::thread;

use crate::cell::UnsafeCell;
use crate::ptr;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicPtr, AtomicUsize};

pub type Key = usize;
type Dtor = unsafe extern "C" fn(*mut u8);

// Cafe OS exposes 16 thread-specific values. WUT reserves 14 and 15, so
// reserve the app-owned slot 13 as the root of Rust's logical TLS table.
// A WUT application embedding this std must not use OS_THREAD_SPECIFIC_13.
const THREAD_TABLE_OS_SLOT: i32 = 13;

// The old Cafe implementation exposed only 15 logical keys because it mapped
// each key directly to an OS slot. Rust's thread-local statics and std need
// more. Keep the table bounded and allocation-free to avoid TLS/allocator
// recursion during first access.
const KEY_COUNT: usize = 512;
const THREAD_TABLE_COUNT: usize = 256;
const KEY_INDEX_BITS: usize = 9;
const KEY_INDEX_MASK: usize = KEY_COUNT - 1;
const KEY_GENERATION_MAX: usize = usize::MAX >> KEY_INDEX_BITS;

const KEY_FREE: usize = 0;
const KEY_INITIALIZING: usize = 1;
const KEY_LIVE: usize = 2;
const KEY_RETIRED: usize = 3;

struct KeyMetadata {
    state: AtomicUsize,
    dtor: Atomic<*mut ()>,
}

impl KeyMetadata {
    const fn new() -> Self {
        Self {
            state: AtomicUsize::new(KEY_FREE),
            dtor: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

static KEYS: [KeyMetadata; KEY_COUNT] = [const { KeyMetadata::new() }; KEY_COUNT];

#[derive(Clone, Copy)]
struct TlsCell {
    generation: usize,
    value: *mut u8,
}

impl TlsCell {
    const fn empty() -> Self {
        Self {
            generation: 0,
            value: ptr::null_mut(),
        }
    }
}

struct ThreadTable {
    claimed: AtomicUsize,
    values: UnsafeCell<[TlsCell; KEY_COUNT]>,
}

impl ThreadTable {
    const fn new() -> Self {
        Self {
            claimed: AtomicUsize::new(0),
            values: UnsafeCell::new([TlsCell::empty(); KEY_COUNT]),
        }
    }
}

// A table is only accessed by the OS thread whose slot 13 points to it. The
// atomic claim bit transfers ownership before the pointer is published and
// after the previous owner has cleared it.
unsafe impl Sync for ThreadTable {}

static THREAD_TABLES: [ThreadTable; THREAD_TABLE_COUNT] =
    [const { ThreadTable::new() }; THREAD_TABLE_COUNT];

#[inline]
const fn key_state(generation: usize, state: usize) -> usize {
    (generation << 2) | state
}

pub fn create(dtor: Option<Dtor>) -> Key {
    for index in 1..KEY_COUNT {
        let metadata = &KEYS[index];
        let previous_state = metadata.state.load(Relaxed);
        if previous_state & 3 != KEY_FREE {
            continue;
        }

        let previous_generation = previous_state >> 2;
        if previous_generation == KEY_GENERATION_MAX {
            let _ = metadata.state.compare_exchange(
                previous_state,
                key_state(previous_generation, KEY_RETIRED),
                Acquire,
                Relaxed,
            );
            continue;
        }

        let generation = previous_generation + 1;
        if metadata
            .state
            .compare_exchange(
                previous_state,
                key_state(generation, KEY_INITIALIZING),
                Acquire,
                Relaxed,
            )
            .is_err()
        {
            continue;
        }
        metadata
            .dtor
            .store(dtor.map_or(ptr::null_mut(), |dtor| dtor as *mut ()), Relaxed);
        metadata.state.store(key_state(generation, KEY_LIVE), Release);
        return (generation << KEY_INDEX_BITS) | index;
    }

    rtabort!("out of Cafe TLS logical keys (maximum 511 active keys)");
}

#[inline]
fn decode_key(key: Key) -> Option<(usize, usize)> {
    let index = key & KEY_INDEX_MASK;
    let generation = key >> KEY_INDEX_BITS;
    if index == 0 || generation == 0 || index >= KEY_COUNT {
        None
    } else {
        Some((index, generation))
    }
}

#[inline]
fn key_is_live(index: usize, generation: usize) -> bool {
    let metadata = &KEYS[index];
    metadata.state.load(Acquire) == key_state(generation, KEY_LIVE)
}

/// Return this thread's table, creating it from the static pool if necessary.
/// This path must stay allocation-free and must not use std locks or TLS.
fn thread_table() -> &'static ThreadTable {
    let existing = unsafe { thread::get_specific(THREAD_TABLE_OS_SLOT).cast::<ThreadTable>() };
    if !existing.is_null() {
        return unsafe { &*existing };
    }

    for table in &THREAD_TABLES {
        if table
            .claimed
            .compare_exchange(0, 1, Acquire, Relaxed)
            .is_err()
        {
            continue;
        }

        // Clear every cell before making the table visible to the new owner.
        // This prevents values from a thread whose table was reclaimed from
        // being observed after table reuse.
        unsafe { *table.values.get() = [TlsCell::empty(); KEY_COUNT] };
        unsafe {
            thread::set_specific(
                THREAD_TABLE_OS_SLOT,
                (table as *const ThreadTable).cast_mut().cast(),
            )
        };
        return table;
    }

    rtabort!("out of Cafe TLS thread tables (maximum 256 live threads)");
}

fn existing_thread_table() -> Option<&'static ThreadTable> {
    let table = unsafe { thread::get_specific(THREAD_TABLE_OS_SLOT).cast::<ThreadTable>() };
    if table.is_null() {
        None
    } else {
        Some(unsafe { &*table })
    }
}

/// Runs the destructors of the calling thread's values, like POSIX does at
/// thread exit. Destructors may store new values, so this repeats for a few
/// rounds. The table is reclaimed only after runtime thread cleanup.
fn run_dtors_for(only_index: Option<usize>) {
    let Some(table) = existing_thread_table() else {
        return;
    };

    for _ in 0..4 {
        let mut ran = false;
        for index in 1..KEY_COUNT {
            if only_index.is_some_and(|only| only != index) {
                continue;
            }
            let metadata = &KEYS[index];
            let state = metadata.state.load(Acquire);
            if state & 3 != KEY_LIVE {
                continue;
            }
            let generation = state >> 2;
            let dtor = metadata.dtor.load(Acquire);
            let cell = unsafe { (*table.values.get())[index] };
            let value = if cell.generation == generation {
                cell.value
            } else {
                ptr::null_mut()
            };
            if dtor.is_null() || value.is_null() {
                continue;
            }

            unsafe {
                (*table.values.get())[index] = TlsCell {
                    generation,
                    value: ptr::null_mut(),
                };
            }

            // A concurrent destroy/recreate invalidates this generation.
            // Do not dispatch a destructor belonging to a raced-away key.
            if key_is_live(index, generation)
                && metadata.dtor.load(Acquire) == dtor
            {
                unsafe { crate::mem::transmute::<*mut (), Dtor>(dtor)(value) };
            }
            ran = true;
        }
        if !ran {
            break;
        }
    }
}

pub fn run_dtors() {
    run_dtors_for(None);
}

/// Release the current thread's table after all std runtime cleanup has had a
/// chance to access its TLS. Values left after the four destructor rounds are
/// intentionally leaked, as in the existing Cafe backend.
pub fn release_thread_table() {
    let Some(table) = existing_thread_table() else {
        return;
    };

    // Do not let a later owner observe pointers from the previous thread.
    unsafe { *table.values.get() = [TlsCell::empty(); KEY_COUNT] };
    unsafe { thread::set_specific(THREAD_TABLE_OS_SLOT, ptr::null_mut()) };
    table.claimed.store(0, Release);
}

// A test-only C ABI hook lets the target probe exercise the production
// four-round loop with a raw key. `thread_local!` deliberately prevents a
// destructor from resurrecting its own LocalKey, so that case cannot be
// expressed through the public std API.
#[cfg(cafe_tls_probe)]
static TLS_PROBE_KEY: AtomicUsize = AtomicUsize::new(0);
#[cfg(cafe_tls_probe)]
static TLS_PROBE_DTOR_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(cafe_tls_probe)]
static TLS_PROBE_VALUE: u8 = 0;

#[cfg(cafe_tls_probe)]
unsafe extern "C" fn tls_probe_self_resurrect(value: *mut u8) {
    let calls = TLS_PROBE_DTOR_CALLS.fetch_add(1, Relaxed) + 1;
    if calls < 4 {
        let key = TLS_PROBE_KEY.load(Acquire);
        unsafe { set(key, value) };
    }
}

#[cfg(cafe_tls_probe)]
#[unsafe(no_mangle)]
pub extern "C" fn horizon_cafe_tls_self_rearm_probe() -> u32 {
    TLS_PROBE_DTOR_CALLS.store(0, Relaxed);
    let key = create(Some(tls_probe_self_resurrect));
    TLS_PROBE_KEY.store(key, Release);
    unsafe { set(key, &raw const TLS_PROBE_VALUE as *mut u8) };
    let Some((index, _)) = decode_key(key) else {
        return 0;
    };
    run_dtors_for(Some(index));
    let calls = TLS_PROBE_DTOR_CALLS.load(Acquire) as u32;
    unsafe {
        set(key, ptr::null_mut());
        destroy(key);
    }
    calls
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    let Some((index, generation)) = decode_key(key) else {
        return;
    };
    if !key_is_live(index, generation) {
        return;
    }
    let table = thread_table();
    unsafe { (*table.values.get())[index] = TlsCell { generation, value } };
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    let Some((index, generation)) = decode_key(key) else {
        return ptr::null_mut();
    };
    if !key_is_live(index, generation) {
        return ptr::null_mut();
    }
    let Some(table) = existing_thread_table() else {
        return ptr::null_mut();
    };
    let cell = unsafe { (*table.values.get())[index] };
    if cell.generation == generation {
        cell.value
    } else {
        ptr::null_mut()
    }
}

#[inline]
pub unsafe fn destroy(key: Key) {
    let Some((index, generation)) = decode_key(key) else {
        return;
    };
    let metadata = &KEYS[index];
    if metadata
        .state
        .compare_exchange(
            key_state(generation, KEY_LIVE),
            key_state(generation, KEY_INITIALIZING),
            Acquire,
            Relaxed,
        )
            .is_err()
    {
        return;
    }

    // Invalidate the dtor before making this index available for reuse. Each
    // thread's cell carries the generation, so an old value cannot leak into
    // a newly allocated key even if a set raced with this destroy.
    metadata.dtor.store(ptr::null_mut(), Release);
    metadata.state.store(key_state(generation, KEY_FREE), Release);
}
