//! Cafe `nsyshid` boot-protocol mouse reports translated to Horizon events.

use horizon_client_platform::{
    ClientDeviceEvent, ClientWindowEvent, KeyState, MouseButton, MouseScrollDelta,
    TouchPhase,
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawMouseEvent {
    kind: u8,
    buttons: u8,
    x: i8,
    y: i8,
    wheel: i8,
    reserved: [u8; 3],
}

const _: () = assert!(core::mem::size_of::<RawMouseEvent>() == 8);

const REPORT: u8 = 1;
const DETACHED: u8 = 2;

unsafe extern "C" {
    fn horizon_wiiu_mouse_init() -> i32;
    fn horizon_wiiu_mouse_teardown() -> i32;
    fn horizon_wiiu_mouse_pop(event: *mut RawMouseEvent) -> i32;
    fn horizon_wiiu_mouse_dropped() -> u32;
}

/// Initialize `nsyshid` and register the boot-mouse event callbacks.
pub fn initialize() -> Result<(), i32> {
    match unsafe { horizon_wiiu_mouse_init() } {
        0 => Ok(()),
        error => Err(error),
    }
}

/// Stop `nsyshid` and unregister the HID client.
pub fn shutdown() -> i32 {
    unsafe { horizon_wiiu_mouse_teardown() }
}

/// One translated event in the original report order.
pub enum MouseOutput {
    Device(ClientDeviceEvent),
    Window(ClientWindowEvent),
}

/// Drains reports into a reusable ordered event vector, releasing held buttons
/// on detach or queue overflow.
#[derive(Default)]
pub struct MouseAdapter {
    buttons: u8,
    observed_drops: u32,
}

impl MouseAdapter {
    pub fn release_all_into(&mut self, output: &mut Vec<MouseOutput>) {
        self.release_buttons(output);
    }

    pub fn drain_into(&mut self, output: &mut Vec<MouseOutput>) {
        let dropped = unsafe { horizon_wiiu_mouse_dropped() };
        if dropped != self.observed_drops {
            self.observed_drops = dropped;
            self.release_buttons(output);
            let mut ignored = RawMouseEvent::default();
            while unsafe { horizon_wiiu_mouse_pop(&mut ignored) } != 0 {}
            return;
        }

        loop {
            let mut raw = RawMouseEvent::default();
            if unsafe { horizon_wiiu_mouse_pop(&mut raw) } == 0 {
                break;
            }
            match raw.kind {
                REPORT => self.report(raw, output),
                DETACHED => self.release_buttons(output),
                _ => {}
            }
        }
    }

    fn report(&mut self, raw: RawMouseEvent, output: &mut Vec<MouseOutput>) {
        let delta = (f64::from(raw.x), f64::from(raw.y));
        if delta != (0.0, 0.0) {
            output.push(MouseOutput::Device(ClientDeviceEvent::MouseMotion { delta }));
        }
        self.set_buttons(raw.buttons & 0x07, output);
        if raw.wheel != 0 {
            output.push(MouseOutput::Window(ClientWindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(0.0, f32::from(raw.wheel)),
                phase: TouchPhase::Moved,
            }));
        }
    }

    fn set_buttons(&mut self, next: u8, output: &mut Vec<MouseOutput>) {
        for (bit, button) in [
            (0, MouseButton::Left),
            (1, MouseButton::Right),
            (2, MouseButton::Middle),
        ] {
            let mask = 1 << bit;
            if (self.buttons & mask) != (next & mask) {
                output.push(MouseOutput::Window(ClientWindowEvent::MouseInput {
                    state: if next & mask != 0 {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    },
                    button,
                }));
            }
        }
        self.buttons = next;
    }

    fn release_buttons(&mut self, output: &mut Vec<MouseOutput>) {
        self.set_buttons(0, output);
    }
}
