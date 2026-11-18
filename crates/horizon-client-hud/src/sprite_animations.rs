//! Horizon's existing HUD sprite animation updates, independent of the GPU.

use horizon_client_core::ui_geometry::Rect;
use horizon_hud_data::Patch;
use horizon_terrain_mesh::sprites::SpriteAnimation;

/// An atlas animation and the last frame submitted to its texture.
pub type State = (Rect, SpriteAnimation, (usize, usize, u32));
pub type States = Vec<State>;

/// Track the animations belonging to a newly created atlas texture.
pub fn track(animations: &[(Rect, SpriteAnimation)]) -> States {
    animations
        .iter()
        .map(|(rect, animation)| (*rect, animation.clone(), (usize::MAX, 0, 0)))
        .collect()
}

/// Updates retain their animation states until the backend accepts the patches.
pub struct Update<'a> {
    states: &'a mut [State],
    patches: Vec<Patch>,
    next: Vec<(usize, (usize, usize, u32))>,
}

impl Update<'_> {
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Mark these patches submitted. Dropping without this keeps them retryable.
    pub fn commit(self) {
        for (index, state) in self.next {
            self.states[index].2 = state;
        }
    }
}

/// Compute Horizon's frame selection and interpolation for one atlas tick.
pub fn prepare(states: &mut [State], size: [u32; 2], tick: u64) -> Update<'_> {
    prepare_if(states, size, tick, |_| true)
}

/// Prepare only sprites whose atlas regions can be sampled by this frame.
/// Skipped sprites retain their old state and catch up when next selected.
pub fn prepare_if(
    states: &mut [State],
    size: [u32; 2],
    tick: u64,
    mut selected: impl FnMut(Rect) -> bool,
) -> Update<'_> {
    let [width, height] = size;
    let mut patches = Vec::new();
    let mut next = Vec::new();
    for (index, (rect, animation, previous)) in states.iter().enumerate() {
        // `SpriteContents.AnimationState`: the frame from the mcmeta's
        // list, blended into the next one by the sub-frame's thousandths
        // when the sprite interpolates.
        let blend = animation.blend(tick);
        let state = (blend.frame, blend.next, blend.millis);
        if state == *previous || !selected(*rect) {
            continue;
        }
        let (w, h) = rect.texels((width, height));
        let pixels = if blend.millis == 0 {
            animation.resized_pixels(blend.frame, w, h)
        } else {
            let current = animation.resized_pixels(blend.frame, w, h);
            let next = animation.resized_pixels(blend.next, w, h);
            let millis = blend.millis;
            current
                .iter()
                .zip(next)
                .map(|(&a, b)| {
                    ((u32::from(a) * (1000 - millis) + u32::from(b) * millis + 500) / 1000) as u8
                })
                .collect()
        };
        patches.push(Patch {
            x: (rect.u0 * width as f32).round() as u32,
            y: (rect.v0 * height as f32).round() as u32,
            width: w,
            height: h,
            rgba: pixels,
        });
        next.push((index, state));
    }
    Update {
        states,
        patches,
        next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_terrain_mesh::sprites::test_frames;

    #[test]
    fn animation_patches_interpolate_and_retry_until_committed() {
        let animation = test_frames(
            0,
            vec![vec![0, 0, 0, 255], vec![100, 200, 0, 255]],
            r#"{"frametime":2,"interpolate":true,"frames":[0,1]}"#,
        );
        let rect = Rect {
            u0: 0.25,
            v0: 0.5,
            u1: 0.75,
            v1: 1.0,
            w: 2,
            h: 2,
        };
        let mut states = track(&[(rect, animation)]);
        let update = prepare(&mut states, [4, 4], 0);
        assert_eq!((update.patches()[0].x, update.patches()[0].y), (1, 2));
        assert_eq!(
            (update.patches()[0].width, update.patches()[0].height),
            (2, 2)
        );
        assert_eq!(update.patches()[0].rgba, [0, 0, 0, 255].repeat(4));
        drop(update);
        let retry = prepare(&mut states, [4, 4], 0);
        assert_eq!(retry.patches().len(), 1);
        retry.commit();
        assert!(prepare(&mut states, [4, 4], 0).patches().is_empty());
        let halfway = prepare(&mut states, [4, 4], 1);
        assert_eq!(halfway.patches()[0].rgba, [50, 100, 0, 255].repeat(4));
        halfway.commit();
        let next = prepare(&mut states, [4, 4], 2);
        assert_eq!(next.patches()[0].rgba, [100, 200, 0, 255].repeat(4));
    }
    #[test]
    fn skipped_animation_catches_up_when_selected_and_retries_on_failure() {
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
        let mut states = track(&[(rect, animation)]);
        let skipped = prepare_if(&mut states, [1, 1], 0, |_| false);
        assert!(skipped.patches().is_empty());
        skipped.commit();
        let visible = prepare_if(&mut states, [1, 1], 1, |_| true);
        assert_eq!(visible.patches()[0].rgba, [50, 100, 0, 255]);
        drop(visible);
        let retry = prepare_if(&mut states, [1, 1], 1, |_| true);
        assert_eq!(retry.patches()[0].rgba, [50, 100, 0, 255]);
        retry.commit();
        assert!(
            prepare_if(&mut states, [1, 1], 1, |_| true)
                .patches()
                .is_empty()
        );
    }
}
