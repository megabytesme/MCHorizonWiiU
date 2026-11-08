//! Bridge for Horizon's HUD geometry and the native GX2 presenter.
//!
//! The caller still creates Horizon's batches and owns the GX2 render target.
//! The C presenter copies the atlas at creation and reads frame data only
//! during `draw`; the shader must outlive the presenter.

use horizon_hud_data::{Blend, HudVertex, Patch};
use std::ops::Range;

#[cfg(feature = "horizon-hud")]
pub mod horizon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Unsupported,
    AllocationFailed,
    /// An unexpected result from the C presenter.
    Backend(i32),
}

/// Data from Horizon's HUD batch, kept borrowed through the draw call.
pub struct Frame<'a> {
    pub vertices: &'a [HudVertex],
    pub runs: &'a [(Range<u32>, Blend)],
    pub patches: &'a [Patch],
    pub viewport: [u32; 2],
    pub scale: f32,
    pub pack_text_shader: bool,
}

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
#[repr(C)]
struct RawPatch {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    rgba: *const u8,
    length: usize,
}

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
#[repr(C)]
struct RawRun {
    start: u32,
    end: u32,
    blend: u32,
}

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
#[repr(C)]
struct RawFrame {
    vertices: *const HudVertex,
    vertex_count: u32,
    runs: *const RawRun,
    run_count: u32,
    patches: *const RawPatch,
    patch_count: u32,
    viewport_width: u32,
    viewport_height: u32,
    scale: f32,
    pack_text_shader: u32,
}

// Paired with the C presenter's assertions: Cafe uses 32-bit pointers and
// size_t. These are ABI checks, independent of the host used for tests.
#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
const _: () = {
    use std::mem::{align_of, offset_of, size_of};
    assert!(size_of::<RawRun>() == 12 && align_of::<RawRun>() == 4);
    assert!(offset_of!(RawRun, start) == 0);
    assert!(offset_of!(RawRun, end) == 4);
    assert!(offset_of!(RawRun, blend) == 8);
    assert!(size_of::<RawPatch>() == 24 && align_of::<RawPatch>() == 4);
    assert!(offset_of!(RawPatch, x) == 0);
    assert!(offset_of!(RawPatch, y) == 4);
    assert!(offset_of!(RawPatch, width) == 8);
    assert!(offset_of!(RawPatch, height) == 12);
    assert!(offset_of!(RawPatch, rgba) == 16);
    assert!(offset_of!(RawPatch, length) == 20);
    assert!(size_of::<RawFrame>() == 40 && align_of::<RawFrame>() == 4);
    assert!(offset_of!(RawFrame, vertices) == 0);
    assert!(offset_of!(RawFrame, vertex_count) == 4);
    assert!(offset_of!(RawFrame, runs) == 8);
    assert!(offset_of!(RawFrame, run_count) == 12);
    assert!(offset_of!(RawFrame, patches) == 16);
    assert!(offset_of!(RawFrame, patch_count) == 20);
    assert!(offset_of!(RawFrame, viewport_width) == 24);
    assert!(offset_of!(RawFrame, viewport_height) == 28);
    assert!(offset_of!(RawFrame, scale) == 32);
    assert!(offset_of!(RawFrame, pack_text_shader) == 36);
};

// Own the C descriptors for exactly as long as the borrowed frame is in use.
#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
struct Prepared<'a> {
    frame: &'a Frame<'a>,
    _extra_patches: &'a [Patch],
    runs: Vec<RawRun>,
    patches: Vec<RawPatch>,
}

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
impl<'a> Prepared<'a> {
    fn new(frame: &'a Frame<'a>) -> Result<Self, Error> {
        Self::with_extra_patches(frame, &[])
    }

    fn with_extra_patches(frame: &'a Frame<'a>, extra: &'a [Patch]) -> Result<Self, Error> {
        u32::try_from(frame.vertices.len()).map_err(|_| Error::Invalid)?;
        u32::try_from(frame.runs.len()).map_err(|_| Error::Invalid)?;
        let patch_count = extra
            .len()
            .checked_add(frame.patches.len())
            .ok_or(Error::Invalid)?;
        u32::try_from(patch_count).map_err(|_| Error::Invalid)?;
        if frame.pack_text_shader {
            return Err(Error::Unsupported);
        }
        let mut runs = Vec::with_capacity(frame.runs.len());
        for (range, blend) in frame.runs {
            // Never discard the payload of a Depth or CitGlint run.
            let blend = match blend {
                Blend::Alpha => 0,
                Blend::Additive => 2,
                _ => return Err(Error::Unsupported),
            };
            runs.push(RawRun {
                start: range.start,
                end: range.end,
                blend,
            });
        }
        // Desktop updates animated sprites before uploading the frame's
        // glyph patches. Preserve that order without cloning pixel buffers.
        let patches = extra
            .iter()
            .chain(frame.patches)
            .map(|p| RawPatch {
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
                rgba: p.rgba.as_ptr(),
                length: p.rgba.len(),
            })
            .collect();
        Ok(Self {
            frame,
            _extra_patches: extra,
            runs,
            patches,
        })
    }

