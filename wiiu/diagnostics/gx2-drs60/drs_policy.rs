#![allow(dead_code)]

mod dynamic_resolution;

mod bridge {
    use super::dynamic_resolution::{DynamicResolution, Pace};
    use std::time::Duration;

    include!("cafe_drs_exact.rs");

    const FLOOR: u32 = 50;
    const TARGET_FPS: u32 = 60;
    static mut CONTROLLER: Option<DynamicResolution> = None;

    #[repr(C)]
    struct Frame {
        sample: CafeGpuSample,
        frame_ns: u64,
        submission_ns: u64,
        wait_ns: u64,
        drawn: [u32; 2],
        scale: u32,
        outputs: u32,
    }

    impl Frame {
        fn timing_ms(&self, target: u32) -> Option<f32> {
            CafeFrameWork {
                presented: [self.outputs & 1 != 0, self.outputs & 2 != 0],
                submission: Duration::from_nanos(self.submission_ns),
                gpu_wait: Duration::from_nanos(self.wait_ns),
                gpu_sample: Some(self.sample),
                terrain_sections: self.drawn.map(|draws| draws as usize),
                world_resolution: Some(WorldResolution {
                    requested_scale: self.scale,
                    effective_scale: self.scale,
                    dimensions: [[0; 2]; 2],
                    surface_bytes: 0,
                }),
                ..Default::default()
            }
            .drs_pipeline_ms(Duration::from_nanos(self.frame_ns), target)
        }
    }

    fn feed(controller: &mut DynamicResolution, frame: &Frame) -> u32 {
        if let Some(pipeline_ms) = frame.timing_ms(TARGET_FPS) {
            let frame_ms = Duration::from_nanos(frame.frame_ns).as_secs_f32() * 1000.0;
            controller.observe(Some(pipeline_ms), frame_ms, FLOOR, TARGET_FPS, Pace::Gentle);
        } else {
            controller.discard_measurements();
        }
        controller.scale(FLOOR)
    }

    // The fixture calls these only from its GX2/render thread. It owns Frame storage.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn drs_timing(frame: *const Frame, target: u32) -> f32 {
        unsafe { &*frame }.timing_ms(target).unwrap_or(-1.0)
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn drs_feed(frame: *const Frame) -> u32 {
        let controller = unsafe { &mut *std::ptr::addr_of_mut!(CONTROLLER) };
        feed(
            controller.get_or_insert_with(DynamicResolution::new),
            unsafe { &*frame },
        )
    }

    fn check_transitions(light: &Frame, heavy: &Frame) -> bool {
        let mut controller = DynamicResolution::new();
        for (frame, target) in [(light, 60), (heavy, 50), (light, 60)] {
            if !(0..500).any(|_| feed(&mut controller, frame) == target) {
                return false;
            }
        }
        true
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn drs_self_check(light: *const Frame, heavy: *const Frame) -> bool {
        check_transitions(unsafe { &*light }, unsafe { &*heavy })
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn drs_forget() {
        let controller = unsafe { &mut *std::ptr::addr_of_mut!(CONTROLLER) };
        *controller = None;
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn frame(scale: u32, ticks: u64, wait_ns: u64) -> Frame {
            Frame {
                sample: CafeGpuSample {
                    endpoints: [[1000, 1000 + ticks], [1000 + ticks, 1000 + 2 * ticks]],
                    submitted: 10,
                    retired: 10,
                    status: 1,
                    ..Default::default()
                },
                frame_ns: 24_000_000,
                submission_ns: 23_000_000,
                wait_ns,
                drawn: [2, 2],
                scale,
                outputs: 3,
            }
        }

        #[test]
        fn target_60_light_heavy_light_windows() {
            assert!(check_transitions(
                &frame(FLOOR, 108_000, 18_000_000),
                &frame(60, 297_000, 18_000_000),
            ));
        }

        #[test]
        fn unretired_unpaired_and_cpu_limited_samples_hold_scale() {
            let mut controller = DynamicResolution::new();
            let mut result = FLOOR;
            for _ in 0..500 {
                let light = frame(FLOOR, 108_000, 2_000_000);
                assert_eq!(light.timing_ms(60), None);
                assert_eq!(light.timing_ms(30), Some(8.0));
                result = feed(&mut controller, &light);
            }
            assert_eq!(result, FLOOR);
            let mut light = frame(FLOOR, 108_000, 18_000_000);
            light.outputs = 1;
            assert_eq!(light.timing_ms(60), None);
            light.outputs = 3;
            light.sample.retired -= 1;
            assert_eq!(light.timing_ms(60), None);
        }
    }
}

#[cfg(target_os = "cafe")]
mod allocation {
    use std::alloc::{GlobalAlloc, Layout};

    unsafe extern "C" {
        fn drs_alloc(size: usize, align: usize) -> *mut u8;
        fn drs_free(ptr: *mut u8);
    }

    struct CafeHeap;

    unsafe impl GlobalAlloc for CafeHeap {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { drs_alloc(layout.size(), layout.align()) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            unsafe { drs_free(ptr) }
        }
    }

    #[global_allocator]
    static HEAP: CafeHeap = CafeHeap;
}
