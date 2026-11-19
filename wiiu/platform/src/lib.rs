//! CafeOS input adapter for Horizon's existing controller processing.
//!
//! The host feeds the returned `PadState` to Horizon; bindings, button edges,
//! deadzones, menu navigation and Nintendo letter swapping remain in Horizon.

use horizon_client_input::gamepad::{Button, PadState};
use std::collections::VecDeque;

#[cfg(all(feature = "cafe", target_arch = "powerpc"))]
pub mod audio;
pub mod hud;
pub mod keyboard;
pub mod mouse;
pub mod touch;

/// The scalar subset read from WUT's `VPADStatus` by `vpad_bridge.c`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VpadSample {
    pub buttons: u32,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
    /// Calibrated 1920×1080 GamePad screen coordinates.
    pub touch_x: u16,
    pub touch_y: u16,
    /// Physical touch contact, independent of coordinate validity.
    pub touch_active: u32,
    /// Nonzero only when calibrated coordinates are valid and in range.
    pub touch_valid: u32,
}

pub const VPAD_SAMPLE_CAPACITY: usize = 16;
pub const VPAD_HISTORY_CAPACITY: usize = VPAD_SAMPLE_CAPACITY * 3;
pub const BUTTON_HISTORY_QUEUE_CAPACITY: usize = 128;

/// Bounded replay queue for physical controller button-state transitions.
/// History carries no sample freshness or touch evidence.
pub struct ButtonHistory {
    masks: VecDeque<u32>,
}

impl Default for ButtonHistory {
    fn default() -> Self {
        Self {
            masks: VecDeque::with_capacity(BUTTON_HISTORY_QUEUE_CAPACITY),
        }
    }
}

impl ButtonHistory {
    /// Queue one poll's ordered masks, or clear history while input is gated.
    /// Returns false on overflow; that path clears pending masks fail-closed.
    pub fn push_poll(&mut self, masks: &[u32], keyboard_owned: bool, focused: bool) -> bool {
        if keyboard_owned || !focused {
            self.clear();
            return true;
        }
        if self.masks.len().saturating_add(masks.len()) > BUTTON_HISTORY_QUEUE_CAPACITY {
            self.clear();
            return false;
        }
        self.masks.extend(masks.iter().copied());
        true
    }

    pub fn pop_front(&mut self) -> Option<u32> {
        self.masks.pop_front()
    }

