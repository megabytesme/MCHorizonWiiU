//! Safe owner for the native Cafe AX audio bridge.
//!
//! AX owns its callback and DSP buffers until `horizon_wiiu_audio_close` returns.
//! The Rust renderer uses this wrapper to submit blocks from its mixer worker;
//! the AX callback itself only drains the bridge's fixed SPSC queue.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" {
    fn horizon_wiiu_audio_open(frames: *mut u32, rate: *mut u32) -> *mut std::ffi::c_void;
    fn horizon_wiiu_audio_submit(handle: *mut std::ffi::c_void, stereo: *const f32, frames: u32);
    fn horizon_wiiu_audio_close(handle: *mut std::ffi::c_void);
}

/// An initialized AX stream with a bounded native SPSC queue.
pub struct CafeAudio {
    handle: NonNull<std::ffi::c_void>,
    frames: u32,
    rate: u32,
    submit_active: AtomicBool,
}

// SAFETY: the native handle is owned here and Drop closes it after all Arc
// owners have released it.
unsafe impl Send for CafeAudio {}
// SAFETY: `submit_active` serializes safe calls into the native SPSC producer;
// `frames` and `rate` are immutable after construction.
unsafe impl Sync for CafeAudio {}

impl CafeAudio {
    /// Initialize AX and acquire the bounded native output bridge.
    pub fn open() -> Result<Self, &'static str> {
        let mut frames = 0;
        let mut rate = 0;
        // SAFETY: the bridge initializes AX and returns an owned handle, or null.
        let Some(handle) = NonNull::new(unsafe { horizon_wiiu_audio_open(&mut frames, &mut rate) })
        else {
            return Err("Wii U AX audio initialization failed");
        };
        if frames == 0 || frames > 256 || rate == 0 {
            // SAFETY: the returned handle is owned and has no producer yet.
            unsafe { horizon_wiiu_audio_close(handle.as_ptr()) };
            return Err("Wii U AX reported an unsupported audio frame format");
        }
        Ok(Self {
            handle,
            frames,
            rate,
            submit_active: AtomicBool::new(false),
        })
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Submit one interleaved stereo f32 block. The native bridge copies it
    /// synchronously into its fixed queue, so the caller retains the slice.
    pub fn submit(&self, stereo: &[f32]) {
        if stereo.len() != self.frames as usize * 2 {
            return;
        }
        if self
            .submit_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        // SAFETY: the handle remains open for this borrow; C copies exactly one
        // frame before returning and does not retain the sample pointer.
        unsafe {
            horizon_wiiu_audio_submit(self.handle.as_ptr(), stereo.as_ptr(), self.frames);
        }
        self.submit_active.store(false, Ordering::Release);
    }
}

impl Drop for CafeAudio {
    fn drop(&mut self) {
        // SAFETY: NonNull came from a successful open; Arc keeps this owner alive
        // until the mixer worker has stopped using it.
        unsafe { horizon_wiiu_audio_close(self.handle.as_ptr()) };
    }
}
