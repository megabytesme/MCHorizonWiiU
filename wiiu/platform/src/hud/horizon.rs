//! Adapt Horizon's original CPU HUD to the GX2 presenter.

use horizon_client_hud::{HudAtlas, HudBatch};

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
use super::Error;
use super::{Blend, Frame};
#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
use std::borrow::Cow;
use std::ops::Range;

/// Prepare the original CPU atlas for a new renderer. Horizon's atlas pixels
/// omit glyphs rasterized into an earlier GPU texture, so their cache must be
/// reset before the new renderer is created, as in `build_renderer`.
#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
fn prepare_atlas(atlas: &HudAtlas) -> Cow<'_, [u8]> {
    atlas.font.reset_cache();
    atlas.pixels()
}

#[cfg(any(test, all(feature = "cafe", target_arch = "powerpc")))]
fn prepare_animations(
    states: &mut horizon_client_hud::sprite_animations::States,
    size: [u32; 2],
    tick: u64,
    enabled: bool,
) -> Option<horizon_client_hud::sprite_animations::Update<'_>> {
    if enabled {
        Some(horizon_client_hud::sprite_animations::prepare(
            states, size, tick,
        ))
    } else {
        None
    }
}

/// Borrows the original batch while retaining Horizon's ordered blend runs.
pub struct BatchFrame<'a> {
    batch: &'a HudBatch,
    runs: Vec<(Range<u32>, Blend)>,
    viewport: [u32; 2],
    pack_text_shader: bool,
}

impl<'a> BatchFrame<'a> {
    /// Build a frame for a physical viewport.
    pub fn new(atlas: &HudAtlas, batch: &'a HudBatch, viewport: [u32; 2]) -> Self {
        Self {
            batch,
            runs: batch.runs(),
            viewport,
            pack_text_shader: atlas.text_shader.is_some(),
        }
    }

    /// Borrow the original vertices and patches for one synchronous draw.
    pub fn frame(&self) -> Frame<'_> {
        Frame {
            vertices: &self.batch.vertices,
            runs: &self.runs,
            patches: &self.batch.patches,
            viewport: self.viewport,
            scale: self.batch.scale,
            pack_text_shader: self.pack_text_shader,
        }
    }
}

/// A GX2 presenter initialized from one specific Horizon atlas revision.
#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
pub struct HorizonPresenter<'shader> {
    presenter: super::Presenter<'shader>,
    atlas_revision: u64,
    atlas_size: [u32; 2],
    animations: horizon_client_hud::sprite_animations::States,
}

#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
impl<'shader> HorizonPresenter<'shader> {
    /// Create a presenter from the original Horizon atlas.
    ///
    /// # Safety
    /// GX2 must be initialized and remain valid on this thread until drop.
    /// `gfd` must be an aligned, complete, valid presenter shader GFD and
    /// remain alive until the returned presenter is dropped. Create the
    /// presenter before laying out its first batch: this resets the font cache
    /// and batches from an earlier renderer may contain stale glyph UVs.
    pub unsafe fn new(gfd: &'shader [u8], atlas: &HudAtlas, capacity: u32) -> Result<Self, Error> {
        // `pixels()` can decompress into an owned buffer. GX2 copies those
        // pixels during creation, before the temporary buffer is dropped.
        let pixels = prepare_atlas(atlas);
        let presenter = super::Presenter::new(gfd, atlas.width, atlas.height, &pixels, capacity)?;
        Ok(Self {
            presenter,
            atlas_revision: atlas.revision,
            atlas_size: [atlas.width, atlas.height],
            animations: horizon_client_hud::sprite_animations::track(&atlas.sprite_animations),
        })
    }

    /// Draw one Horizon batch at its current game tick. When Horizon's
    /// `animated_textures` option is enabled, sprite updates are committed
    /// only after the GX2 presenter accepts the frame.
    ///
    /// # Safety
    /// The calling render thread must have a valid GX2 color target bound.
    /// This changes GX2 render state.
    pub unsafe fn draw(
        &mut self,
        atlas: &HudAtlas,
        batch: &HudBatch,
        viewport: [u32; 2],
        tick: u64,
        animated_textures: bool,
    ) -> Result<(), Error> {
        self.draw_inner(atlas, batch, viewport, None, tick, animated_textures)
    }

    /// Draw one original Horizon batch into a target viewport rectangle while
    /// preserving the viewport and scale used to lay out the batch.
    ///
    /// Use the layout viewport passed to `HudBatch::new` as
    /// `layout_viewport`; `target` is `[x, y, width, height]` on the active
    /// GX2 target. Its aspect ratio must match the layout viewport within 0.5%.
    /// A host can letterbox a differently shaped output rectangle. Reuse the
    /// same batch for each output instead of drawing the menu again.
    ///
    /// Animated sprite updates are committed only after this target accepts
    /// the frame, matching [`Self::draw`].
    ///
    /// # Safety
    /// The calling render thread must have a valid GX2 color target bound.
    /// This changes GX2 render state.
    pub unsafe fn draw_to(
        &mut self,
        atlas: &HudAtlas,
        batch: &HudBatch,
        layout_viewport: [u32; 2],
        target: [u32; 4],
        tick: u64,
        animated_textures: bool,
    ) -> Result<(), Error> {
        self.draw_inner(
            atlas,
            batch,
            layout_viewport,
            Some(target),
            tick,
            animated_textures,
        )
    }

