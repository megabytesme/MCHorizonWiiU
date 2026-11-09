//! Map GamePad touch contacts onto Horizon's existing pointer event path.
//!
//! Pass the current [`VpadState`] each host frame. Cached state makes repeated
//! calls safe when VPAD has no new sample; identical positions produce no
//! duplicate events. Coordinates are calibrated at 1920×1080 and scaled to the
//! actual physical DRC output viewport before Horizon applies GUI scaling.

use horizon_client_platform::{
    ClientWindowEvent, KeyState, MouseButton, MouseScrollDelta, PhysicalPosition, TouchPhase,
};

use crate::{FreshTouchRelease, VpadState, VpadTouch};

const CALIBRATED_WIDTH: f64 = 1920.0;
const CALIBRATED_HEIGHT: f64 = 1080.0;
const GESTURE_THRESHOLD: f64 = 12.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Gesture {
    #[default]
    Idle,
    Pending,
    Scrolling,
    Moved,
}

/// Converts one physical GamePad contact into Horizon mouse events.
pub struct TouchMapper {
    gesture: Gesture,
    focused: bool,
    suppress_until_release: bool,
    contact_seen: bool,
    start_position: Option<PhysicalPosition<f64>>,
    last_position: Option<PhysicalPosition<f64>>,
}

impl Default for TouchMapper {
    fn default() -> Self {
        Self {
            gesture: Gesture::Idle,
            focused: true,
            suppress_until_release: false,
            contact_seen: false,
            start_position: None,
            last_position: None,
        }
    }
}

impl TouchMapper {
    /// Emit pointer events for the current cached VPAD state.
    ///
    /// `viewport` is the physical pixel size of the DRC render target (normally
    /// 854×480). A contact with no valid point does not start a gesture. Short
    /// contacts become taps on release; vertical gestures become wheel events.
    /// Cached no-sample/busy state retains the last pointer position until a
    /// real release or reset. After reset/focus loss, a held contact must be
    /// physically released before it can start another gesture. This
    /// freshness-free helper never re-arms suppression; hardware hosts should
    /// use [`Self::events_with_freshness`] and pass each poll's marker once.
    pub fn events(&mut self, state: &VpadState, viewport: [u32; 2]) -> Vec<ClientWindowEvent> {
        self.events_with_freshness(state, viewport, None)
    }

    /// Process the current state and optional one-shot evidence from this VPAD
    /// poll. Only a fresh sampled release can clear focus/reset suppression.
    ///
    /// Pass the marker returned by `state.update_with_freshness` or
    /// `state.poll_with_freshness` immediately, with that exact `state`, once
    /// per poll and before another poll or reset. Do not retain a marker or pair
    /// it with a replacement state. Pass `None` for no-sample, busy, and reset.
    pub fn events_with_freshness(
        &mut self,
        state: &VpadState,
        viewport: [u32; 2],
        fresh_release: Option<FreshTouchRelease>,
    ) -> Vec<ClientWindowEvent> {
        if fresh_release.is_some() {
            if state.touch_active() {
                // A later contact arrived before this update was consumed.
                self.suppress_until_release = true;
                self.contact_seen = true;
            } else {
                self.suppress_until_release = false;
                self.contact_seen = false;
            }
        }

        if !self.focused {
            self.observe_unfocused(state);
            return Vec::new();
        }

        if self.suppress_until_release {
            return Vec::new();
        }

        if !state.touch_active() {
            if self.contact_seen {
                // An inactive cache/reset is not evidence of a physical release.
                self.suppress_until_release = true;
            }
            return self.finish_contact();
        }
        self.contact_seen = true;

        let Some(point) = state.touch() else {
            return Vec::new();
        };
        let Some(position) = scale_to_viewport(point, viewport) else {
            return Vec::new();
        };

        if self.gesture == Gesture::Idle {
            self.gesture = Gesture::Pending;
            self.start_position = Some(position);
            self.last_position = Some(position);
            return vec![cursor_event(position)];
        }

        let previous = self.last_position.unwrap_or(position);
        if previous == position {
            return Vec::new();
        }
        self.last_position = Some(position);

        let mut events = Vec::with_capacity(3);
        match self.gesture {
            Gesture::Idle => unreachable!("a valid contact starts pending above"),
            Gesture::Pending => {
                let start = self.start_position.unwrap_or(previous);
                let dx = position.x - start.x;
                let dy = position.y - start.y;
                if dy.abs() >= GESTURE_THRESHOLD && dy.abs() > dx.abs() {
                    self.gesture = Gesture::Scrolling;
                    // Emit the full displacement accumulated while the gesture
                    // was undecided, so slow first movements are not lost.
                    events.push(cursor_event(position));
                    events.push(wheel_event(dy));
                } else if dx.abs().max(dy.abs()) >= GESTURE_THRESHOLD {
                    // Horizontal and diagonal movement is not a tap, but it is
                    // not scrolling either. Keep it click-free so a swipe over
                    // an actionable button cannot activate it.
                    self.gesture = Gesture::Moved;
                    events.push(cursor_event(position));
                } else {
                    events.push(cursor_event(position));
                }
            }
            Gesture::Scrolling => {
                events.push(cursor_event(position));
                events.push(wheel_event(position.y - previous.y));
            }
            Gesture::Moved => events.push(cursor_event(position)),
        }
        events
    }

