extern crate self as horizon_client;

pub mod presentation {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct WorldResolution {
        pub requested_scale: u32,
        pub effective_scale: u32,
        pub dimensions: [[u32; 2]; 2],
        pub surface_bytes: u32,
    }
}

#[path = "source/crates/horizon-client-render/src/dynamic_resolution.rs"]
mod dynamic_resolution;
#[path = "source/wiiu/loading_host/world_scale.rs"]
mod world_scale;

use std::{
    ffi::c_void,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

static SERIAL: Mutex<()> = Mutex::new(());
static CREATES: AtomicUsize = AtomicUsize::new(0);
static DESTROYS: AtomicUsize = AtomicUsize::new(0);
static COMPOSITES: AtomicUsize = AtomicUsize::new(0);
static RESTORES: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
unsafe extern "C" fn horizon_gx2_world_scale_create(
    dimensions: *const u32,
    surface_bytes: *mut u32,
) -> *mut c_void {
    CREATES.fetch_add(1, SeqCst);
    if FAIL.load(SeqCst) {
        return std::ptr::null_mut();
    }
    let dimensions = unsafe { std::slice::from_raw_parts(dimensions, 4) };
    unsafe { *surface_bytes = 8 * (dimensions[0] * dimensions[1] + dimensions[2] * dimensions[3]) };
    Box::into_raw(Box::new(0u32)).cast()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn horizon_gx2_world_scale_destroy(state: *mut c_void) {
    DESTROYS.fetch_add(1, SeqCst);
    unsafe { drop(Box::from_raw(state.cast::<u32>())) };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn horizon_gx2_world_scale_begin(
    state: *mut c_void,
    target: u32,
    clear: *const f32,
) {
    assert!(!state.is_null() && target < 2 && !clear.is_null());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn horizon_gx2_world_scale_end(
    state: *mut c_void,
    target: u32,
    viewport: *const u32,
    composite: bool,
) {
    assert!(!state.is_null() && target < 2 && !viewport.is_null());
    RESTORES.fetch_add(1, SeqCst);
    if composite {
        COMPOSITES.fetch_add(1, SeqCst);
    }
}

#[test]
fn native_scale_preserves_native_hud_extent_and_retires_surfaces() {
    let _serial = SERIAL.lock().unwrap();
    for counter in [&CREATES, &DESTROYS, &COMPOSITES, &RESTORES] {
        counter.store(0, SeqCst);
    }
    let viewports = [[1, 2, 1280, 719], [0, 0, 854, 480]];
    let clear = [0.0, 0.0, 0.0, 1.0];
    let mut scale = world_scale::WorldScale::default();
    scale.prepare(100, viewports, false);
    assert!(scale.bind(0, viewports[0], &clear).is_none());
    assert_eq!(CREATES.load(SeqCst), 0);
    assert_eq!(
        scale.resolution().unwrap().dimensions,
        [[1280, 719], [854, 480]]
    );

    scale.prepare(75, viewports, false);
    let unqualified = scale.resolution().unwrap();
    assert_eq!(unqualified.requested_scale, 75);
    assert_eq!(unqualified.effective_scale, 100);
    assert_eq!(unqualified.dimensions, [[1280, 719], [854, 480]]);
    assert_eq!(unqualified.surface_bytes, 0);
    assert!(scale.bind(0, viewports[0], &clear).is_none());
    assert_eq!(CREATES.load(SeqCst), 0);

    scale.prepare(75, viewports, true);
    let active = scale.resolution().unwrap();
    assert_eq!(active.dimensions, [[960, 539], [641, 360]]);
    assert_eq!(active.effective_scale, 75);
    assert_ne!(active.surface_bytes, 0);
    let binding = scale.bind(0, viewports[0], &clear).unwrap();
    assert_eq!(binding.size, [960, 539]);
    binding.composite();
    drop(scale.bind(1, viewports[1], &clear));
    assert_eq!(RESTORES.load(SeqCst), 2);
    assert_eq!(COMPOSITES.load(SeqCst), 1);

    scale.prepare(75, viewports, true);
    assert_eq!(CREATES.load(SeqCst), 1);
    assert_eq!(DESTROYS.load(SeqCst), 0);

    FAIL.store(true, SeqCst);
    scale.prepare(50, viewports, true);
    let fallback = scale.resolution().unwrap();
    assert_eq!(fallback.requested_scale, 50);
    assert_eq!(fallback.effective_scale, 100);
    assert_eq!(fallback.dimensions, [[1280, 719], [854, 480]]);
    assert_eq!(fallback.surface_bytes, 0);
    assert_eq!(DESTROYS.load(SeqCst), 1);
    scale.prepare(50, viewports, true);
    assert_eq!(CREATES.load(SeqCst), 2);

    scale.release();
    FAIL.store(false, SeqCst);
    scale.prepare(50, viewports, true);
    assert_eq!(
        scale.resolution().unwrap().dimensions,
        [[640, 360], [427, 240]]
    );
    drop(scale);
    assert_eq!(DESTROYS.load(SeqCst), 2);
}

#[test]
fn synthetic_gpu_samples_drive_native_target_requests() {
    let _serial = SERIAL.lock().unwrap();
    for target_fps in [30, 60] {
        check_target_fps(target_fps);
    }
}

fn check_target_fps(target_fps: u32) {
    use dynamic_resolution::{DynamicResolution, Pace};

    let mut controller = DynamicResolution::new();
    let mut targets = world_scale::WorldScale::default();
    let viewports = [[0, 0, 1280, 720], [0, 0, 854, 480]];
    let floor = 75;
    targets.prepare(controller.scale(floor), viewports, true);

    for _ in 0..100 {
        assert_eq!(
            controller.observe(None, 100.0, floor, target_fps, Pace::Quick),
            None
        );
    }
    assert_eq!(controller.scale(floor), 75);
    assert_eq!(
        targets.resolution().unwrap().dimensions,
        [[960, 540], [641, 360]]
    );

    let mut changes = Vec::new();
    for _ in 0..1000 {
        if let Some(scale) = controller.observe(Some(8.0), 16.0, floor, target_fps, Pace::Quick) {
            targets.prepare(scale, viewports, true);
            changes.push(targets.resolution().unwrap().effective_scale);
        }
    }
    assert_eq!(changes, [80, 85, 90, 95, 100]);
    assert_eq!(targets.resolution().unwrap().surface_bytes, 0);

    for _ in 0..240 {
        assert_eq!(
            controller.observe(Some(8.0), 100.0, floor, target_fps, Pace::Quick),
            None,
        );
    }
    assert_eq!(controller.scale(floor), 100);

    changes.clear();
    let (gpu_ms, frame_ms) = if target_fps == 30 {
        (50.0, 60.0)
    } else {
        (25.0, 33.0)
    };
    for _ in 0..500 {
        if let Some(scale) =
            controller.observe(Some(gpu_ms), frame_ms, floor, target_fps, Pace::Quick)
        {
            targets.prepare(scale, viewports, true);
            changes.push(targets.resolution().unwrap().effective_scale);
        }
    }
    assert_eq!(changes, [95, 90, 85, 80, 75]);
    assert_eq!(
        targets.resolution().unwrap().dimensions,
        [[960, 540], [641, 360]]
    );
}
