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
use horizon_client::hud::{
    Rect,
    cit_glint::{Factor, GlintTextures},
};
use horizon_client::mesher::{SectionMesh, TerrainVertex};
use horizon_client::sprites::SpriteAnimation;
use horizon_client::{
    game::{
        CafeGameArtwork, CafeGameBackend, CafeLoadingArtwork, CafeLoadingPresenter, CafeScreenshot,
        CafeScreenshotOutput, CafeTerrainBuffers,
    },
    gfx::{Frame, panorama},
    presentation::{HudBackend, PresentationBackend, TerrainBackend},
};
use horizon_wiiu_gx2_terrain::{
    TerrainAtlas, TerrainAtlasLayout, TerrainClipRows, TerrainGpuVertex, terrain_clip_rows,
};
use horizon_wiiu_platform::hud::{CitSlot, horizon::HorizonPresenter};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    marker::PhantomData,
    mem::MaybeUninit,
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
    fn horizon_loading_host_read_rgba(target: u32, out: *mut u8, length: usize) -> i32;
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
    fn horizon_gx2_terrain_index_allocation_bytes(terrain: *const c_void) -> u32;
    fn horizon_gx2_terrain_section_create(
        out: *mut *mut c_void,
        count: u32,
        fill: unsafe extern "C" fn(*mut c_void, *mut MaybeUninit<TerrainGpuVertex>, u32) -> i32,
        context: *mut c_void,
        allocation_bytes: *mut u32,
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
        clip_rows: *const f32,
        section_origin: *const f32,
        fog_color: *const f32,
        fog_ranges: *const f32,
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
    vertex_count: u32,
    allocation_bytes: u32,
}
impl Drop for TerrainSection {
    fn drop(&mut self) {
        unsafe { horizon_gx2_terrain_section_destroy(self.native.as_ptr()) }
    }
}

struct TerrainUpload<'a> {
    atlas: &'a TerrainAtlasLayout,
    source: &'a [TerrainVertex],
    error: Option<&'static str>,
}

