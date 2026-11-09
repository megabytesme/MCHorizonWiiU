//! Native GX2 presentation adapter for Horizon's existing Cafe loading session.
//!
//! The executable host owns WUT/GX2 and calls `CafeLoadingSession::draw` with
//! this presenter while the TV or DRC colour target is bound. This module owns
//! only copies of the HUD atlas and panorama; the original `HudBatch`, faces,
//! and frame are borrowed synchronously from Horizon.

#![cfg(all(target_os = "cafe", target_arch = "powerpc"))]

use horizon_client::{
    game::{CafeLoadingArtwork, CafeLoadingPresenter},
    gfx::{panorama, Frame},
};
use horizon_wiiu_platform::hud::horizon::HorizonPresenter;
use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PanoramaVertex {
    position: [f32; 4],
    direction: [f32; 3],
}

const _: () = {
    assert!(std::mem::size_of::<PanoramaVertex>() == 28);
    assert!(std::mem::offset_of!(PanoramaVertex, direction) == 16);
};

extern "C" {
    fn OSReport(format: *const std::ffi::c_char, ...);
    fn horizon_loading_host_dimensions(dimensions: *mut u32) -> i32;
    fn horizon_loading_host_hud_gfd(length: *mut usize) -> *const u8;
    fn horizon_loading_host_panorama_gfd(length: *mut usize) -> *const u8;
    fn horizon_loading_host_present(
        draw: unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> i32,
        context: *mut c_void,
    ) -> i32;
    fn horizon_gx2_panorama_create(
        out: *mut *mut c_void,
        gfd: *const c_void,
        size: u32,
        rgba: *const u8,
        length: usize,
    ) -> i32;
    fn horizon_gx2_panorama_draw_to(
        panorama: *mut c_void,
        target_width: u32,
        target_height: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        vertices: *const PanoramaVertex,
    ) -> i32;
    fn horizon_gx2_panorama_destroy(panorama: *mut c_void);
}

macro_rules! gx2_loading_stage {
    ($stage:literal) => {{
        // Static OSReport strings stay allocation-free and cannot unwind
        // through the synchronous GX2 draw callback.
        unsafe {
            OSReport(
                concat!("Horizon GX2 loading: ", $stage, "\n\0")
                    .as_ptr()
                    .cast(),
            );
        }
    }};
}

fn output_dimensions() -> Result<[[u32; 2]; 2], &'static str> {
    let mut dimensions = [0u32; 4];
    // SAFETY: output is a valid four-element buffer; GX2 has already been
    // initialized as required by the presenter constructor.
    if unsafe { horizon_loading_host_dimensions(dimensions.as_mut_ptr()) } != 0 {
        return Err("WHB TV/DRC colour buffers are unavailable");
    }
    let outputs = [
        [dimensions[0], dimensions[1]],
        [dimensions[2], dimensions[3]],
    ];
    if outputs.iter().any(|size| size.contains(&0)) {
        return Err("WHB returned an empty TV/DRC colour buffer");
    }
    Ok(outputs)
}

fn fit_viewport(source: [u32; 2], output: [u32; 2]) -> Option<[u32; 4]> {
    let [source_width, source_height] = source;
    let [output_width, output_height] = output;
    if source_width == 0 || source_height == 0 || output_width == 0 || output_height == 0 {
        return None;
    }
    let (width, height) = if u64::from(output_width) * u64::from(source_height)
        <= u64::from(output_height) * u64::from(source_width)
    {
        let height = (u64::from(output_width) * u64::from(source_height)
            + u64::from(source_width) / 2)
            / u64::from(source_width);
        (output_width, height.min(u64::from(output_height)) as u32)
    } else {
        let width = (u64::from(output_height) * u64::from(source_width)
            + u64::from(source_height) / 2)
            / u64::from(source_height);
        (width.min(u64::from(output_width)) as u32, output_height)
    };
    if width == 0 || height == 0 {
        return None;
    }
    Some([
        (output_width - width) / 2,
        (output_height - height) / 2,
        width,
        height,
    ])
}

fn hud_error_message(error: horizon_wiiu_platform::hud::Error) -> &'static str {
    match error {
        horizon_wiiu_platform::hud::Error::Unsupported => {
            "Horizon HUD batch uses a GX2-unsupported blend or viewport"
        }
        horizon_wiiu_platform::hud::Error::Invalid => "Horizon HUD batch is invalid",
        horizon_wiiu_platform::hud::Error::AllocationFailed => {
            "GX2 HUD presenter allocation failed"
        }
        horizon_wiiu_platform::hud::Error::Backend(_) => "GX2 HUD submission failed",
    }
}

