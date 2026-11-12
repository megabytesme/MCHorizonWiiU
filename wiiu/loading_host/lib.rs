//! Native GX2 presentation adapter for Horizon's existing Cafe loading session.
//!
//! The executable host owns WUT/GX2 and calls `CafeLoadingSession::draw` with
//! this presenter while the TV or DRC colour target is bound. This module owns
//! only copies of the HUD atlas and panorama; the original `HudBatch`, faces,
//! and frame are borrowed synchronously from Horizon.

#![cfg(all(target_os = "cafe", target_arch = "powerpc"))]

use horizon_client::chunk::ColumnLayout;
use horizon_client::gfx::SectionKey;
use horizon_client::graphics::PresentMode;
use horizon_client::hud::{cit_glint::GlintTextures, Rect};
use horizon_client::mesher::SectionMesh;
use horizon_client::sprites::SpriteAnimation;
use horizon_client::{
    game::{CafeGameArtwork, CafeGameBackend, CafeLoadingArtwork, CafeLoadingPresenter},
    gfx::{panorama, Frame},
    presentation::{HudBackend, PresentationBackend, TerrainBackend},
};
use horizon_wiiu_gx2_terrain::{expand_quads_into, TerrainAtlas, TerrainGpuVertex};
use horizon_wiiu_platform::hud::horizon::HorizonPresenter;
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    marker::PhantomData,
    ptr::NonNull,
};

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
    fn horizon_loading_host_terrain_gfd(length: *mut usize) -> *const u8;
    fn horizon_loading_host_present(
        draw: unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> i32,
        context: *mut c_void,
    ) -> i32;
    fn horizon_loading_host_present_with_clear(
        draw: unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> i32,
        context: *mut c_void,
        rgba: *const f32,
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
    fn horizon_gx2_terrain_create(
        out: *mut *mut c_void,
        gfd: *const c_void,
        width: u32,
        height: u32,
        rgba: *const u8,
        length: usize,
    ) -> i32;
    fn horizon_gx2_terrain_destroy(terrain: *mut c_void);
    fn horizon_gx2_terrain_section_create(
        out: *mut *mut c_void,
        vertices: *const TerrainGpuVertex,
        count: u32,
    ) -> i32;
    fn horizon_gx2_terrain_section_update(
        section: *mut c_void,
        vertices: *const TerrainGpuVertex,
        count: u32,
    ) -> i32;
    fn horizon_gx2_terrain_wait_idle();
    fn horizon_gx2_terrain_section_destroy(section: *mut c_void);
    fn horizon_gx2_terrain_draw(
        terrain: *mut c_void,
        section: *mut c_void,
        target_width: u32,
        target_height: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> i32;
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

struct TerrainSection {
    native: NonNull<c_void>,
    vertices: Vec<TerrainGpuVertex>,
}
impl Drop for TerrainSection {
    fn drop(&mut self) {
        unsafe { horizon_gx2_terrain_section_destroy(self.native.as_ptr()) }
    }
}

struct Terrain<'shader> {
    native: NonNull<c_void>,
    _shader: PhantomData<&'shader [u8]>,
    _thread: PhantomData<*mut ()>,
}
impl Drop for Terrain<'_> {
    fn drop(&mut self) {
        unsafe { horizon_gx2_terrain_destroy(self.native.as_ptr()) }
    }
}

/// Uses the actual DRC size as Horizon's original source layout and fits that
/// layout inside each queried TV/DRC buffer.
pub struct Gx2LoadingPresenter<'shader> {
    hud_shader: &'shader [u8],
    panorama_shader: &'shader [u8],
    terrain_shader: &'shader [u8],
    hud: Option<HorizonPresenter<'shader>>,
    panorama: Option<Panorama<'shader>>,
    terrain: Option<Terrain<'shader>>,
    terrain_atlas: Option<TerrainAtlas>,
    terrain_generation: Option<u64>,
    sections: HashMap<SectionKey, TerrainSection>,
    generation: Option<u64>,
    atlas_revision: Option<u64>,
    panorama_identity: Option<(u64, u64, usize, u32)>,
    layout: [u32; 2],
    outputs: [[u32; 2]; 2],
    capacity: u32,
    trace_first_prepare: bool,
    trace_first_present: bool,
    trace_first_target: [bool; 2],
    last_frame_partial: bool,
    unsupported_logged: u32,
    _thread: PhantomData<*mut ()>,
}

