//! Opt-in GX2 integration diagnostic using Horizon's original boot HUD.

use horizon_client_hud::{HudAtlas, HudBatch};
use horizon_hud_data::{rgba, HudVertex};
use horizon_wiiu_platform::hud::{horizon::HorizonPresenter, Error};

pub struct Probe {
    atlas: HudAtlas,
    presenter: HorizonPresenter<'static>,
}

#[repr(C)]
pub struct Metrics {
    scale: f32,
    first_x: f32,
    first_y: f32,
    first_u: f32,
    first_v: f32,
    color: u32,
    sample: [u8; 4],
}

fn error_code(error: Error) -> i32 {
    match error {
        Error::Invalid => 1,
        Error::Unsupported => 2,
        Error::AllocationFailed => 3,
        Error::Backend(_) => 4,
    }
}

/// The GFD is executable-owned and lives until the probe is destroyed.
#[no_mangle]
pub unsafe extern "C" fn horizon_hud_probe_create(
    gfd: *const u8,
    gfd_len: usize,
    out: *mut *mut Probe,
) -> i32 {
    if gfd.is_null() || gfd_len == 0 || out.is_null() {
        return 1;
    }
    *out = std::ptr::null_mut();
    horizon_wiiu_entropy::link();
    let atlas = HudAtlas::boot();
    let shader: &'static [u8] = std::slice::from_raw_parts(gfd, gfd_len);
    match HorizonPresenter::new(shader, &atlas, 12) {
        Ok(presenter) => {
            *out = Box::into_raw(Box::new(Probe { atlas, presenter }));
            0
        }
        Err(error) => error_code(error),
    }
}

/// Draw real Horizon HUD quads into the color target bound by the caller.
#[no_mangle]
pub unsafe extern "C" fn horizon_hud_probe_draw(
    probe: *mut Probe,
    width: u32,
    height: u32,
    vertices: *mut HudVertex,
    metrics: *mut Metrics,
) -> i32 {
    let Some(probe) = probe.as_mut() else {
        return 1;
    };
    if width == 0 || height == 0 || vertices.is_null() || metrics.is_null() {
        return 1;
    }
    let mut batch = HudBatch::new([width, height]);
    let s = batch.scale;
    batch.fill(
        &probe.atlas,
        32.0 / s,
        32.0 / s,
        128.0 / s,
        128.0 / s,
        rgba(255, 0, 0, 255),
    );
    batch.fill(
        &probe.atlas,
        192.0 / s,
        32.0 / s,
        128.0 / s,
        128.0 / s,
        rgba(0, 255, 0, 255),
    );
    if batch.vertices.len() != 12 {
        return 1;
    }
    std::ptr::copy_nonoverlapping(batch.vertices.as_ptr(), vertices, 12);
    let first = &batch.vertices[0];
    let x = (first.uv[0] * probe.atlas.width as f32).floor() as usize;
    let y = (first.uv[1] * probe.atlas.height as f32).floor() as usize;
    let at = (y * probe.atlas.width as usize + x) * 4;
    let pixels = probe.atlas.pixels();
    let Some(sample) = pixels.get(at..at + 4) else {
        return 1;
    };
    *metrics = Metrics {
        scale: s,
        first_x: first.pos[0],
        first_y: first.pos[1],
        first_u: first.uv[0],
        first_v: first.uv[1],
        color: first.color,
        sample: sample.try_into().unwrap(),
    };
    match probe
        .presenter
        .draw(&probe.atlas, &batch, [width, height], 0, true)
    {
        Ok(()) => 0,
        Err(error) => error_code(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn horizon_hud_probe_destroy(probe: *mut Probe) {
    if !probe.is_null() {
        drop(Box::from_raw(probe));
    }
}