    pub fn clear(&mut self) {
        self.masks.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.masks.len()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct VpadBatch {
    latest: VpadSample,
    transition_count: u32,
    transitions: [u32; VPAD_HISTORY_CAPACITY],
}

impl Default for VpadBatch {
    fn default() -> Self {
        Self {
            latest: VpadSample::default(),
            transition_count: 0,
            transitions: [0; VPAD_HISTORY_CAPACITY],
        }
    }
}

#[derive(Debug)]
pub struct VpadPoll {
    pub pad: PadState,
    pub fresh_sample: Option<FreshVpadSample>,
    pub fresh_touch_release: Option<FreshTouchRelease>,
    /// Ordered button masks from physical VPAD samples in this poll batch.
    /// These are historical controller states, not fresh poll evidence.
    pub button_history: [u32; VPAD_HISTORY_CAPACITY],
    pub button_history_len: usize,
}

// This type is passed to the WUT bridge compiled from vpad_bridge.h.
const _: () = {
    assert!(std::mem::size_of::<VpadBatch>() == 228);
    assert!(std::mem::size_of::<VpadSample>() == 32);
    assert!(std::mem::offset_of!(VpadSample, touch_x) == 20);
    assert!(std::mem::offset_of!(VpadSample, touch_active) == 24);
    assert!(std::mem::offset_of!(VpadSample, touch_valid) == 28);
};

/// A calibrated contact on the Wii U GamePad's 1920×1080 touch surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VpadTouch {
    pub x: u16,
    pub y: u16,
}

/// Non-cloneable evidence that one successful VPAD sample reported no contact.
/// Default state, reset, no-sample and busy polling cannot produce this marker.
///
/// Treat it as paired with the exact `VpadState` updated by that poll: pass it
/// immediately to `TouchMapper::events_with_freshness` with that same state,
/// before resetting the state or polling it again. Do not retain or reuse it.
#[derive(Debug)]
pub struct FreshTouchRelease(());

/// Non-cloneable evidence that one VPAD update received a successful hardware
/// sample. Unlike [`FreshTouchRelease`], this evidence says nothing about
/// touchscreen contact; controller rearming may use a fresh neutral pad sample
/// even while touch remains active.
///
/// Pair it with the exact `VpadState` updated by the same call and consume it
/// immediately. A reset, cached/default state, no-sample result, busy poll, or
/// terminal poll error cannot produce this marker.
#[derive(Debug)]
pub struct FreshVpadSample(());

/// Convert WUT VPAD button positions and stick axes to Horizon's pad state.
pub fn pad_state(sample: VpadSample) -> PadState {
    let mut state = PadState {
        left: [sample.left_x, sample.left_y],
        right: [sample.right_x, sample.right_y],
        triggers: [
            if sample.buttons & 0x0080 != 0 {
                1.0
            } else {
                0.0
            },
            if sample.buttons & 0x0040 != 0 {
                1.0
            } else {
                0.0
            },
        ],
        ..PadState::default()
    };
    // WUT's VPAD_BUTTON_* values; Horizon identifies face buttons by position.
    for (mask, button) in [
        (0x4000, Button::South), // B
        (0x8000, Button::East),  // A
        (0x1000, Button::West),  // Y
        (0x2000, Button::North), // X
        (0x0020, Button::LeftBumper),
        (0x0010, Button::RightBumper),
        (0x0004, Button::Select),
        (0x0008, Button::Start),
        (0x0004_0000, Button::LeftStick),
        (0x0002_0000, Button::RightStick),
        (0x0200, Button::DPadUp),
        (0x0100, Button::DPadDown),
        (0x0800, Button::DPadLeft),
        (0x0400, Button::DPadRight),
    ] {
        state.set(button, sample.buttons & mask != 0);
    }
    state
}

/// Rebuild a queued button state while retaining the newest real poll's axes.
pub fn pad_state_with_button_mask(latest: PadState, buttons: u32) -> PadState {
    pad_state(VpadSample {
        buttons,
        left_x: latest.left[0],
        left_y: latest.left[1],
        right_x: latest.right[0],
        right_y: latest.right[1],
        ..Default::default()
    })
}

/// Rearm a blocked controller only on a real neutral sample.
pub fn rearm_on_fresh_neutral(blocked: &mut bool, fresh_sample: bool, pad: PadState) -> bool {
    let neutral = pad.buttons == 0 && pad.triggers.iter().all(|value| *value == 0.0);
    if *blocked && fresh_sample && neutral {
        *blocked = false;
        true
    } else {
        false
    }
}

/// Retains the last state when VPAD has no new sample in a frame.
///
/// A missing or busy sample is not a release event: the last valid touch is
/// retained until VPAD reports a new sample, the host resets state, or polling
/// reports a terminal error. While contact remains active, invalid coordinates
/// preserve the last valid point; a point is `None` until one has been observed.
#[derive(Default)]
pub struct VpadState {
    pad: PadState,
    touch_active: bool,
    touch: Option<VpadTouch>,
}

impl VpadState {
    pub fn update(&mut self, sample: Option<VpadSample>) -> PadState {
        self.update_with_freshness(sample).0
    }

    /// Update state and return one-shot evidence when this call received a
    /// fresh successful sample that reports physical release.
    ///
    /// Consume the returned marker immediately with this same state in
    /// `TouchMapper::events_with_freshness`. Do not carry it across another
    /// update, a reset, or a replacement `VpadState`.
    pub fn update_with_freshness(
        &mut self,
        sample: Option<VpadSample>,
    ) -> (PadState, Option<FreshTouchRelease>) {
        let (pad, _fresh_sample, release) = self.update_with_evidence(sample);
        (pad, release)
    }