    /// Cancel a pending tap on controller loss or host reset.
    ///
    /// Re-arming requires observing a physical release, preventing a held touch
    /// from becoming a fresh click after a reset.
    pub fn reset(&mut self, state: &VpadState) -> Vec<ClientWindowEvent> {
        let events = self.cancel_contact();
        if self.contact_seen || state.touch_active() {
            self.suppress_until_release = true;
        }
        self.last_position = None;
        self.start_position = None;
        events
    }

    /// Lose focus and release any held click. No pointer events are emitted
    /// until focus returns, and a contact held across the transition is ignored
    /// until it is physically released.
    pub fn focus_lost(&mut self, state: &VpadState) -> Vec<ClientWindowEvent> {
        let events = self.cancel_contact();
        self.focused = false;
        if state.touch_active() || self.contact_seen {
            self.suppress_until_release = true;
            self.contact_seen = true;
        }
        self.last_position = None;
        self.start_position = None;
        events
    }

    /// Restore focus. If the GamePad is still being touched, wait for release
    /// before accepting a new contact.
    pub fn focus_gained(&mut self, state: &VpadState) {
        self.focused = true;
        if state.touch_active() || self.contact_seen {
            self.suppress_until_release = true;
            self.contact_seen = true;
        }
        self.last_position = None;
        self.start_position = None;
    }

    fn observe_unfocused(&mut self, state: &VpadState) {
        if state.touch_active() {
            self.suppress_until_release = true;
            self.contact_seen = true;
        }
    }

    fn finish_contact(&mut self) -> Vec<ClientWindowEvent> {
        let events = match self.gesture {
            Gesture::Pending => vec![
                button_event(KeyState::Pressed),
                button_event(KeyState::Released),
            ],
            Gesture::Scrolling | Gesture::Moved | Gesture::Idle => Vec::new(),
        };
        self.clear_gesture();
        events
    }

    fn cancel_contact(&mut self) -> Vec<ClientWindowEvent> {
        self.clear_gesture();
        Vec::new()
    }

    fn clear_gesture(&mut self) {
        self.gesture = Gesture::Idle;
        self.start_position = None;
        self.last_position = None;
    }
}

fn scale_to_viewport(point: VpadTouch, viewport: [u32; 2]) -> Option<PhysicalPosition<f64>> {
    let [width, height] = viewport;
    if width == 0 || height == 0 || point.x >= 1920 || point.y >= 1080 {
        return None;
    }
    Some(PhysicalPosition::new(
        (f64::from(point.x) * f64::from(width) / CALIBRATED_WIDTH).min(f64::from(width - 1)),
        (f64::from(point.y) * f64::from(height) / CALIBRATED_HEIGHT).min(f64::from(height - 1)),
    ))
}

fn cursor_event(position: PhysicalPosition<f64>) -> ClientWindowEvent {
    ClientWindowEvent::CursorMoved { position }
}

fn button_event(state: KeyState) -> ClientWindowEvent {
    ClientWindowEvent::MouseInput {
        state,
        button: MouseButton::Left,
    }
}

