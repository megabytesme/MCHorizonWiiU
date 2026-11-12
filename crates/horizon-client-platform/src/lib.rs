//! Platform-neutral contracts used by Horizon's client frontends.

pub mod events;
pub mod input;
pub mod logging;
pub mod path;
pub use events::{
    ClientDeviceEvent, ClientWindowEvent, ImeEvent, MouseScrollDelta, PhysicalPosition,
    PhysicalSize, TouchPhase,
};
pub use input::{
    KeyCode, KeyLocation, KeyState, KeyboardEvent, LogicalKey, Modifiers, MouseButton,
};

use std::time::{Duration, Instant};

/// What the game asks of its platform loop after the current step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventLoopControl {
    Exit,
    Poll,
    WaitUntil(Instant),
}

/// Applies game scheduling requests to a platform event loop.
pub trait EventLoopCtl {
    fn set_control_flow(&self, control: EventLoopControl);

    /// Ask the platform to open its native text input for the focused field.
    /// Returns `true` only when the platform accepted the request; callers can
    /// then fall back to Horizon's own on-screen keyboard when it returns false.
    fn open_text_input(&self, _initial_text: &str, _max_utf16_units: usize) -> bool {
        false
    }

    /// Ends the loop after the current iteration.
    fn exit(&self) {
        self.set_control_flow(EventLoopControl::Exit);
    }
}

/// A drawn frame permits polling until the next idle iteration. If the
/// compositor stops delivering redraws, an undrawn frame waits for the next
/// display interval instead of keeping the loop spinning.
pub fn redraw_flow(frame_drawn: &mut bool, offscreen: bool, now: Instant) -> EventLoopControl {
    let drew_frame = std::mem::take(frame_drawn);
    if offscreen || drew_frame {
        EventLoopControl::Poll
    } else {
        EventLoopControl::WaitUntil(now + Duration::from_millis(16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_redraw_waits_and_a_new_frame_resumes_polling() {
        let now = Instant::now();
        let mut frame_drawn = true;
        assert_eq!(
            redraw_flow(&mut frame_drawn, false, now),
            EventLoopControl::Poll
        );
        assert_eq!(
            redraw_flow(&mut frame_drawn, false, now),
            EventLoopControl::WaitUntil(now + Duration::from_millis(16))
        );
        frame_drawn = true;
        assert_eq!(
            redraw_flow(&mut frame_drawn, false, now),
            EventLoopControl::Poll
        );
    }

    #[test]
    fn offscreen_keeps_polling_without_a_presented_frame() {
        assert_eq!(
            redraw_flow(&mut false, true, Instant::now()),
            EventLoopControl::Poll
        );
    }
}