struct Panorama<'shader> {
    native: NonNull<c_void>,
    _shader: PhantomData<&'shader [u8]>,
    _thread: PhantomData<*mut ()>,
}

impl Drop for Panorama<'_> {
    fn drop(&mut self) {
        // SAFETY: this owner is render-thread-bound and the C destructor
        // drains GX2 before releasing the copied texture and shader resources.
        unsafe { horizon_gx2_panorama_destroy(self.native.as_ptr()) }
    }
}

impl<'shader> Panorama<'shader> {
    unsafe fn new(gfd: &'shader [u8], faces: &panorama::Faces) -> Result<Self, &'static str> {
        let rgba = faces.texels();
        let expected = (faces.size as usize)
            .checked_mul(faces.size as usize)
            .and_then(|n| n.checked_mul(24))
            .ok_or("Horizon panorama dimensions overflow")?;
        if rgba.len() != expected {
            return Err("Horizon panorama texel length does not match its face size");
        }
        let mut out = std::ptr::null_mut();
        // SAFETY: C copies all face pixels before this call returns. The GFD
        // borrow is retained by `Self` until the native shader is destroyed.
        let code = unsafe {
            horizon_gx2_panorama_create(
                &mut out,
                gfd.as_ptr().cast(),
                faces.size,
                rgba.as_ptr(),
                rgba.len(),
            )
        };
        if code != 0 {
            return Err("GX2 panorama resource creation failed");
        }
        Ok(Self {
            native: NonNull::new(out).ok_or("GX2 panorama returned a null resource")?,
            _shader: PhantomData,
            _thread: PhantomData,
        })
    }

    unsafe fn draw(
        &mut self,
        target_viewport: [u32; 2],
        rect: [u32; 4],
        yaw: f32,
        pitch: f32,
    ) -> Result<(), &'static str> {
        let [target_width, target_height] = target_viewport;
        let [x, y, width, height] = rect;
        let vertices = panorama_vertices(width, height, yaw, pitch)?;
        // SAFETY: GX2 target is bound by the host; this only borrows the local
        // vertex array for the synchronous C draw call.
        let code = unsafe {
            horizon_gx2_panorama_draw_to(
                self.native.as_ptr(),
                target_width,
                target_height,
                x,
                y,
                width,
                height,
                vertices.as_ptr(),
            )
        };
        if code == 0 {
            Ok(())
        } else {
            Err("GX2 panorama draw failed")
        }
    }
}

fn panorama_vertices(
    width: u32,
    height: u32,
    yaw: f32,
    pitch: f32,
) -> Result<[PanoramaVertex; 36], &'static str> {
    if width == 0 || height == 0 {
        return Err("GX2 panorama viewport is empty");
    }
    // Same position-only cube and winding as gfx/panorama.wgsl. Each output
    // receives its own projection because TV and DRC viewports may differ.
    const CORNERS: [[f32; 3]; 8] = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    const INDICES: [usize; 36] = [
        4, 7, 6, 4, 6, 5, 5, 6, 2, 5, 2, 1, 1, 2, 3, 1, 3, 0, 0, 3, 7, 0, 7, 4, 7, 3, 2, 7, 2, 6,
        0, 4, 5, 0, 5, 1,
    ];
    let matrix = panorama::projection(width, height, yaw, pitch);
    let columns = matrix.to_cols_array_2d();
    Ok(std::array::from_fn(|i| {
        let direction = CORNERS[INDICES[i]];
        let [x, y, z] = direction;
        let position = std::array::from_fn(|axis| {
            columns[0][axis] * x + columns[1][axis] * y + columns[2][axis] * z + columns[3][axis]
        });
        PanoramaVertex {
            position,
            direction,
        }
    }))
}

/// Uses the actual DRC size as Horizon's original source layout and fits that
/// layout inside each queried TV/DRC buffer.
pub struct Gx2LoadingPresenter<'shader> {
    hud_shader: &'shader [u8],
    panorama_shader: &'shader [u8],
    hud: Option<HorizonPresenter<'shader>>,
    panorama: Option<Panorama<'shader>>,
    generation: Option<u64>,
    atlas_revision: Option<u64>,
    panorama_identity: Option<(u64, u64, usize, u32)>,
    layout: [u32; 2],
    outputs: [[u32; 2]; 2],
    capacity: u32,
    trace_first_prepare: bool,
    trace_first_present: bool,
    trace_first_target: [bool; 2],
    _thread: PhantomData<*mut ()>,
}