fn wheel_event(delta_y: f64) -> ClientWindowEvent {
    ClientWindowEvent::MouseWheel {
        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, delta_y)),
        phase: TouchPhase::Moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VpadSample;

    fn state(active: u32, valid: u32, x: u16, y: u16) -> VpadState {
        let mut state = VpadState::default();
        state.update(Some(VpadSample {
            touch_active: active,
            touch_valid: valid,
            touch_x: x,
            touch_y: y,
            ..Default::default()
        }));
        state
    }

    fn sampled_state(
        active: u32,
        valid: u32,
        x: u16,
        y: u16,
    ) -> (VpadState, Option<FreshTouchRelease>) {
        let mut state = VpadState::default();
        let (_, release) = state.update_with_freshness(Some(VpadSample {
            touch_active: active,
            touch_valid: valid,
            touch_x: x,
            touch_y: y,
            ..Default::default()
        }));
        (state, release)
    }

    #[test]
    fn start_scales_to_drc_without_activating_until_release() {
        let state = state(1, 1, 960, 540);
        let mut mapper = TouchMapper::default();
        assert_eq!(
            mapper.events(&state, [854, 480]),
            vec![cursor_event(PhysicalPosition::new(427.0, 240.0))]
        );
    }

    #[test]
    fn short_contact_becomes_one_click_on_release() {
        let mut mapper = TouchMapper::default();
        let start = state(1, 1, 200, 300);
        assert_eq!(mapper.events(&start, [854, 480]).len(), 1);
        assert!(mapper.events(&start, [854, 480]).is_empty());

        let moved = state(1, 1, 208, 308);
        assert_eq!(
            mapper.events(&moved, [854, 480]),
            vec![cursor_event(PhysicalPosition::new(
                208.0 * 854.0 / 1920.0,
                308.0 * 480.0 / 1080.0,
            ))]
        );

        let released = state(0, 0, 0, 0);
        assert_eq!(
            mapper.events(&released, [854, 480]),
            vec![
                button_event(KeyState::Pressed),
                button_event(KeyState::Released),
            ]
        );
        assert!(mapper.events(&released, [854, 480]).is_empty());
    }

    #[test]
    fn vertical_swipe_emits_accumulated_wheel_delta_without_clicking() {
        let mut mapper = TouchMapper::default();
        let start = state(1, 1, 500, 500);
        let _ = mapper.events(&start, [854, 480]);

        let first = state(1, 1, 500, 520);
        assert_eq!(mapper.events(&first, [854, 480]).len(), 1);

        let threshold = state(1, 1, 500, 530);
        let y0 = 500.0 * 480.0 / 1080.0;
        let y1 = 530.0 * 480.0 / 1080.0;
        assert_eq!(
            mapper.events(&threshold, [854, 480]),
            vec![
                cursor_event(PhysicalPosition::new(500.0 * 854.0 / 1920.0, y1)),
                wheel_event(y1 - y0),
            ]
        );

        let continued = state(1, 1, 500, 550);
        let y2 = 550.0 * 480.0 / 1080.0;
        assert_eq!(
            mapper.events(&continued, [854, 480]),
            vec![
                cursor_event(PhysicalPosition::new(500.0 * 854.0 / 1920.0, y2)),
                wheel_event(y2 - y1),
            ]
        );
        assert!(mapper.events(&state(0, 0, 0, 0), [854, 480]).is_empty());
    }

    #[test]
    fn horizontal_swipe_cancels_tap_without_pressing_its_origin() {
        let mut mapper = TouchMapper::default();
        let start = state(1, 1, 500, 500);
        let _ = mapper.events(&start, [854, 480]);
        let moved = state(1, 1, 560, 500);
        assert_eq!(
            mapper.events(&moved, [854, 480]),
            vec![cursor_event(PhysicalPosition::new(
                560.0 * 854.0 / 1920.0,
                500.0 * 480.0 / 1080.0,
            ))]
        );
        assert!(mapper.events(&state(0, 0, 0, 0), [854, 480]).is_empty());
    }

    #[test]
    fn invalid_contact_does_not_start_until_coordinates_are_valid() {
        let mut mapper = TouchMapper::default();
        let mut invalid_start = state(1, 0, 0, 0);
        assert!(mapper.events(&invalid_start, [854, 480]).is_empty());

        invalid_start.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 100,
            touch_y: 200,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&invalid_start, [854, 480]).len(), 1);

        // The VpadState retains the last valid point through invalid coordinates.
        invalid_start.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 0,
            ..Default::default()
        }));
        assert!(mapper.events(&invalid_start, [854, 480]).is_empty());

        // Cached state models no-sample/busy: identical repeated calls do not
        // synthesize early taps or extra moves.
        assert!(mapper.events(&invalid_start, [854, 480]).is_empty());
        assert_eq!(
            mapper.events(&state(0, 0, 0, 0), [854, 480]),
            vec![
                button_event(KeyState::Pressed),
                button_event(KeyState::Released),
            ]
        );
    }

    #[test]
    fn zero_sized_viewport_defers_gesture_and_reset_cancels_pending_tap() {
        let mut mapper = TouchMapper::default();
        let contact = state(1, 1, 1919, 1079);
        assert!(mapper.events(&contact, [0, 480]).is_empty());
        assert!(mapper.events(&contact, [854, 0]).is_empty());
        assert_eq!(mapper.events(&contact, [854, 480]).len(), 1);
        assert!(mapper.reset(&contact).is_empty());
        assert!(mapper.reset(&contact).is_empty());
    }

    #[test]
    fn resize_repositions_an_active_contact_and_edges_clamp_inside_drc() {
        let mut mapper = TouchMapper::default();
        let contact = state(1, 1, 1919, 1079);
        assert_eq!(mapper.events(&contact, [854, 480]).len(), 1);
        assert_eq!(
            mapper.events(&contact, [1280, 720]),
            vec![cursor_event(PhysicalPosition::new(1279.0, 719.0))]
        );
    }

    #[test]
    fn held_contact_across_focus_loss_does_not_click_again_until_released() {
        let mut mapper = TouchMapper::default();
        let contact = state(1, 1, 250, 350);
        assert_eq!(mapper.events(&contact, [854, 480]).len(), 1);
        assert!(mapper.focus_lost(&contact).is_empty());

        mapper.focus_gained(&contact);
        assert!(mapper.events(&contact, [854, 480]).is_empty());
        assert!(mapper.events(&contact, [854, 480]).is_empty());

        let (released, release) = sampled_state(0, 0, 0, 0);
        assert!(mapper
            .events_with_freshness(&released, [854, 480], release)
            .is_empty());
        let next_contact = state(1, 1, 600, 700);
        assert_eq!(mapper.events(&next_contact, [854, 480]).len(), 1);
    }

    #[test]
    fn release_while_unfocused_rearms_for_a_new_contact_after_regain() {
        let mut mapper = TouchMapper::default();
        let contact = state(1, 1, 250, 350);
        let _ = mapper.events(&contact, [854, 480]);
        let _ = mapper.focus_lost(&contact);

        let (released, release) = sampled_state(0, 0, 0, 0);
        assert!(mapper
            .events_with_freshness(&released, [854, 480], release)
            .is_empty());
        mapper.focus_gained(&released);

        let next_contact = state(1, 1, 600, 700);
        assert_eq!(mapper.events(&next_contact, [854, 480]).len(), 1);
    }

    #[test]
    fn contacts_started_while_unfocused_are_not_replayed_on_regain() {
        let mut mapper = TouchMapper::default();
        let held = state(1, 1, 250, 350);
        let _ = mapper.events(&held, [854, 480]);
        let _ = mapper.focus_lost(&held);

        let (released, release) = sampled_state(0, 0, 0, 0);
        assert!(mapper
            .events_with_freshness(&released, [854, 480], release)
            .is_empty());
        let contact_while_unfocused = state(1, 1, 600, 700);
        assert!(mapper
            .events(&contact_while_unfocused, [854, 480])
            .is_empty());
        mapper.focus_gained(&contact_while_unfocused);
        assert!(mapper
            .events(&contact_while_unfocused, [854, 480])
            .is_empty());

        let (released_again, release_again) = sampled_state(0, 0, 0, 0);
        assert!(mapper
            .events_with_freshness(&released_again, [854, 480], release_again)
            .is_empty());
        assert_eq!(mapper.events(&held, [854, 480]).len(), 1);
    }

    #[test]
    fn reset_cached_as_no_contact_does_not_clear_focus_loss_suppression() {
        let mut mapper = TouchMapper::default();
        let mut vpad = VpadState::default();
        let (_, release) = vpad.update_with_freshness(Some(VpadSample::default()));
        assert!(release.is_some());
        vpad.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 250,
            touch_y: 350,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&vpad, [854, 480]).len(), 1);
        assert!(mapper.focus_lost(&vpad).is_empty());

        // A controller reset clears the cached held bit but is not a physical
        // release sample, so it must not re-arm the mapper.
        vpad.reset();
        // No-sample and busy results likewise provide no physical release.
        let (_, no_sample_release) = vpad.update_with_freshness(None);
        assert!(no_sample_release.is_none());
        mapper.focus_gained(&vpad);
        assert!(mapper.events(&vpad, [854, 480]).is_empty());
        assert!(mapper.suppress_until_release);

        // Only a fresh VPAD sample that reports no contact clears suppression.
        let (_, fresh_release) = vpad.update_with_freshness(Some(VpadSample::default()));
        assert!(fresh_release.is_some());
        assert!(mapper
            .events_with_freshness(&vpad, [854, 480], fresh_release)
            .is_empty());
        vpad.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 500,
            touch_y: 400,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&vpad, [854, 480]).len(), 1);
    }

    #[test]
    fn replacement_default_state_is_not_a_fresh_release() {
        let mut mapper = TouchMapper::default();
        let mut previous = VpadState::default();
        let _ = previous.update_with_freshness(Some(VpadSample::default()));
        previous.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 200,
            touch_y: 300,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&previous, [854, 480]).len(), 1);
        assert!(mapper.focus_lost(&previous).is_empty());

        // A new default state carries no one-shot release evidence.
        let mut replacement = VpadState::default();
        mapper.focus_gained(&replacement);
        assert!(mapper.events(&replacement, [854, 480]).is_empty());
        let (_, no_sample) = replacement.update_with_freshness(None);
        assert!(no_sample.is_none());
        assert!(mapper.events(&replacement, [854, 480]).is_empty());

        let (_, release) = replacement.update_with_freshness(Some(VpadSample::default()));
        assert!(mapper
            .events_with_freshness(&replacement, [854, 480], release)
            .is_empty());
        replacement.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 400,
            touch_y: 500,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&replacement, [854, 480]).len(), 1);
    }

    #[test]
    fn reset_after_vpad_reset_keeps_tap_suppressed_until_fresh_release() {
        let mut mapper = TouchMapper::default();
        let contact = state(1, 1, 250, 350);
        assert_eq!(mapper.events(&contact, [854, 480]).len(), 1);

        let mut reset_vpad = contact;
        reset_vpad.reset();
        assert!(mapper.reset(&reset_vpad).is_empty());
        mapper.focus_gained(&reset_vpad);
        assert!(mapper.events(&reset_vpad, [854, 480]).is_empty());

        let (_, release) = reset_vpad.update_with_freshness(Some(VpadSample::default()));
        assert!(mapper
            .events_with_freshness(&reset_vpad, [854, 480], release)
            .is_empty());
        reset_vpad.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 500,
            touch_y: 400,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&reset_vpad, [854, 480]).len(), 1);
    }

    #[test]
    fn invalid_contact_seen_before_reset_also_requires_a_fresh_release() {
        let mut mapper = TouchMapper::default();
        let invalid_contact = state(1, 0, 0, 0);
        assert!(mapper.events(&invalid_contact, [854, 480]).is_empty());

        let mut reset_vpad = invalid_contact;
        reset_vpad.reset();
        assert!(mapper.reset(&reset_vpad).is_empty());
        let _ = mapper.focus_lost(&reset_vpad);
        mapper.focus_gained(&reset_vpad);
        assert!(mapper.events(&reset_vpad, [854, 480]).is_empty());

        let (_, release) = reset_vpad.update_with_freshness(Some(VpadSample::default()));
        assert!(mapper
            .events_with_freshness(&reset_vpad, [854, 480], release)
            .is_empty());
        reset_vpad.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 1,
            touch_x: 500,
            touch_y: 400,
            ..Default::default()
        }));
        assert_eq!(mapper.events(&reset_vpad, [854, 480]).len(), 1);
    }
}