    /// Update the cached VPAD state and return separate evidence for a real
    /// successful hardware sample and for a fresh touchscreen release.
    ///
    /// The sample marker is present for every `Some(VpadSample)`, including
    /// when the controller is neutral but the touchscreen is held. The touch
    /// marker remains present only when that sample reports no touch contact.
    pub fn update_with_evidence(
        &mut self,
        sample: Option<VpadSample>,
    ) -> (PadState, Option<FreshVpadSample>, Option<FreshTouchRelease>) {
        let mut release = None;
        let fresh_sample = sample.map(|_| FreshVpadSample(()));
        if let Some(sample) = sample {
            self.pad = pad_state(sample);
            self.touch_active = sample.touch_active != 0;
            if !self.touch_active {
                release = Some(FreshTouchRelease(()));
                self.touch = None;
            } else if sample.touch_valid != 0 && sample.touch_x < 1920 && sample.touch_y < 1080 {
                self.touch = Some(VpadTouch {
                    x: sample.touch_x,
                    y: sample.touch_y,
                });
            }
        }
        (self.pad, fresh_sample, release)
    }

    /// Whether the latest VPAD sample reports physical contact.
    ///
    /// Retained when VPAD has no sample or is busy; cleared on a real release/reset.
    pub fn touch_active(&self) -> bool {
        self.touch_active
    }

    /// Last valid calibrated point; it can be stale while coordinates are invalid.
    /// it is cleared only on physical release/reset. `None` means no point is known.
    pub fn touch(&self) -> Option<VpadTouch> {
        self.touch
    }

    /// Clear held controls when the host loses the controller.
    pub fn reset(&mut self) -> PadState {
        self.pad = PadState::default();
        self.touch_active = false;
        self.touch = None;
        self.pad
    }

    /// Read VPAD channel 0. On a non-busy error the host must feed a neutral
    /// state to Horizon; the cached state is cleared before returning the error.
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    pub fn poll(&mut self) -> Result<PadState, i32> {
        self.poll_with_freshness().map(|(pad, _release)| pad)
    }

    /// Read VPAD once and return one-shot release evidence with the updated pad.
    /// No-sample retains state and returns no marker; busy and terminal errors
    /// retain the existing error API, and terminal errors still reset state.
    ///
    /// Consume a returned marker immediately with this same `VpadState` and
    /// poll, before calling this method or `reset` again. `Err(-4)` (busy) and
    /// any terminal error provide no release marker.
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    pub fn poll_with_freshness(&mut self) -> Result<(PadState, Option<FreshTouchRelease>), i32> {
        self.poll_with_evidence()
            .map(|poll| (poll.pad, poll.fresh_touch_release))
    }

