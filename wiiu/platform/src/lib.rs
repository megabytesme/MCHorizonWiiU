//! CafeOS input adapter for Horizon's existing controller processing.
//!
//! The host feeds the returned `PadState` to Horizon; bindings, button edges,
//! deadzones, menu navigation and Nintendo letter swapping remain in Horizon.

use horizon_client_input::gamepad::{Button, PadState};

pub mod hud;
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

// This type is passed to the WUT bridge compiled from vpad_bridge.h.
const _: () = {
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
        let mut release = None;
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
        (self.pad, release)
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
        extern "C" {
            fn horizon_wiiu_vpad_read(sample: *mut VpadSample) -> i32;
        }
        let mut sample = VpadSample::default();
        // The C bridge writes the sample only on success and owns VPADStatus's ABI.
        match unsafe { horizon_wiiu_vpad_read(&mut sample) } {
            1 => Ok(self.update_with_freshness(Some(sample))),
            -1 => Ok(self.update_with_freshness(None)),
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