unsafe extern "C" fn fill_terrain_vertices(
    context: *mut c_void,
    vertices: *mut MaybeUninit<TerrainGpuVertex>,
    count: u32,
) -> i32 {
    // SAFETY: the native upload borrows the context only for this call.
    let upload = unsafe { &mut *context.cast::<TerrainUpload<'_>>() };
    let count = count as usize;
    if vertices.is_null()
        || !vertices.is_aligned()
        || count != upload.source.len()
        || count > isize::MAX as usize / std::mem::size_of::<TerrainGpuVertex>()
    {
        upload.error = Some("GX2 terrain vertex mapping is invalid");
        return -1;
    }
    // SAFETY: C allocated and mapped `count` vertices of the matching ABI.
    // They are uninitialized, exclusively CPU-owned, and not yet GPU-visible.
    let output = unsafe { std::slice::from_raw_parts_mut(vertices, count) };
    match upload.atlas.write_quad_vertices(upload.source, output) {
        Ok(()) => 0,
        Err(error) => {
            upload.error = Some(error);
            -1
        }
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
    terrain_atlas: Option<TerrainAtlasLayout>,
    terrain_generation: Option<u64>,
    terrain_min_y: Option<i32>,
    sections: HashMap<SectionKey, TerrainSection>,
    translucent_terrain_omitted: HashSet<SectionKey>,
    generation: Option<u64>,
    atlas_revision: Option<u64>,
    cit_generation: Option<u64>,
    panorama_identity: Option<(u64, u64, usize, u32)>,
    layout: [u32; 2],
    outputs: [[u32; 2]; 2],
    capacity: u32,
    trace_first_prepare: bool,
    trace_first_present: bool,
    trace_first_target: [bool; 2],
    loading_trace_iteration: u32,
    trace_next_loading_frame: bool,
    trace_loading_iteration: bool,
    last_frame_partial: bool,
    screenshot_requested: Option<CafeScreenshotOutput>,
    screenshot_result: Option<Result<CafeScreenshot, String>>,
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
            terrain_min_y: None,
            sections: HashMap::new(),
            translucent_terrain_omitted: HashSet::new(),
            generation: None,
            atlas_revision: None,
            cit_generation: None,
            panorama_identity: None,
            layout: [outputs[1][0], outputs[1][1]],
            outputs,
            capacity,
            trace_first_prepare: false,
            trace_first_present: false,
            trace_first_target: [false; 2],
            loading_trace_iteration: 0,
            trace_next_loading_frame: false,
            trace_loading_iteration: false,
            last_frame_partial: false,
            screenshot_requested: None,
            screenshot_result: None,
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

    /// Trace the next loading frame's preparation and presentation phases.
    /// The request is consumed by `prepare_loading` and remains active through
    /// that frame's synchronous TV/DRC presentation callbacks.
    pub fn trace_next_loading_frame(&mut self) {
        self.trace_next_loading_frame = true;
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
            self.cit_generation = None;
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
        self.loading_trace_iteration = self.loading_trace_iteration.wrapping_add(1);
        self.trace_loading_iteration = std::mem::replace(&mut self.trace_next_loading_frame, false);
        if self.trace_loading_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u prepare begin\n\0"
                        .as_ptr()
                        .cast(),
                    self.loading_trace_iteration,
                );
            }
        }
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
        if self.trace_loading_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u prepare returned ok=%u\n\0"
                        .as_ptr()
                        .cast(),
                    self.loading_trace_iteration,
                    u32::from(result.is_ok()),
                );
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
        let trace_iteration = self.trace_loading_iteration;
        let iteration = self.loading_trace_iteration;
        if trace_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u present_loading entry\n\0"
                        .as_ptr()
                        .cast(),
                    iteration,
                );
            }
        }
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
        if trace_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u present_loading validation complete\n\0"
                        .as_ptr()
                        .cast(),
                    iteration,
                );
            }
        }
        struct Draw<'a, 'frame, 'shader> {
            presenter: &'a mut Gx2LoadingPresenter<'shader>,
            atlas: &'a horizon_client::hud::HudAtlas,
            frame: &'a Frame<'frame>,
            targets: [[u32; 4]; 2],
            tick: u64,
            animated: bool,
            result: Result<(), &'static str>,
            trace_iteration: bool,
            iteration: u32,
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
            if draw.trace_iteration {
                unsafe {
                    OSReport(
                        "Horizon GX2 loading iteration=%u target=%u callback begin\n\0"
                            .as_ptr()
                            .cast(),
                        draw.iteration,
                        target,
                    );
                }
            }
            if let Some(panorama) = draw.presenter.panorama.as_mut() {
                if draw.trace_iteration {
                    unsafe {
                        OSReport(
                            "Horizon GX2 loading iteration=%u target=%u panorama.draw begin\n\0"
                                .as_ptr()
                                .cast(),
                            draw.iteration,
                            target,
                        );
                    }
                }
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
                if draw.trace_iteration {
                    unsafe {
                        OSReport(
                            "Horizon GX2 loading iteration=%u target=%u panorama.draw returned\n\0"
                                .as_ptr()
                                .cast(),
                            draw.iteration,
                            target,
                        );
                    }
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
                if draw.trace_iteration {
                    OSReport(
                        "Horizon GX2 loading iteration=%u target=%u hud.draw_to begin\n\0"
                            .as_ptr()
                            .cast(),
                        draw.iteration,
                        target,
                    );
                }
                hud.draw_to(
                    draw.atlas,
                    draw.frame.hud,
                    draw.presenter.layout,
                    target_rect,
                    target,
                    draw.tick,
                    draw.animated,
                )
            };
            if draw.trace_iteration {
                unsafe {
                    OSReport(
                        "Horizon GX2 loading iteration=%u target=%u hud.draw_to returned ok=%u\n\0"
                            .as_ptr()
                            .cast(),
                        draw.iteration,
                        target,
                        u32::from(hud_result.is_ok()),
                    );
                }
            }
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
                    let cit = u32::from(
                        runs.iter()
                            .any(|(_, b)| matches!(b, horizon_client::hud::Blend::CitGlint(_))),
                    );
                    let glint = u32::from(
                        draw.frame
                            .hud
                            .vertices
                            .iter()
                            .any(|vertex| vertex.uv[0] >= 2.0),
                    );
                    unsafe {
                        OSReport(
                                "Horizon GX2 HUD unsupported: text_shader=%u cit=%u glint=%u aspect=%u source=%ux%u target=%ux%u\n\0"
                                .as_ptr()
                                .cast(),
                            u32::from(draw.atlas.text_shader.is_some()),
                            cit,
                            glint,
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
            trace_iteration,
            iteration,
        };
        // SAFETY: the C helper calls the callback synchronously on this same
        // render thread, binds and finishes TV and DRC independently, then returns.
        if trace {
            gx2_loading_stage!("first native TV/DRC present begin");
        }
        if trace_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u native present begin\n\0"
                        .as_ptr()
                        .cast(),
                    iteration,
                );
            }
        }
        let host_result = unsafe {
            horizon_loading_host_present(draw_target, (&mut draw as *mut Draw<'_, '_, '_>).cast())
        };
        if trace {
            gx2_loading_stage!("first native TV/DRC present returned");
        }
        if trace_iteration {
            unsafe {
                OSReport(
                    "Horizon GX2 loading iteration=%u native present returned result=%d\n\0"
                        .as_ptr()
                        .cast(),
                    iteration,
                    host_result,
                );
            }
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

struct TerrainProjection {
    camera: [f64; 3],
    min_y: i32,
    yaw_sin: f32,
    yaw_cos: f32,
    pitch_sin: f32,
    pitch_cos: f32,
    focal: f32,
    aspect: f32,
    depth_scale: f32,
    depth_offset: f32,
}

impl TerrainProjection {
    fn new(frame: &Frame<'_>, min_y: i32, aspect: f32) -> Self {
        let yaw = (frame.yaw - 180.0).to_radians();
        let (yaw_sin, yaw_cos) = yaw.sin_cos();
        let pitch = frame.pitch.to_radians();
        let (pitch_sin, pitch_cos) = pitch.sin_cos();
        let near = 0.05f32;
        let far = frame.depth_far.max(near + 1.0);
        Self {
            camera: frame.camera.to_array(),
            min_y,
            yaw_sin,
            yaw_cos,
            pitch_sin,
            pitch_cos,
            focal: 1.0 / (frame.fov_degrees.to_radians() * 0.5).tan(),
            aspect,
            depth_scale: far / (near - far),
            depth_offset: far * near / (near - far),
        }
    }

    fn section_origin(&self, key: SectionKey) -> [f64; 3] {
        // Subtract the camera in f64 before converting the section origin to
        // f32; local vertex coordinates stay small and are added on the GPU.
        [
            f64::from(key.0) * 16.0 - self.camera[0],
            f64::from(self.min_y + key.1 * 16) - self.camera[1],
            f64::from(key.2) * 16.0 - self.camera[2],
        ]
    }

    fn clip_rows(&self, origin: [f64; 3]) -> TerrainClipRows {
        terrain_clip_rows(
            origin,
            self.yaw_sin,
            self.yaw_cos,
            self.pitch_sin,
            self.pitch_cos,
            self.focal,
            self.aspect,
            self.depth_scale,
            self.depth_offset,
        )
    }
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

fn set_translucent_terrain_omitted(
    omitted: &mut HashSet<SectionKey>,
    key: SectionKey,
    is_omitted: bool,
) {
    if is_omitted {
        omitted.insert(key);
    } else {
        omitted.remove(&key);
    }
}

fn retain_translucent_terrain_omissions(
    omitted: &mut HashSet<SectionKey>,
    removed_columns: &HashSet<(i32, i32)>,
) {
    omitted.retain(|(x, _, z)| !removed_columns.contains(&(*x, *z)));
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
        "translucent terrain" => {
            "Horizon GX2: skipped translucent terrain; solid terrain remains visible\n\0"
        }
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
    fn terrain_buffer_census(&self) -> Option<CafeTerrainBuffers> {
        let terrain = self.terrain.as_ref()?;
        let mut census = CafeTerrainBuffers {
            sections: self.sections.len(),
            shared_index_allocation_bytes: unsafe {
                horizon_gx2_terrain_index_allocation_bytes(terrain.native.as_ptr()) as usize
            },
            ..Default::default()
        };
        for section in self.sections.values() {
            census.vertices += section.vertex_count as usize;
            census.vertex_allocation_bytes += section.allocation_bytes as usize;
        }
        census.quads = census.vertices / 4;
        census.vertex_payload_bytes = census.vertices * std::mem::size_of::<TerrainGpuVertex>();
        Some(census)
    }

    fn request_screenshot(&mut self, output: CafeScreenshotOutput) {
        self.screenshot_requested = Some(output);
        self.screenshot_result = None;
    }

    fn take_screenshot(&mut self) -> Option<Result<CafeScreenshot, String>> {
        self.screenshot_requested = None;
        self.screenshot_result.take()
    }

    fn last_game_frame_was_partial(&self) -> bool {
        self.last_frame_partial
    }

    fn prepare_game_artwork(&mut self, artwork: CafeGameArtwork<'_>) -> horizon_client::Result<()> {
        self.prepare(CafeLoadingArtwork {
            atlas: artwork.atlas,
            panorama: Some(artwork.panorama),
            generation: artwork.hud_generation,
        })?;
        if self.cit_generation != Some(artwork.hud_generation) {
            if let Some(textures) = artwork.cit_glints.filter(|textures| !textures.is_empty()) {
                let slots = textures
                    .slots
                    .iter()
                    .map(|slot| {
                        let blend = textures
                            .blends
                            .get(usize::from(slot.blend))
                            .ok_or("CIT glint slot references a missing blend")?;
                        let factors = blend.map_or(
                            [Factor::One, Factor::Zero, Factor::One, Factor::Zero],
                            |b| [b.src, b.dst, b.src_alpha, b.dst_alpha],
                        );
                        Ok(CitSlot {
                            rgba: slot.rgba,
                            factors: factors.map(cit_factor),
                            blur: u32::from(slot.blur),
                            blend: u32::from(slot.blend),
                        })
                    })
                    .collect::<Result<Vec<_>, &str>>()?;
                let hud = self.hud.as_mut().ok_or("GX2 HUD presenter missing")?;
                unsafe {
                    hud.set_cit_glints([textures.width, textures.height], &textures.pixels, &slots)
                }
                .map_err(|error| format!("GX2 CIT glint upload failed ({error:?})"))?;
            }
            self.cit_generation = Some(artwork.hud_generation);
        }
        if self.terrain_generation != Some(artwork.terrain_generation) || self.terrain.is_none() {
            let source = artwork.terrain_pixels.get();
            let TerrainAtlas { pixels, layout } = TerrainAtlas::from_layers(
                &source,
                artwork.terrain_sprite_size,
                artwork.terrain_layers,
            )?;
            let mut native = std::ptr::null_mut();
            let code = unsafe {
                horizon_gx2_terrain_create(
                    &mut native,
                    self.terrain_shader.as_ptr().cast(),
                    layout.width,
                    layout.height,
                    pixels.as_ptr(),
                    pixels.len(),
                )
            };
            if code != 0 {
                return Err(format!("GX2 terrain atlas creation failed ({code})").into());
            }
            let native = NonNull::new(native).ok_or("GX2 terrain returned a null resource")?;
            // GX2 has copied the pixels. Section uploads only need UV layout.
            let staging_bytes = pixels.capacity() as u32;
            drop(pixels);
            unsafe {
                OSReport(
                    "Horizon memory gx2_terrain rust_staging_bytes_released=%u\n\0".as_ptr().cast(),
                    staging_bytes,
                );
            }
            self.terrain = Some(Terrain {
                native,
                _shader: PhantomData,
                _thread: PhantomData,
            });
            self.terrain_atlas = Some(layout);
            self.terrain_generation = Some(artwork.terrain_generation);
            self.sections.clear();
            self.translucent_terrain_omitted.clear();
        }
        Ok(())
    }
}

fn cit_factor(factor: Factor) -> u32 {
    match factor {
        Factor::Zero => 0,
        Factor::One => 1,
        Factor::SrcColor => 2,
        Factor::OneMinusSrcColor => 3,
        Factor::SrcAlpha => 4,
        Factor::OneMinusSrcAlpha => 5,
        Factor::DstAlpha => 6,
        Factor::OneMinusDstAlpha => 7,
        Factor::DstColor => 8,
        Factor::OneMinusDstColor => 9,
        Factor::SrcAlphaSaturate => 10,
    }
}

impl TerrainBackend for Gx2LoadingPresenter<'_> {
    fn upload_section(
        &mut self,
        key: SectionKey,
        mesh: &SectionMesh,
    ) -> horizon_client::Result<()> {
        if !mesh.solid.len().is_multiple_of(4) {
            return Err("Horizon solid section mesh is not quad aligned".into());
        }
        let atlas = self
            .terrain_atlas
            .as_ref()
            .ok_or("GX2 terrain atlas is not prepared")?;
        if self.terrain_min_y.is_none() {
            return Err("GX2 terrain section layout is not prepared".into());
        }
        let vertex_count =
            u32::try_from(mesh.solid.len()).map_err(|_| "GX2 section vertex count overflow")?;
        let mut upload = TerrainUpload {
            atlas,
            source: &mesh.solid,
            error: None,
        };
        let mut native = std::ptr::null_mut();
        let mut allocation_bytes = 0;
        // SAFETY: the native function maps a new GX2 buffer and borrows
        // `upload` synchronously, releasing the mapping before returning.
        let code = unsafe {
            horizon_gx2_terrain_section_create(
                &mut native,
                vertex_count,
                fill_terrain_vertices,
                std::ptr::from_mut(&mut upload).cast(),
                &mut allocation_bytes,
            )
        };
        if let Some(error) = upload.error {
            return Err(error.into());
        }
        if code != 0 {
            return Err(format!("GX2 indexed section upload failed ({code})").into());
        }
        let native = NonNull::new(native).ok_or("GX2 section returned a null resource")?;
        // Publish only after the upload succeeds. The previous section can
        // still be drawn if allocation, mapping or conversion fails.
        self.sections.insert(
            key,
            TerrainSection {
                native,
                vertex_count,
                allocation_bytes,
            },
        );
        set_translucent_terrain_omitted(
            &mut self.translucent_terrain_omitted,
            key,
            !mesh.translucent.is_empty(),
        );
        Ok(())
    }

    fn clear_sections(&mut self, layout: ColumnLayout) {
        self.terrain_min_y = Some(layout.min_y());
        self.sections.clear();
        self.translucent_terrain_omitted.clear();
    }

    fn remove_columns(&mut self, columns: &mut dyn Iterator<Item = (i32, i32)>) {
        let removed: HashSet<_> = columns.collect();
        self.sections
            .retain(|(x, _, z), _| !removed.contains(&(*x, *z)));
        retain_translucent_terrain_omissions(&mut self.translucent_terrain_omitted, &removed);
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
        self.translucent_terrain_omitted.clear();
        self.terrain = None;
        self.hud = None;
        self.panorama = None;
        self.panorama_identity = None;
        self.generation = None;
        self.atlas_revision = None;
        self.cit_generation = None;
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
        let mut unsupported: Vec<_> = if frame.title {
            Vec::new()
        } else {
            unsupported_world_passes(frame).collect()
        };
        if !frame.title && !self.translucent_terrain_omitted.is_empty() {
            unsupported.push((1 << 20, "translucent terrain"));
        }
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
            terrain_min_y: Option<i32>,
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
                // Wait once before drawing all retained section buffers;
                // projection uniforms vary per draw, but geometry buffers do not.
                unsafe { horizon_gx2_terrain_wait_idle() };
                let projection = draw
                    .terrain_min_y
                    .map(|min_y| TerrainProjection::new(draw.frame, min_y, aspect));
                let (fog_color, fog_ranges) = draw.frame.fog_uniforms();
                for (&key, section) in &draw.presenter.sections {
                    let Some(projection) = projection.as_ref() else {
                        draw.result = Err("GX2 terrain section layout is not prepared");
                        return -1;
                    };
                    let origin = projection.section_origin(key);
                    let clip_rows = projection.clip_rows(origin);
                    let [x, y, z] = origin.map(|value| value as f32);
                    let section_origin = [x, y, z, 0.0];
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
                            clip_rows.rows.as_ptr().cast(),
                            section_origin.as_ptr(),
                            fog_color.as_ptr(),
                            fog_ranges.as_ptr(),
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
                    target,
                    draw.frame.ticks,
                    true,
                )
            }
            .is_err()
            {
                draw.result = Err("Horizon HUD draw failed");
                return -1;
            }
            let output = match target {
                0 => CafeScreenshotOutput::Tv,
                _ => CafeScreenshotOutput::GamePad,
            };
            if draw.presenter.screenshot_requested == Some(output) {
                draw.presenter.screenshot_requested = None;
                let size = (width as usize)
                    .checked_mul(height as usize)
                    .and_then(|n| n.checked_mul(4));
                draw.presenter.screenshot_result = Some(match size {
                    Some(size) if width > 0 && height > 0 && size <= 1920 * 1080 * 4 => {
                        let mut rgba = vec![0; size];
                        if unsafe {
                            horizon_loading_host_read_rgba(target, rgba.as_mut_ptr(), rgba.len())
                        } == 0
                        {
                            Ok(CafeScreenshot {
                                width,
                                height,
                                rgba,
                            })
                        } else {
                            Err(format!("GX2 {output:?} readback failed"))
                        }
                    }
                    _ => Err("unsupported Cafe screenshot dimensions".into()),
                });
            }
            0
        }
        let clear = frame.base_clear_color();
        let targets = self.target_viewports();
        let terrain_min_y = self.terrain_min_y;
        let mut draw = Draw {
            presenter: self,
            frame,
            terrain_min_y,
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
        if let Err(error) = draw.result {
            draw.presenter.screenshot_result = None;
            return Err(error.into());
        }
        if host != 0 {
            draw.presenter.screenshot_result = None;
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