    /// Read VPAD and return one-shot evidence for any newly read hardware
    /// sample, independently of the touchscreen release marker. `Ok` with
    /// `None` sample evidence means VPAD supplied no new sample; busy and
    /// terminal errors return `Err` and also provide no evidence.
    #[cfg(all(feature = "cafe", target_arch = "powerpc"))]
    pub fn poll_with_evidence(&mut self) -> Result<VpadPoll, i32> {
        extern "C" {
            fn horizon_wiiu_vpad_read(batch: *mut VpadBatch) -> i32;
        }
        let mut batch = VpadBatch::default();
        // The C bridge writes the sample only on success and owns VPADStatus's ABI.
        match unsafe { horizon_wiiu_vpad_read(&mut batch) } {
            1 => {
                let (pad, fresh_sample, fresh_touch_release) =
                    self.update_with_evidence(Some(batch.latest));
                let button_history_len =
                    (batch.transition_count as usize).min(VPAD_HISTORY_CAPACITY);
                Ok(VpadPoll {
                    pad,
                    fresh_sample,
                    fresh_touch_release,
                    button_history: batch.transitions,
                    button_history_len,
                })
            }
            -1 => {
                let (pad, fresh_sample, fresh_touch_release) = self.update_with_evidence(None);
                Ok(VpadPoll {
                    pad,
                    fresh_sample,
                    fresh_touch_release,
                    button_history: [0; VPAD_HISTORY_CAPACITY],
                    button_history_len: 0,
                })
            }
            // A busy channel can be retried without releasing held controls or touch.
            -4 => Err(-4),
            error => {
                self.reset();
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_wii_u_controls_to_horizons_positional_pad() {
        let state = pad_state(VpadSample {
            buttons: 0x4000 | 0x8000 | 0x0020 | 0x0040 | 0x0200 | 0x0002_0000,
            left_x: -0.5,
            left_y: 0.75,
            right_x: 0.25,
            right_y: -1.0,
            ..Default::default()
        });
        assert!(state.held(Button::South));
        assert!(state.held(Button::East));
        assert!(state.held(Button::LeftBumper));
        assert!(state.held(Button::DPadUp));
        assert!(state.held(Button::RightStick));
        assert!(!state.held(Button::North));
        assert_eq!(state.triggers, [0.0, 1.0]);
        assert_eq!(state.left, [-0.5, 0.75]);
        assert_eq!(state.right, [0.25, -1.0]);
    }

    #[test]
    fn button_history_preserves_order_and_uses_latest_axes() {
        let mut history = ButtonHistory::default();
        assert!(history.push_poll(&[0x4000, 0, 0x4000, 0], false, true));
        assert_eq!(history.pop_front(), Some(0x4000));
        assert_eq!(history.pop_front(), Some(0));
        assert_eq!(history.pop_front(), Some(0x4000));
        assert_eq!(history.pop_front(), Some(0));
        assert_eq!(history.pop_front(), None);

        let latest = pad_state(VpadSample {
            left_x: 0.5,
            left_y: -0.25,
            ..Default::default()
        });
        let replayed = pad_state_with_button_mask(latest, 0x4000);
        assert!(replayed.held(Button::South));
        assert_eq!(replayed.left, [0.5, -0.25]);
    }

    #[test]
    fn button_history_fails_closed_on_overflow_and_clears_at_keyboard_or_focus_gates() {
        let mut history = ButtonHistory::default();
        assert!(history.push_poll(&[0x4000, 0], false, true));
        let overflow = vec![0; BUTTON_HISTORY_QUEUE_CAPACITY];
        assert!(!history.push_poll(&overflow, false, true));
        assert_eq!(history.len(), 0);

        assert!(history.push_poll(&[0x4000], false, true));
        assert!(history.push_poll(&[0], true, true));
        assert_eq!(history.len(), 0);
        assert!(history.push_poll(&[0x4000], false, true));
        assert!(history.push_poll(&[0], false, false));
        assert_eq!(history.len(), 0);
        assert!(history.push_poll(&[0], false, true));
        assert_eq!(history.pop_front(), Some(0));
    }

    #[test]
    fn replaying_button_history_does_not_create_fresh_sample_evidence() {
        let mut history = ButtonHistory::default();
        assert!(history.push_poll(&[0x4000, 0], false, true));
        assert_eq!(history.pop_front(), Some(0x4000));
        assert_eq!(history.pop_front(), Some(0));

        let mut state = VpadState::default();
        let (_, fresh_sample, fresh_release) = state.update_with_evidence(None);
        assert!(fresh_sample.is_none());
        assert!(fresh_release.is_none());
    }

    #[test]
    fn overflow_neutralizes_immediately_and_rearms_only_on_fresh_neutral() {
        let mut history = ButtonHistory::default();
        assert!(history.push_poll(&[0x4000], false, true));
        let overflow = vec![0; BUTTON_HISTORY_QUEUE_CAPACITY];
        assert!(!history.push_poll(&overflow, false, true));
        assert_eq!(history.pop_front(), None);

        let mut blocked = true;
        let held = pad_state(VpadSample {
            buttons: 0x4000,
            ..Default::default()
        });
        let neutral = PadState::default();

        // The overflow caller dispatches PadState::default immediately; the
        // gate remains blocked while this latest physical sample is held.
        assert!(!rearm_on_fresh_neutral(&mut blocked, true, held));
        assert!(blocked);
        assert!(!rearm_on_fresh_neutral(&mut blocked, false, neutral));
        assert!(blocked);
        assert!(rearm_on_fresh_neutral(&mut blocked, true, neutral));
        assert!(!blocked);
    }

    #[test]
    fn no_new_sample_keeps_buttons_held_until_a_release_sample() {
        let mut reader = VpadState::default();
        assert!(reader
            .update(Some(VpadSample {
                buttons: 0x4000,
                ..Default::default()
            }))
            .held(Button::South));
        assert!(reader.update(None).held(Button::South));
        assert!(!reader
            .update(Some(VpadSample::default()))
            .held(Button::South));
    }

    #[test]
    fn reset_releases_a_previously_held_button() {
        let mut reader = VpadState::default();
        reader.update(Some(VpadSample {
            buttons: 0x4000,
            ..Default::default()
        }));
        assert!(!reader.reset().held(Button::South));
        assert!(!reader.update(None).held(Button::South));
    }

    #[test]
    fn calibrated_touch_is_retained_without_a_sample_and_cleared_on_release_or_reset() {
        let mut reader = VpadState::default();
        let contact = VpadSample {
            touch_x: 1919,
            touch_y: 1079,
            touch_active: 1,
            touch_valid: 1,
            ..Default::default()
        };
        reader.update(Some(contact));
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), Some(VpadTouch { x: 1919, y: 1079 }));
        reader.update(None);
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), Some(VpadTouch { x: 1919, y: 1079 }));
        reader.update(Some(VpadSample::default()));
        assert!(!reader.touch_active());
        assert_eq!(reader.touch(), None);