impl<'shader> Gx2LoadingPresenter<'shader> {
    /// # Safety
    /// GX2 must already be initialized on this thread and remain active until drop.
    /// Both slices must contain complete GFD shader groups that outlive this value.
    pub unsafe fn new(
        hud_shader: &'shader [u8],
        panorama_shader: &'shader [u8],
        capacity: u32,
    ) -> Result<Self, &'static str> {
        if capacity == 0 {
            return Err("GX2 loading presenter capacity is empty");
        }
        let outputs = output_dimensions()?;
        Ok(Self {
            hud_shader,
            panorama_shader,
            hud: None,
            panorama: None,
            generation: None,
            atlas_revision: None,
            panorama_identity: None,
            layout: [outputs[1][0], outputs[1][1]],
            outputs,
            capacity,
            trace_first_prepare: false,
            trace_first_present: false,
            trace_first_target: [false; 2],
            _thread: PhantomData,
        })
    }

    /// Use the GFD blobs embedded by this crate's CMake support target.
    ///
    /// # Safety
    /// GX2 must already be initialized and remain active on this thread until
    /// the returned presenter is dropped.
    pub unsafe fn new_embedded(
        capacity: u32,
    ) -> Result<Gx2LoadingPresenter<'static>, &'static str> {
        let mut hud_length = 0usize;
        let mut panorama_length = 0usize;
        let hud = unsafe { horizon_loading_host_hud_gfd(&mut hud_length) };
        let panorama = unsafe { horizon_loading_host_panorama_gfd(&mut panorama_length) };
        if hud.is_null() || panorama.is_null() || hud_length == 0 || panorama_length == 0 {
            return Err("GX2 loading GFD shaders were not embedded");
        }
        // SAFETY: CMake links these static binary objects into the Cafe image;
        // their addresses remain valid until process shutdown.
        let hud = unsafe { std::slice::from_raw_parts(hud, hud_length) };
        let panorama = unsafe { std::slice::from_raw_parts(panorama, panorama_length) };
        unsafe { Gx2LoadingPresenter::new(hud, panorama, capacity) }
    }

    fn prepare(&mut self, artwork: CafeLoadingArtwork<'_>) -> Result<(), &'static str> {
        self.outputs = output_dimensions()?;
        self.layout = self.outputs[1];
        if self
            .target_viewports()
            .iter()
            .any(|r| r[2] == 0 || r[3] == 0)
        {
            return Err("TV or DRC cannot fit the GamePad layout aspect ratio".into());
        }
        let atlas = artwork.atlas;
        if atlas.width == 0 || atlas.height == 0 {
            return Err("Horizon HUD atlas is empty");
        }
        let new_hud = self.generation != Some(artwork.generation)
            || self.atlas_revision != Some(atlas.revision)
            || self.hud.is_none();
        if new_hud {
            // Create before layout: HorizonPresenter resets the original font
            // and UI-theme caches so this frame's new batch has valid glyph
            // and transient UI-texture UVs.
            horizon_client::ui_theme::reset_atlas();
            // SAFETY: constructor's GX2 and shader lifetime requirements are upheld.
            self.hud = None;
            self.hud = Some(
                unsafe { HorizonPresenter::new(self.hud_shader, atlas, self.capacity) }
                    .map_err(|_| "GX2 HUD presenter creation failed")?,
            );
            self.generation = Some(artwork.generation);
            self.atlas_revision = Some(atlas.revision);
        }
        let identity = artwork.panorama.map(|faces| {
            (
                artwork.generation,
                atlas.revision,
                faces as *const _ as usize,
                faces.size,
            )
        });
        if identity != self.panorama_identity {
            self.panorama = None;
            self.panorama = match artwork.panorama {
                Some(faces) => Some(unsafe { Panorama::new(self.panorama_shader, faces) }?),
                None => None,
            };
            self.panorama_identity = identity;
        }
        Ok(())
    }
}