    fn raw(&self) -> RawFrame {
        RawFrame {
            vertices: self.frame.vertices.as_ptr(),
            vertex_count: self.frame.vertices.len() as u32,
            runs: self.runs.as_ptr(),
            run_count: self.runs.len() as u32,
            patches: self.patches.as_ptr(),
            patch_count: self.patches.len() as u32,
            viewport_width: self.frame.viewport[0],
            viewport_height: self.frame.viewport[1],
            scale: self.frame.scale,
            pack_text_shader: u32::from(self.frame.pack_text_shader),
        }
    }
}

fn validate_target_viewport(source: [u32; 2], target: [u32; 4]) -> Result<(), Error> {
    let [source_width, source_height] = source;
    let [x, y, width, height] = target;
    if source_width == 0
        || source_height == 0
        || width == 0
        || height == 0
        || x.checked_add(width).is_none()
        || y.checked_add(height).is_none()
    {
        return Err(Error::Invalid);
    }

    let target_scaled = u64::from(width) * u64::from(source_height);
    let source_scaled = u64::from(height) * u64::from(source_width);
    let difference = target_scaled.abs_diff(source_scaled);
    let largest = target_scaled.max(source_scaled);
    if difference > largest / 200 {
        return Err(Error::Unsupported);
    }
    Ok(())
}

#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
mod cafe {
    use super::*;
    use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

    extern "C" {
        fn horizon_gx2_hud_create(
            out: *mut *mut c_void,
            gfd: *const c_void,
            width: u32,
            height: u32,
            rgba: *const u8,
            length: usize,
            capacity: u32,
        ) -> i32;
        fn horizon_gx2_hud_draw(hud: *mut c_void, frame: *const RawFrame) -> i32;
        fn horizon_gx2_hud_draw_to(
            hud: *mut c_void,
            frame: *const RawFrame,
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> i32;
        fn horizon_gx2_hud_destroy(hud: *mut c_void);
    }

    fn result(code: i32) -> Result<(), Error> {
        match code {
            0 => Ok(()),
            -1 => Err(Error::Invalid),
            -2 => Err(Error::Unsupported),
            -3 => Err(Error::AllocationFailed),
            n => Err(Error::Backend(n)),
        }
    }

    /// A render-thread-bound presenter. The compiled GFD stays borrowed until drop.
    pub struct Presenter<'shader> {
        hud: NonNull<c_void>,
        _shader: PhantomData<&'shader [u8]>,
        _thread: PhantomData<*mut ()>,
    }

    impl<'shader> Presenter<'shader> {
        /// # Safety
        /// GX2 must be initialized and remain valid on this thread until drop.
        /// `gfd` must be an aligned, complete, valid GFD compiled from the
        /// presenter's shaders; the WUT GFD loader has no length argument.
        pub unsafe fn new(
            gfd: &'shader [u8],
            width: u32,
            height: u32,
            rgba: &[u8],
            capacity: u32,
        ) -> Result<Self, Error> {
            let mut hud = std::ptr::null_mut();
            result(horizon_gx2_hud_create(
                &mut hud,
                gfd.as_ptr().cast(),
                width,
                height,
                rgba.as_ptr(),
                rgba.len(),
                capacity,
            ))?;
            let hud = NonNull::new(hud).ok_or(Error::Invalid)?;
            Ok(Self {
                hud,
                _shader: PhantomData,
                _thread: PhantomData,
            })
        }

        /// # Safety
        /// The calling render thread must have a valid GX2 color target bound.
        /// This changes GX2 render state.
        pub unsafe fn draw(&mut self, frame: &Frame<'_>) -> Result<(), Error> {
            let prepared = Prepared::new(frame)?;
            result(horizon_gx2_hud_draw(self.hud.as_ptr(), &prepared.raw()))
        }

        /// Draw source-layout vertices into a target rectangle without changing
        /// their normalization. The target must preserve the source aspect
        /// ratio (within 0.5%); hosts can letterbox mismatched screens.
        ///
        /// # Safety
        /// The calling render thread must have a valid GX2 color target bound.
        /// `target` is `[x, y, width, height]` in that target and must fit its
        /// bounds. This changes GX2 render state.
        pub unsafe fn draw_to(&mut self, frame: &Frame<'_>, target: [u32; 4]) -> Result<(), Error> {
            let prepared = Prepared::new(frame)?;
            validate_target_viewport(frame.viewport, target)?;
            result(horizon_gx2_hud_draw_to(
                self.hud.as_ptr(),
                &prepared.raw(),
                target[0],
                target[1],
                target[2],
                target[3],
            ))
        }