        reader.update(Some(contact));
        reader.reset();
        assert_eq!(reader.touch(), None);
    }

    #[test]
    fn fresh_sample_evidence_is_independent_of_touch_release() {
        let mut state = VpadState::default();
        let contact = VpadSample {
            buttons: 0,
            touch_active: 1,
            touch_valid: 0,
            ..Default::default()
        };
        let (pad, fresh_sample, fresh_touch_release) = state.update_with_evidence(Some(contact));
        assert_eq!(pad, PadState::default());
        assert!(fresh_sample.is_some());
        assert!(fresh_touch_release.is_none());

        let (cached, no_sample, no_release) = state.update_with_evidence(None);
        assert_eq!(cached, PadState::default());
        assert!(no_sample.is_none());
        assert!(no_release.is_none());
        assert_eq!(state.reset(), PadState::default());
        let (reset_state, no_reset_evidence, _) = state.update_with_evidence(None);
        assert_eq!(reset_state, PadState::default());
        assert!(no_reset_evidence.is_none());
    }

    #[test]
    fn invalid_touch_coordinates_are_never_exposed() {
        let mut reader = VpadState::default();
        reader.update(Some(VpadSample {
            touch_x: 500,
            touch_y: 400,
            touch_active: 1,
            touch_valid: 0,
            ..Default::default()
        }));
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), None);
    }

    #[test]
    fn touch_coordinates_must_be_inside_the_calibrated_screen() {
        let mut reader = VpadState::default();
        for (x, y) in [(1920, 1079), (1919, 1080)] {
            reader.update(Some(VpadSample {
                touch_x: x,
                touch_y: y,
                touch_active: 1,
                touch_valid: 1,
                ..Default::default()
            }));
            assert!(reader.touch_active());
            assert_eq!(
                reader.touch(),
                None,
                "accepted out-of-range point ({x}, {y})"
            );
        }

        reader.update(Some(VpadSample {
            touch_x: 1919,
            touch_y: 1079,
            touch_active: 1,
            touch_valid: 1,
            ..Default::default()
        }));
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), Some(VpadTouch { x: 1919, y: 1079 }));
    }

    #[test]
    fn invalid_coordinates_do_not_synthesize_release_between_valid_points() {
        let mut reader = VpadState::default();
        reader.update(Some(VpadSample {
            touch_x: 100,
            touch_y: 200,
            touch_active: 1,
            touch_valid: 1,
            ..Default::default()
        }));
        reader.update(Some(VpadSample {
            touch_active: 1,
            touch_valid: 0,
            ..Default::default()
        }));
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), Some(VpadTouch { x: 100, y: 200 }));

        reader.update(Some(VpadSample {
            touch_x: 300,
            touch_y: 400,
            touch_active: 1,
            touch_valid: 1,
            ..Default::default()
        }));
        assert!(reader.touch_active());
        assert_eq!(reader.touch(), Some(VpadTouch { x: 300, y: 400 }));

        reader.update(Some(VpadSample::default()));
        assert!(!reader.touch_active());
        assert_eq!(reader.touch(), None);
    }
}
