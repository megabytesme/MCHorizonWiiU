use cafe_sys::coreinit::core::get_core_count;
use cafe_sys::coreinit::thread::{self, ThreadAttributes};
use cafe_sys::ffi;

use crate::alloc::Layout;
use crate::ffi::{CStr, c_void};
use crate::num::NonZero;
// use crate::sync::Mutex;
use crate::sys::pal::ticks::duration_to_ticks;
use crate::thread::ThreadInit;
use crate::time::Duration;
use crate::{io, ptr};

pub fn sleep(dur: Duration) {
    unsafe { thread::sleep_ticks(duration_to_ticks(dur)) }
}

pub fn yield_now() {
    unsafe { thread::yield_() }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    let cores = unsafe { get_core_count() } as usize;
    NonZero::new(cores).ok_or(io::Error::UNKNOWN_THREAD_COUNT)
}

pub const DEFAULT_MIN_STACK_SIZE: usize = 1024 * 1024;

const STACK_ALIGN: usize = 64;

pub struct Thread {
    os: *mut thread::Thread,
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let stack_size = stack.next_multiple_of(STACK_ALIGN);
        let stack_layout = Layout::from_size_align(stack_size, STACK_ALIGN).map_err(|_| {
            io::const_error!(io::ErrorKind::InvalidInput, "invalid thread stack size")
        })?;
        let thread_layout = Layout::new::<thread::Thread>();

        let stack = unsafe { crate::sys::alloc::alloc(stack_layout) };
        let thread = unsafe { crate::sys::alloc::alloc(thread_layout) }.cast::<thread::Thread>();

        if stack.is_null() || thread.is_null() {
            unsafe {
                if !stack.is_null() {
                    crate::sys::alloc::dealloc(stack, stack_layout);
                }
                if !thread.is_null() {
                    crate::sys::alloc::dealloc(thread.cast(), thread_layout);
                }
            }
            return Err(io::const_error!(io::ErrorKind::OutOfMemory, "unable to allocate thread"));
        }

        let data = Box::into_raw(init);
        let created = unsafe {
            thread::create(
                thread,
                Some(thread_start),
                0,
                data.cast(),
                stack.byte_add(stack_size).cast(),
                stack_size as u32,
                thread::get_priority(thread::get_current()),
                ThreadAttributes::AFFINITY_ANY,
            )
        };
        if created == ffi::FALSE {
            unsafe {
                drop(Box::from_raw(data));
                crate::sys::alloc::dealloc(stack, stack_layout);
                crate::sys::alloc::dealloc(thread.cast(), thread_layout);
            }
            return Err(io::const_error!(io::ErrorKind::Uncategorized, "unable to create thread"));
        }

        unsafe {
            thread::set_deallocator(thread, Some(free_thread));
            thread::resume(thread);
        }
        Ok(Thread { os: thread })
    }

    pub fn join(self) {
        let thread = self.os;
        crate::mem::forget(self);
        unsafe { thread::join(thread, ptr::null_mut()) };
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        unsafe { thread::detach(self.os) }
    }
}

/// Runs on an OS thread once the thread is released (joined, or detached and ended).
extern "C" fn free_thread(thread: *mut thread::Thread, stack_end: *mut c_void) {
    // The OS passes only the pointers, so the real layouts are unknown here.
    // The Cafe `dealloc` ignores the layout.
    let any = Layout::new::<()>();
    unsafe {
        let name = thread::get_name(thread);
        if !name.is_null() {
            crate::sys::alloc::dealloc(name.cast_mut().cast(), any);
        }
        crate::sys::alloc::dealloc(stack_end.cast(), any);
        crate::sys::alloc::dealloc(thread.cast(), any);
    }
}

extern "C" fn thread_start(_int_arg: i32, data: *mut c_void) -> i32 {
    let init = unsafe { Box::from_raw(data.cast::<ThreadInit>()) };
    let rust_start = init.init();
    rust_start();
    crate::sys::thread_local::key::run_dtors();
    crate::rt::thread_cleanup();
    0
}

/// std calls this on spawned threads only. `OSSetThreadName` keeps the pointer, so the name is copied; `free_thread` frees it.
pub fn set_name(name: &CStr) {
    let bytes = name.to_bytes_with_nul();
    let copy = unsafe { crate::sys::alloc::alloc(Layout::for_value(bytes)) };
    if copy.is_null() {
        return;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), copy, bytes.len());
        thread::set_name(thread::get_current(), copy.cast());
    }
}