        #[cfg(feature = "horizon-hud")]
        pub(super) unsafe fn draw_with_patches(
            &mut self,
            frame: &Frame<'_>,
            extra: &[Patch],
        ) -> Result<(), Error> {
            let prepared = Prepared::with_extra_patches(frame, extra)?;
            result(horizon_gx2_hud_draw(self.hud.as_ptr(), &prepared.raw()))
        }

        #[cfg(feature = "horizon-hud")]
        pub(super) unsafe fn draw_with_patches_to(
            &mut self,
            frame: &Frame<'_>,
            extra: &[Patch],
            target: [u32; 4],
        ) -> Result<(), Error> {
            let prepared = Prepared::with_extra_patches(frame, extra)?;
            validate_target_viewport(frame.viewport, target)?;
            result(horizon_gx2_hud_draw_to(
                self.hud.as_ptr(),
                &prepared.raw(),
                target[0],
                target[1],
                target[2],
                target[3],
            ))
        }
    }

    impl Drop for Presenter<'_> {
        fn drop(&mut self) {
            unsafe { horizon_gx2_hud_destroy(self.hud.as_ptr()) }
        }
    }
}

#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
pub use cafe::Presenter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_layout_normalization_is_independent_of_target_viewport() {
        let vertices = [HudVertex {
            pos: [213.5, 120.0],
            uv: [0.0; 2],
            color: 0,
            depth: 0.0,
            glint_uv: [0.0; 2],
            text: 0,
        }; 3];
        let runs = [(0..3, Blend::Alpha)];
        let frame = Frame {
            vertices: &vertices,
            runs: &runs,
            patches: &[],
            viewport: [854, 480],
            scale: 2.0,
            pack_text_shader: false,
        };

        let prepared = Prepared::new(&frame).unwrap();
        let raw = prepared.raw();
        assert_eq!((raw.viewport_width, raw.viewport_height), (854, 480));
        assert!(validate_target_viewport(frame.viewport, [0, 0, 1920, 1080]).is_ok());
        assert!(validate_target_viewport(frame.viewport, [0, 60, 640, 360]).is_ok());
        assert!(matches!(
            validate_target_viewport(frame.viewport, [0, 0, 640, 480]),
            Err(Error::Unsupported)
        ));
        assert!(matches!(
            validate_target_viewport(frame.viewport, [u32::MAX, 0, 1, 1]),
            Err(Error::Invalid)
        ));

        // Independent expected center in NDC, using the source viewport and
        // scale; target output dimensions do not enter this transform.
        let clip_x = vertices[0].pos[0] * frame.scale / 854.0 * 2.0 - 1.0;
        let clip_y = 1.0 - vertices[0].pos[1] * frame.scale / 480.0 * 2.0;
        assert!(clip_x.abs() < 0.00001);
        assert!(clip_y.abs() < 0.00001);
    }

    #[test]
    fn converts_borrowed_frame_and_rejects_unsupported_blends() {
        let pixels = vec![1, 2, 3, 4];
        let patches = [Patch {
            x: 2,
            y: 3,
            width: 1,
            height: 1,
            rgba: pixels,
        }];
        let vertices = [HudVertex {
            pos: [0.0; 2],
            uv: [0.0; 2],
            color: 0,
            depth: 0.0,
            glint_uv: [0.0; 2],
            text: 0,
        }; 3];
        let runs = [(0..3, Blend::Alpha)];
        let frame = Frame {
            vertices: &vertices,
            runs: &runs,
            patches: &patches,
            viewport: [640, 480],
            scale: 2.0,
            pack_text_shader: false,
        };
        let prepared = Prepared::new(&frame).unwrap();
        let raw = prepared.raw();
        assert_eq!(raw.vertices, vertices.as_ptr());
        assert_eq!(
            (raw.vertex_count, raw.run_count, raw.patch_count),
            (3, 1, 1)
        );
        assert_eq!(
            (
                prepared.runs[0].start,
                prepared.runs[0].end,
                prepared.runs[0].blend
            ),
            (0, 3, 0)
        );
        assert_eq!(prepared.patches[0].rgba, patches[0].rgba.as_ptr());
        assert_eq!(prepared.patches[0].length, 4);
        for blend in [
            Blend::Invert,
            Blend::Depth([0; 4]),
            Blend::Vignette,
            Blend::EndPortal,
            Blend::CitGlint(2),
        ] {
            let unsupported = [(0..3, blend)];
            let f = Frame {
                runs: &unsupported,
                ..frame
            };
            assert!(matches!(Prepared::new(&f), Err(Error::Unsupported)));
        }
        let additive = [(0..3, Blend::Additive)];
        let f = Frame {
            runs: &additive,
            ..frame
        };
        assert_eq!(Prepared::new(&f).unwrap().runs[0].blend, 2);
        let f = Frame {
            pack_text_shader: true,
            ..frame
        };
        assert!(matches!(Prepared::new(&f), Err(Error::Unsupported)));
    }
}