    unsafe fn draw_inner(
        &mut self,
        atlas: &HudAtlas,
        batch: &HudBatch,
        layout_viewport: [u32; 2],
        target: Option<[u32; 4]>,
        tick: u64,
        animated_textures: bool,
    ) -> Result<(), Error> {
        if atlas.revision != self.atlas_revision || [atlas.width, atlas.height] != self.atlas_size {
            return Err(Error::Invalid);
        }
        let frame = BatchFrame::new(atlas, batch, layout_viewport);
        let update = prepare_animations(
            &mut self.animations,
            self.atlas_size,
            tick,
            animated_textures,
        );
        if let Some(update) = update {
            let frame = frame.frame();
            match target {
                Some(target) => unsafe {
                    self.presenter
                        .draw_with_patches_to(&frame, update.patches(), target)?;
                },
                None => unsafe {
                    self.presenter.draw_with_patches(&frame, update.patches())?;
                },
            }
            update.commit();
        } else {
            let frame = frame.frame();
            match target {
                Some(target) => unsafe { self.presenter.draw_to(&frame, target)? },
                None => unsafe { self.presenter.draw(&frame)? },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::Prepared;

    #[test]
    fn borrows_horizons_real_batch_and_rejects_unsupported_blends() {
        let atlas = HudAtlas::boot();
        let mut batch = HudBatch::new([640, 480]);
        batch.fill(&atlas, 1.0, 2.0, 3.0, 4.0, 0xff00ffff);
        let view = BatchFrame::new(&atlas, &batch, [640, 480]);
        let frame = view.frame();
        assert_eq!(frame.vertices.as_ptr(), batch.vertices.as_ptr());
        assert_eq!(frame.patches.as_ptr(), batch.patches.as_ptr());
        assert_eq!(frame.viewport, [640, 480]);
        assert_eq!(frame.scale, batch.scale);
        assert_eq!(frame.runs, &[(0..6, Blend::Alpha)]);
        assert!(Prepared::new(&frame).is_ok());

        batch.blend_since(0, Blend::Invert);
        let view = BatchFrame::new(&atlas, &batch, [640, 480]);
        assert!(matches!(
            Prepared::new(&view.frame()),
            Err(Error::Unsupported)
        ));
    }

    #[test]
    fn a_new_presenter_uses_original_pixels_and_resets_font_cache() {
        let mut atlas = HudAtlas::boot();
        let old_epoch = atlas.font.epoch();
        let expected = atlas.pixels().into_owned();
        atlas.pack_pixels();
        assert!(atlas.pixels.is_empty());
        let pixels = prepare_atlas(&atlas);
        assert_eq!(pixels.as_ref(), expected);
        assert!(matches!(pixels, Cow::Owned(_)));
        assert_ne!(atlas.font.epoch(), old_epoch);
    }

    #[test]
    fn animated_patches_precede_the_original_glyph_patches_without_copying() {
        use horizon_hud_data::Patch;

        let atlas = HudAtlas::boot();
        let mut batch = HudBatch::new([640, 480]);
        batch.fill(&atlas, 1.0, 2.0, 3.0, 4.0, 0xffffffff);
        batch.patches.push(Patch {
            x: 2,
            y: 3,
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        });
        let animated = [Patch {
            x: 4,
            y: 5,
            width: 1,
            height: 1,
            rgba: vec![5, 6, 7, 8],
        }];
        let view = BatchFrame::new(&atlas, &batch, [640, 480]);
        let frame = view.frame();
        let prepared = Prepared::with_extra_patches(&frame, &animated).unwrap();
        assert_eq!(prepared.patches.len(), 2);
        assert_eq!(prepared.patches[0].rgba, animated[0].rgba.as_ptr());
        assert_eq!(prepared.patches[1].rgba, batch.patches[0].rgba.as_ptr());
    }

    #[test]
    fn animation_option_preserves_retry_state_when_disabled() {
        use horizon_client_hud::{sprite_animations, Rect};
        use horizon_terrain_mesh::sprites::test_frames;

        let animation = test_frames(
            0,
            vec![vec![0, 0, 0, 255], vec![100, 200, 0, 255]],
            r#"{"frametime":2,"interpolate":true,"frames":[0,1]}"#,
        );
        let rect = Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            w: 1,
            h: 1,
        };
        let mut states = sprite_animations::track(&[(rect, animation)]);
        assert!(prepare_animations(&mut states, [1, 1], 0, false).is_none());
        let first = prepare_animations(&mut states, [1, 1], 0, true).unwrap();
        assert_eq!(first.patches()[0].rgba, [0, 0, 0, 255]);
        drop(first);
        let retry = prepare_animations(&mut states, [1, 1], 0, true).unwrap();
        assert_eq!(retry.patches().len(), 1);
        retry.commit();
        assert!(prepare_animations(&mut states, [1, 1], 2, false).is_none());
        let resumed = prepare_animations(&mut states, [1, 1], 2, true).unwrap();
        assert_eq!(resumed.patches()[0].rgba, [100, 200, 0, 255]);
    }
}