impl CafeLoadingPresenter for Gx2LoadingPresenter<'_> {
    fn layout_viewport(&self) -> [u32; 2] {
        self.layout
    }

    fn target_viewports(&self) -> [[u32; 4]; 2] {
        self.outputs
            .map(|[w, h]| fit_viewport(self.layout, [w, h]).unwrap_or([0; 4]))
    }

    fn prepare_loading(&mut self, artwork: CafeLoadingArtwork<'_>) -> horizon_client::Result<()> {
        let trace = !self.trace_first_prepare;
        if trace {
            self.trace_first_prepare = true;
            gx2_loading_stage!("first presenter prepare begin");
        }
        let result = self.prepare(artwork).map_err(Into::into);
        if trace {
            if result.is_ok() {
                gx2_loading_stage!("first presenter prepare complete");
            } else {
                gx2_loading_stage!("first presenter prepare failed");
            }
        }
        result
    }

    fn present_loading(
        &mut self,
        artwork: CafeLoadingArtwork<'_>,
        frame: &Frame<'_>,
        layout_viewport: [u32; 2],
        target_viewports: [[u32; 4]; 2],
        tick: u64,
        animated_textures: bool,
    ) -> horizon_client::Result<bool> {
        let trace = !self.trace_first_present;
        if trace {
            self.trace_first_present = true;
            gx2_loading_stage!("first presenter present begin");
        }
        let expected_panorama = artwork.panorama.map(|faces| {
            (
                artwork.generation,
                artwork.atlas.revision,
                faces as *const _ as usize,
                faces.size,
            )
        });
        if self.generation != Some(artwork.generation)
            || self.atlas_revision != Some(artwork.atlas.revision)
            || self.hud.is_none()
            || self.panorama_identity != expected_panorama
        {
            return Err("Cafe loading resources were not prepared before layout".into());
        }
        if layout_viewport != self.layout || target_viewports != self.target_viewports() {
            return Err("Cafe loading viewports changed without presenter recreation".into());
        }
        struct Draw<'a, 'frame, 'shader> {
            presenter: &'a mut Gx2LoadingPresenter<'shader>,
            atlas: &'a horizon_client::hud::HudAtlas,
            frame: &'a Frame<'frame>,
            targets: [[u32; 4]; 2],
            tick: u64,
            animated: bool,
            result: Result<(), &'static str>,
        }
        unsafe extern "C" fn draw_target(
            context: *mut c_void,
            target: u32,
            width: u32,
            height: u32,
        ) -> i32 {
            let draw = unsafe { &mut *context.cast::<Draw<'_, '_, '_>>() };
            if draw.result.is_err()
                || target > 1
                || draw.presenter.outputs[target as usize] != [width, height]
            {
                draw.result = Err("Cafe host supplied an unexpected GX2 target");
                return -1;
            }
            let target_index = target as usize;
            let trace_target = !draw.presenter.trace_first_target[target_index];
            if trace_target {
                draw.presenter.trace_first_target[target_index] = true;
                if target == 0 {
                    gx2_loading_stage!("first TV target callback begin");
                } else {
                    gx2_loading_stage!("first DRC target callback begin");
                }
            }
            if let Some(panorama) = draw.presenter.panorama.as_mut() {
                if let Err(error) = unsafe {
                    panorama.draw(
                        [width, height],
                        draw.targets[target as usize],
                        draw.frame.yaw,
                        draw.frame.pitch,
                    )
                } {
                    draw.result = Err(error);
                    return -1;
                }
            }
            let target_rect = draw.targets[target as usize];
            let Some(hud) = draw.presenter.hud.as_mut() else {
                draw.result = Err("GX2 HUD presenter was not prepared");
                return -1;
            };
            // SAFETY: C called this callback after binding the target whose full
            // dimensions it supplied; layout and batch are borrowed synchronously.
            let hud_result = unsafe {
                hud.draw_to(
                    draw.atlas,
                    draw.frame.hud,
                    draw.presenter.layout,
                    target_rect,
                    draw.tick,
                    draw.animated,
                )
            };
            if let Err(error) = hud_result {
                draw.result = Err(hud_error_message(error));
                return -1;
            }
            if trace_target {
                if target == 0 {
                    gx2_loading_stage!("first TV target callback complete");
                } else {
                    gx2_loading_stage!("first DRC target callback complete");
                }
            }
            0
        }

        let mut draw = Draw {
            presenter: self,
            atlas: artwork.atlas,
            frame,
            targets: target_viewports,
            tick,
            animated: animated_textures,
            result: Ok(()),
        };
        // SAFETY: the C helper calls the callback synchronously on this same
        // render thread, binds and finishes TV and DRC independently, then returns.
        if trace {
            gx2_loading_stage!("first native TV/DRC present begin");
        }
        let host_result = unsafe {
            horizon_loading_host_present(draw_target, (&mut draw as *mut Draw<'_, '_, '_>).cast())
        };
        if trace {
            gx2_loading_stage!("first native TV/DRC present returned");
        }
        if draw.result.is_err() || host_result != 0 {
            // The next prepare callback runs before layout and must rebuild
            // from Horizon state with both transient atlas caches reset.
            draw.presenter.hud = None;
            draw.presenter.generation = None;
            draw.presenter.atlas_revision = None;
            draw.presenter.panorama = None;
            draw.presenter.panorama_identity = None;
        }
        draw.result?;
        if host_result != 0 {
            return Err("WHB did not accept both TV and DRC loading frames".into());
        }
        if trace {
            gx2_loading_stage!("first presenter present complete");
        }
        Ok(true)
    }
}