impl<'shader> Gx2LoadingPresenter<'shader> {
    /// # Safety
    /// GX2 must already be initialized on this thread and remain active until drop.
    /// Both slices must contain complete GFD shader groups that outlive this value.
    pub unsafe fn new(
        hud_shader: &'shader [u8],
        panorama_shader: &'shader [u8],
        terrain_shader: &'shader [u8],
        capacity: u32,
    ) -> Result<Self, &'static str> {
        if capacity == 0 {
            return Err("GX2 loading presenter capacity is empty");
        }
        let outputs = output_dimensions()?;
        Ok(Self {
            hud_shader,
            panorama_shader,
            terrain_shader,
            hud: None,
            panorama: None,
            terrain: None,
            terrain_atlas: None,
            terrain_generation: None,
            sections: HashMap::new(),
            generation: None,
            atlas_revision: None,
            panorama_identity: None,
            layout: [outputs[1][0], outputs[1][1]],
            outputs,
            capacity,
            trace_first_prepare: false,
            trace_first_present: false,
            trace_first_target: [false; 2],
            last_frame_partial: false,
            unsupported_logged: 0,
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
        let mut terrain_length = 0usize;
        let hud = unsafe { horizon_loading_host_hud_gfd(&mut hud_length) };
        let panorama = unsafe { horizon_loading_host_panorama_gfd(&mut panorama_length) };
        let terrain = unsafe { horizon_loading_host_terrain_gfd(&mut terrain_length) };
        if hud.is_null()
            || panorama.is_null()
            || terrain.is_null()
            || hud_length == 0
            || panorama_length == 0
            || terrain_length == 0
        {
            return Err("GX2 loading GFD shaders were not embedded");
        }
        // SAFETY: CMake links these static binary objects into the Cafe image;
        // their addresses remain valid until process shutdown.
        let hud = unsafe { std::slice::from_raw_parts(hud, hud_length) };
        let panorama = unsafe { std::slice::from_raw_parts(panorama, panorama_length) };
        let terrain = unsafe { std::slice::from_raw_parts(terrain, terrain_length) };
        unsafe { Gx2LoadingPresenter::new(hud, panorama, terrain, capacity) }
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
            if draw.result.is_err() {
                return -1;
            }
            if target > 1 {
                unsafe {
                    OSReport(
                        "Horizon GX2: invalid target index=%u size=%ux%u\n\0"
                            .as_ptr()
                            .cast(),
                        target,
                        width,
                        height,
                    );
                }
                draw.result = Err("Cafe host supplied an invalid GX2 target index");
                return -1;
            }
            let expected = draw.presenter.outputs[target as usize];
            if expected != [width, height] {
                unsafe {
                    OSReport(
                        "Horizon GX2: target=%u actual=%ux%u expected=%ux%u\n\0"
                            .as_ptr()
                            .cast(),
                        target,
                        width,
                        height,
                        expected[0],
                        expected[1],
                    );
                }
                draw.result = Err("Cafe host supplied a GX2 target with an unexpected size");
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
                if error == horizon_wiiu_platform::hud::Error::Unsupported {
                    let source = draw.presenter.layout;
                    let target = draw.targets[target_index];
                    let target_scaled = u64::from(target[2]) * u64::from(source[1]);
                    let source_scaled = u64::from(target[3]) * u64::from(source[0]);
                    let bad_aspect = u32::from(
                        target_scaled.abs_diff(source_scaled)
                            > target_scaled.max(source_scaled) / 200,
                    );
                    let runs = draw.frame.hud.runs();
                    let unsupported_blend = runs
                        .iter()
                        .find_map(|(_, blend)| match blend {
                            horizon_client::hud::Blend::Alpha
                            | horizon_client::hud::Blend::Additive => None,
                            horizon_client::hud::Blend::Invert => Some(1),
                            horizon_client::hud::Blend::Depth(_) => Some(3),
                            horizon_client::hud::Blend::Vignette => Some(4),
                            horizon_client::hud::Blend::EndPortal => Some(5),
                            horizon_client::hud::Blend::CitGlint(_) => Some(6),
                        })
                        .unwrap_or(0);
                    let unsupported_glint = u32::from(
                        draw.frame
                            .hud
                            .vertices
                            .iter()
                            .any(|vertex| vertex.uv[0] >= 2.0),
                    );
                    unsafe {
                        OSReport(
                                "Horizon GX2 HUD unsupported: text_shader=%u blend=%u glint=%u aspect=%u source=%ux%u target=%ux%u\n\0"
                                .as_ptr()
                                .cast(),
                            u32::from(draw.atlas.text_shader.is_some()),
                            unsupported_blend,
                            unsupported_glint,
                            bad_aspect,
                            source[0],
                            source[1],
                            target[2],
                            target[3],
                        );
                    }
                }
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

fn project_terrain_vertex(
    vertex: &mut TerrainGpuVertex,
    key: SectionKey,
    frame: &Frame<'_>,
    aspect: f32,
) {
    let [cx, cy, cz] = frame.camera.to_array();
    let x = (f64::from(key.0) * 16.0 + f64::from(vertex.position[0]) - cx) as f32;
    let y = (f64::from(key.1) * 16.0 + f64::from(vertex.position[1]) - cy) as f32;
    let z = (f64::from(key.2) * 16.0 + f64::from(vertex.position[2]) - cz) as f32;
    let yaw = (frame.yaw - 180.0).to_radians();
    let (sy, cyaw) = yaw.sin_cos();
    let view_x = cyaw * x + sy * z;
    let yaw_z = -sy * x + cyaw * z;
    let pitch = frame.pitch.to_radians();
    let (sp, cp) = pitch.sin_cos();
    let view_y = cp * y - sp * yaw_z;
    let view_z = sp * y + cp * yaw_z;
    let depth = -view_z;
    let near = 0.05f32;
    let far = frame.depth_far.max(near + 1.0);
    let focal = 1.0 / (frame.fov_degrees.to_radians() * 0.5).tan();
    let depth_scale = far / (near - far);
    let depth_offset = far * near / (near - far);
    vertex.position = [
        view_x * focal / aspect,
        view_y * focal,
        view_z * depth_scale + depth_offset,
        depth,
    ];
}

fn unsupported_world_passes(frame: &Frame<'_>) -> impl Iterator<Item = (u32, &'static str)> {
    [
        (!frame.celestial.is_empty(), 1 << 0, "celestial"),
        (frame.stars.is_some(), 1 << 1, "stars"),
        (frame.clouds.is_some(), 1 << 2, "clouds"),
        (!frame.weather.is_empty(), 1 << 3, "weather"),
        (
            !frame.entities.is_empty() || !frame.glowing.is_empty(),
            1 << 4,
            "entities",
        ),
        (
            !frame.static_block_entities.is_empty(),
            1 << 5,
            "block entities",
        ),
        (frame.skin.is_some(), 1 << 16, "skinned entities"),
        (!frame.hand.is_empty(), 1 << 6, "first-person hand"),
        (!frame.outline.is_empty(), 1 << 7, "block outline"),
        (
            !frame.debug_lines.is_empty()
                || !frame.debug_lines_on_top.is_empty()
                || !frame.debug_crosshair.is_empty(),
            1 << 8,
            "debug lines",
        ),
        (!frame.screen.is_empty(), 1 << 9, "screen effects"),
        (frame.border.is_some(), 1 << 10, "world border"),
        (
            !frame.additive.is_empty() || !frame.see_through.is_empty(),
            1 << 11,
            "translucent geometry",
        ),
        (
            !frame.world_primitives.is_empty(),
            1 << 12,
            "world primitives",
        ),
        (!frame.shadow_only.is_empty(), 1 << 13, "shadow pass"),
        (true, 1 << 14, "terrain lighting (flat fullbright terrain)"),
        (!frame.lightning.bolts.is_empty(), 1 << 17, "lightning"),
        (
            !frame.dynamic_lights.is_empty()
                || !frame.colored_lights.is_empty()
                || !frame.emissive_layers.is_empty(),
            1 << 18,
            "dynamic and emissive lighting",
        ),
        (
            frame.bob.to_cols_array()
                != [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ]
                || frame.spin.to_cols_array()
                    != [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ],
            1 << 19,
            "camera distortion",
        ),
        (
            !frame.post_effects.effects.is_empty() || frame.menu_blur.unwrap_or(0.0) > 0.0,
            1 << 15,
            "post effects",
        ),
    ]
    .into_iter()
    .filter_map(|(active, bit, name)| active.then_some((bit, name)))
}

fn log_unsupported_world_pass(name: &str) {
    let message = match name {
        "celestial" => "Horizon GX2: skipped unsupported celestial pass\n\0",
        "stars" => "Horizon GX2: skipped unsupported stars pass\n\0",
        "clouds" => "Horizon GX2: skipped unsupported clouds pass\n\0",
        "weather" => "Horizon GX2: skipped unsupported weather pass\n\0",
        "entities" => "Horizon GX2: skipped unsupported entities pass\n\0",
        "block entities" => "Horizon GX2: skipped unsupported block-entities pass\n\0",
        "first-person hand" => "Horizon GX2: skipped unsupported hand pass\n\0",
        "block outline" => "Horizon GX2: skipped unsupported block-outline pass\n\0",
        "debug lines" => "Horizon GX2: skipped unsupported debug-lines pass\n\0",
        "screen effects" => "Horizon GX2: skipped unsupported screen-effects pass\n\0",
        "world border" => "Horizon GX2: skipped unsupported world-border pass\n\0",
        "translucent geometry" => "Horizon GX2: skipped unsupported translucent-geometry pass\n\0",
        "world primitives" => "Horizon GX2: skipped unsupported world-primitives pass\n\0",
        "shadow pass" => "Horizon GX2: skipped unsupported shadow pass\n\0",
        "skinned entities" => "Horizon GX2: skipped unsupported skinned-entities pass\n\0",
        "lightning" => "Horizon GX2: skipped unsupported lightning pass\n\0",
        "dynamic and emissive lighting" => {
            "Horizon GX2: skipped unsupported dynamic/emissive lighting\n\0"
        }
        "camera distortion" => "Horizon GX2: skipped unsupported camera distortion\n\0",
        "terrain lighting (flat fullbright terrain)" => {
            "Horizon GX2: terrain is flat fullbright; lightmap sampling unsupported\n\0"
        }
        _ => "Horizon GX2: skipped unsupported world pass\n\0",
    };
    unsafe { OSReport(message.as_ptr().cast()) };
}

impl CafeGameBackend for Gx2LoadingPresenter<'_> {
    fn last_game_frame_was_partial(&self) -> bool {
        self.last_frame_partial
    }

    fn prepare_game_artwork(&mut self, artwork: CafeGameArtwork<'_>) -> horizon_client::Result<()> {
        self.prepare(CafeLoadingArtwork {
            atlas: artwork.atlas,
            panorama: Some(artwork.panorama),
            generation: artwork.hud_generation,
        })?;
        if self.terrain_generation != Some(artwork.terrain_generation) || self.terrain.is_none() {
            let source = artwork.terrain_pixels.get();
            let atlas = TerrainAtlas::from_layers(
                &source,
                artwork.terrain_sprite_size,
                artwork.terrain_layers,
            )?;
            let mut native = std::ptr::null_mut();
            let code = unsafe {
                horizon_gx2_terrain_create(
                    &mut native,
                    self.terrain_shader.as_ptr().cast(),
                    atlas.width,
                    atlas.height,
                    atlas.pixels.as_ptr(),
                    atlas.pixels.len(),
                )
            };
            if code != 0 {
                return Err(format!("GX2 terrain atlas creation failed ({code})").into());
            }
            let native = NonNull::new(native).ok_or("GX2 terrain returned a null resource")?;
            self.terrain = Some(Terrain {
                native,
                _shader: PhantomData,
                _thread: PhantomData,
            });
            self.terrain_atlas = Some(atlas);
            self.terrain_generation = Some(artwork.terrain_generation);
            self.sections.clear();
        }
        Ok(())
    }
}

impl TerrainBackend for Gx2LoadingPresenter<'_> {
    fn upload_section(
        &mut self,
        key: SectionKey,
        mesh: &SectionMesh,
    ) -> horizon_client::Result<()> {
        if !mesh.translucent.is_empty() {
            return Err(
                "GX2 terrain first slice does not support translucent section vertices".into(),
            );
        }
        if !mesh.solid.len().is_multiple_of(4) {
            return Err("Horizon solid section mesh is not quad aligned".into());
        }
        let atlas = self
            .terrain_atlas
            .as_ref()
            .ok_or("GX2 terrain atlas is not prepared")?;
        let mut source = mesh.solid.clone();
        for vertex in &mut source {
            atlas.remap_vertex(vertex)?;
        }
        let origin = [0.0; 3];
        let capacity = source
            .len()
            .checked_div(4)
            .and_then(|quads| quads.checked_mul(6))
            .ok_or("GX2 terrain vertex count overflow")?;
        let mut vertices = vec![TerrainGpuVertex::default(); capacity];
        let expanded_count = expand_quads_into(&source, origin, &mut vertices)
            .ok_or("GX2 terrain quad expansion failed")?;
        vertices.truncate(expanded_count);
        let count =
            u32::try_from(vertices.len()).map_err(|_| "GX2 section vertex count overflow")?;
        let mut native = std::ptr::null_mut();
        let code =
            unsafe { horizon_gx2_terrain_section_create(&mut native, vertices.as_ptr(), count) };
        if code != 0 {
            return Err(format!("GX2 section allocation failed ({code})").into());
        }
        let native = NonNull::new(native).ok_or("GX2 section returned a null resource")?;
        self.sections
            .insert(key, TerrainSection { native, vertices });
        Ok(())
    }

    fn clear_sections(&mut self, _layout: ColumnLayout) {
        self.sections.clear();
    }

    fn remove_columns(&mut self, columns: &mut dyn Iterator<Item = (i32, i32)>) {
        let removed: HashSet<_> = columns.collect();
        self.sections
            .retain(|(x, _, z), _| !removed.contains(&(*x, *z)));
    }
}

impl PresentationBackend for Gx2LoadingPresenter<'_> {
    fn viewport_size(&self) -> [u32; 2] {
        self.layout
    }
    fn resize(&mut self, _width: u32, _height: u32) {
        if let Ok(outputs) = output_dimensions() {
            self.outputs = outputs;
            self.layout = outputs[1];
        }
    }
    fn release_surface(&mut self) {
        self.sections.clear();
        self.terrain = None;
        self.hud = None;
        self.panorama = None;
        self.panorama_identity = None;
        self.generation = None;
        self.atlas_revision = None;
    }
    fn recreate_surface(&mut self) -> horizon_client::Result<()> {
        self.outputs = output_dimensions()?;
        self.layout = self.outputs[1];
        Ok(())
    }
    fn set_present_mode(&mut self, _vsync: bool, _mode: PresentMode) {}
    fn set_gamma(&mut self, _gamma: f32) {}

    fn present_frame(&mut self, frame: &Frame<'_>) -> horizon_client::Result<bool> {
        self.last_frame_partial = false;
        let unsupported: Vec<_> = if frame.title {
            Vec::new()
        } else {
            unsupported_world_passes(frame).collect()
        };
        if self
            .target_viewports()
            .iter()
            .any(|r| r[2] == 0 || r[3] == 0)
        {
            return Err("Game frame viewport does not fit TV and DRC".into());
        }
        struct Draw<'a, 'frame, 'shader> {
            presenter: &'a mut Gx2LoadingPresenter<'shader>,
            frame: &'a Frame<'frame>,
            targets: [[u32; 4]; 2],
            result: Result<(), &'static str>,
        }
        unsafe extern "C" fn draw_target(
            context: *mut c_void,
            target: u32,
            width: u32,
            height: u32,
        ) -> i32 {
            let draw = unsafe { &mut *context.cast::<Draw<'_, '_, '_>>() };
            if target > 1 {
                draw.result = Err("invalid GX2 target index");
                return -1;
            }
            let index = target as usize;
            let rect = draw.targets[index];
            if draw.frame.title {
                if let Some(panorama) = draw.presenter.panorama.as_mut() {
                    if let Err(error) = unsafe {
                        panorama.draw([width, height], rect, draw.frame.yaw, draw.frame.pitch)
                    } {
                        draw.result = Err(error);
                        return -1;
                    }
                }
            } else if let Some(terrain) = draw.presenter.terrain.as_ref() {
                let aspect = rect[2] as f32 / rect[3].max(1) as f32;
                // Wait once before updating all retained section buffers;
                // waiting inside each per-section update stalls streaming.
                unsafe { horizon_gx2_terrain_wait_idle() };
                let mut projected = Vec::new();
                for (&key, section) in &mut draw.presenter.sections {
                    projected.clear();
                    projected.extend(section.vertices.iter().copied().map(|mut v| {
                        project_terrain_vertex(&mut v, key, draw.frame, aspect);
                        v
                    }));
                    if unsafe {
                        horizon_gx2_terrain_section_update(
                            section.native.as_ptr(),
                            projected.as_ptr(),
                            projected.len() as u32,
                        )
                    } != 0
                    {
                        draw.result = Err("GX2 terrain section update failed");
                        return -1;
                    }
                    if unsafe {
                        horizon_gx2_terrain_draw(
                            terrain.native.as_ptr(),
                            section.native.as_ptr(),
                            width,
                            height,
                            rect[0],
                            rect[1],
                            rect[2],
                            rect[3],
                        )
                    } != 0
                    {
                        draw.result = Err("GX2 terrain draw failed");
                        return -1;
                    }
                }
            }
            let Some(hud) = draw.presenter.hud.as_mut() else {
                draw.result = Err("GX2 HUD presenter is not prepared");
                return -1;
            };
            if unsafe {
                hud.draw_batch_to(
                    draw.frame.hud,
                    draw.presenter.layout,
                    rect,
                    draw.frame.ticks,
                    true,
                )
            }
            .is_err()
            {
                draw.result = Err("Horizon HUD draw failed");
                return -1;
            }
            0
        }
        let clear = [
            frame.sky.sky_color[0],
            frame.sky.sky_color[1],
            frame.sky.sky_color[2],
            1.0,
        ];
        let targets = self.target_viewports();
        let mut draw = Draw {
            presenter: self,
            frame,
            targets,
            result: Ok(()),
        };
        let host = unsafe {
            horizon_loading_host_present_with_clear(
                draw_target,
                (&mut draw as *mut Draw<'_, '_, '_>).cast(),
                clear.as_ptr(),
            )
        };
        draw.result?;
        if host != 0 {
            return Err("WHB did not accept both GX2 game frames".into());
        }
        if !unsupported.is_empty() {
            for (flag, name) in &unsupported {
                if draw.presenter.unsupported_logged & *flag == 0 {
                    log_unsupported_world_pass(name);
                    draw.presenter.unsupported_logged |= *flag;
                }
            }
            draw.presenter.last_frame_partial = true;
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

impl HudBackend for Gx2LoadingPresenter<'_> {
    fn set_hud_sprite_animations(&mut self, _animations: &[(Rect, SpriteAnimation)]) {}
    fn set_hud_text_shader(&mut self, _shader: Option<&horizon_shaders::CoreProgram>) {}
    fn set_hud_cit_glints(&mut self, _textures: Option<&GlintTextures>) {}
    fn set_lines_shader(&mut self, _shader: Option<&horizon_shaders::CoreProgram>) {}
    fn max_texture_dimension_2d(&self) -> u32 {
        8192
    }
}
