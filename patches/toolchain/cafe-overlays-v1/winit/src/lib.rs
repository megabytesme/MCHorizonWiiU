//! Small Cafe compile facade for the Winit surface Horizon uses on Wii U.
//! It deliberately has no platform backend; the native Cafe host owns input,
//! presentation and the application loop.

pub use dpi;
pub use raw_window_handle;

pub mod dpi_types {
    pub use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
}

pub mod event_loop {
    use std::{cell::Cell, marker::PhantomData, time::Instant};
    use crate::{error::EventLoopError, window::{Window, WindowAttributes}};

    #[derive(Clone, Copy, Debug)]
    pub enum ControlFlow { Poll, Wait, WaitUntil(Instant) }

    pub struct ActiveEventLoop { exiting: Cell<bool>, _private: PhantomData<()> }
    impl ActiveEventLoop {
        pub fn create_window(&self, _attributes: WindowAttributes) -> Result<Window, EventLoopError> { Ok(Window::new()) }
        pub fn set_control_flow(&self, _control: ControlFlow) {}
        pub fn exit(&self) { self.exiting.set(true); }
    }

    pub struct EventLoop<T = ()>(PhantomData<T>);
    impl<T> EventLoop<T> {
        pub fn new() -> Result<Self, EventLoopError> { Ok(Self(PhantomData)) }
        pub fn run_app<A: crate::application::ApplicationHandler<T>>(&self, app: &mut A) -> Result<(), EventLoopError> {
            let active = ActiveEventLoop { exiting: Cell::new(false), _private: PhantomData };
            app.resumed(&active);
            while !active.exiting.get() { app.about_to_wait(&active); }
            app.exiting(&active);
            Ok(())
        }
    }
}

pub mod application {
    use crate::{event::{DeviceEvent, WindowEvent}, event_loop::ActiveEventLoop, window::WindowId};
    pub trait ApplicationHandler<T = ()> {
        fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}
        fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}
        fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: crate::event::DeviceId, _event: DeviceEvent) {}
        fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
        fn exiting(&mut self, _event_loop: &ActiveEventLoop) {}
    }
}

pub mod error {
    use std::fmt;
    #[derive(Debug)]
    pub enum EventLoopError { NotSupported, ExitFailure(String) }
    impl fmt::Display for EventLoopError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
    impl std::error::Error for EventLoopError {}
    #[derive(Debug)]
    pub struct NotSupportedError;
    impl fmt::Display for NotSupportedError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("operation is unsupported on Cafe") } }
    impl std::error::Error for NotSupportedError {}
}

pub mod window {
    use crate::{dpi::{PhysicalPosition, PhysicalSize}, monitor::MonitorHandle};
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct WindowId(u32);
    #[derive(Clone, Copy, Debug)] pub enum CursorGrabMode { None, Confined, Locked }
    #[derive(Clone, Copy, Debug)] pub enum CursorIcon { Default, Pointer, NotAllowed }
    #[derive(Clone, Debug)] pub enum Fullscreen { Borderless(Option<MonitorHandle>), Exclusive(crate::monitor::VideoModeHandle) }
    pub struct Icon;
    impl Icon { pub fn from_rgba(_rgba: Vec<u8>, _width: u32, _height: u32) -> Result<Self, String> { Err("icons are unsupported on Cafe".into()) } }

    #[derive(Default)] pub struct WindowAttributes { pub(crate) title: String, pub(crate) visible: bool, pub(crate) decorations: bool }
    impl WindowAttributes {
        pub fn with_title(mut self, value: impl Into<String>) -> Self { self.title = value.into(); self }
        pub fn with_visible(mut self, value: bool) -> Self { self.visible = value; self }
        pub fn with_decorations(mut self, value: bool) -> Self { self.decorations = value; self }
        pub fn with_active(self, _active: bool) -> Self { self }
        pub fn with_inner_size<S: Into<crate::dpi::Size>>(self, _size: S) -> Self { self }
        pub fn with_position<P: Into<crate::dpi::Position>>(self, _position: P) -> Self { self }
        pub fn with_fullscreen(self, _fullscreen: Option<Fullscreen>) -> Self { self }
        pub fn with_window_icon(self, _icon: Option<Icon>) -> Self { self }
    }

    #[derive(Debug)]
    pub struct Window { id: WindowId, size: PhysicalSize<u32>, decorated: bool }
    impl Window {
        pub(crate) fn new() -> Self { Self { id: WindowId(NEXT_ID.fetch_add(1, Ordering::Relaxed)), size: PhysicalSize::new(1280, 720), decorated: false } }
        pub fn default_attributes() -> WindowAttributes { WindowAttributes::default() }
        pub fn id(&self) -> WindowId { self.id }
        pub fn inner_size(&self) -> PhysicalSize<u32> { self.size }
        pub fn scale_factor(&self) -> f64 { 1.0 }
        pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, crate::error::NotSupportedError> { Err(crate::error::NotSupportedError) }
        pub fn request_inner_size<S: Into<crate::dpi::Size>>(&self, _size: S) -> Option<PhysicalSize<u32>> { None }
        pub fn set_outer_position<P: Into<crate::dpi::Position>>(&self, _position: P) {}
        pub fn current_monitor(&self) -> Option<MonitorHandle> { None }
        pub fn primary_monitor(&self) -> Option<MonitorHandle> { None }
        pub fn available_monitors(&self) -> std::iter::Empty<MonitorHandle> { std::iter::empty() }
        pub fn fullscreen(&self) -> Option<Fullscreen> { None }
        pub fn is_decorated(&self) -> bool { self.decorated }
        pub fn is_minimized(&self) -> Option<bool> { Some(false) }
        pub fn set_title(&self, _title: &str) { crate::unsupported_log("Window::set_title"); }
        pub fn set_decorations(&self, _decorated: bool) { crate::unsupported_log("Window::set_decorations"); }
        pub fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) { crate::unsupported_log("Window::set_fullscreen"); }
        pub fn set_cursor(&self, _cursor: CursorIcon) { crate::unsupported_log("Window::set_cursor"); }
        pub fn set_cursor_grab(&self, _mode: CursorGrabMode) -> Result<(), String> { crate::unsupported_log("Window::set_cursor_grab"); Err("cursor capture is unsupported on Cafe".into()) }
        pub fn set_cursor_position<P: Into<crate::dpi::Position>>(&self, _position: P) -> Result<(), String> { crate::unsupported_log("Window::set_cursor_position"); Err("cursor positioning is unsupported on Cafe".into()) }
        pub fn set_cursor_visible(&self, _visible: bool) { crate::unsupported_log("Window::set_cursor_visible"); }
        pub fn set_visible(&self, _visible: bool) { crate::unsupported_log("Window::set_visible"); }
        pub fn focus_window(&self) { crate::unsupported_log("Window::focus_window"); }
        pub fn request_redraw(&self) {}
        pub fn set_ime_allowed(&self, _allowed: bool) { crate::unsupported_log("Window::set_ime_allowed"); }
        pub fn set_ime_cursor_area(&self, _position: PhysicalPosition<i32>, _size: PhysicalSize<u32>) { crate::unsupported_log("Window::set_ime_cursor_area"); }
        pub fn pre_present_notify(&self) {}
    }
    impl From<WindowId> for u64 { fn from(id: WindowId) -> Self { u64::from(id.0) } }
    impl Drop for Window { fn drop(&mut self) { crate::cleanup_window(self.id); } }
    impl raw_window_handle::HasWindowHandle for Window {
        fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
            Err(raw_window_handle::HandleError::NotSupported)
        }
    }
    impl raw_window_handle::HasDisplayHandle for Window {
        fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
            Err(raw_window_handle::HandleError::NotSupported)
        }
    }
}

pub mod monitor {
    use crate::dpi::{PhysicalPosition, PhysicalSize};
    #[derive(Clone, Debug)] pub struct MonitorHandle;
    #[derive(Clone, Debug)] pub struct VideoModeHandle;
    impl MonitorHandle {
        pub fn size(&self) -> PhysicalSize<u32> { PhysicalSize::new(1280, 720) }
        pub fn position(&self) -> PhysicalPosition<i32> { PhysicalPosition::new(0, 0) }
        pub fn scale_factor(&self) -> f64 { 1.0 }
        pub fn name(&self) -> Option<String> { None }
        pub fn refresh_rate_millihertz(&self) -> Option<u32> { Some(60000) }
        pub fn video_modes(&self) -> std::iter::Empty<VideoModeHandle> { std::iter::empty() }
    }
    impl VideoModeHandle {
        pub fn size(&self) -> PhysicalSize<u32> { PhysicalSize::new(1280, 720) }
        pub fn bit_depth(&self) -> u16 { 32 }
        pub fn refresh_rate_millihertz(&self) -> u32 { 60000 }
    }
}

pub mod keyboard {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum KeyCode { Again, AltLeft, AltRight, ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Backquote, Backslash, Backspace, BracketLeft, BracketRight, CapsLock, Comma, ContextMenu, ControlLeft, ControlRight, Copy, Cut, Delete, Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, End, Enter, Equal, Escape, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24, Find, Help, Home, Insert, IntlBackslash, IntlRo, IntlYen, KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Lang1, Lang2, Lang3, Lang4, Lang5, Minus, NumLock, Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadAdd, NumpadBackspace, NumpadClear, NumpadClearEntry, NumpadComma, NumpadDecimal, NumpadDivide, NumpadEnter, NumpadEqual, NumpadHash, NumpadMemoryAdd, NumpadMemoryClear, NumpadMemoryRecall, NumpadMemoryStore, NumpadMemorySubtract, NumpadMultiply, NumpadParenLeft, NumpadParenRight, NumpadSubtract, PageDown, PageUp, Paste, Pause, Period, PrintScreen, Quote, ScrollLock, Select, Semicolon, ShiftLeft, ShiftRight, Slash, Space, SuperLeft, SuperRight, Tab, Undo }
    #[derive(Clone, Copy, Debug)] pub enum NativeKeyCode { Unidentified }
    #[derive(Clone, Copy, Debug)] pub enum NativeKey { Unidentified }
    #[derive(Clone, Copy, Debug)] pub enum PhysicalKey { Code(KeyCode), Unidentified(NativeKeyCode) }
    #[derive(Clone, Copy, Debug)] pub enum KeyLocation { Standard, Left, Right, Numpad }
    #[derive(Clone, Copy, Debug)] pub enum NamedKey { Escape, CapsLock, Enter }
    #[derive(Clone, Debug)] pub enum Key { Character(String), Named(NamedKey), Dead(Option<char>), Unidentified(NativeKey) }
    #[derive(Clone, Copy, Debug, Default)] pub struct ModifiersState(u8);
    impl ModifiersState { pub fn shift_key(self)->bool { self.0 & 1 != 0 } pub fn control_key(self)->bool { self.0 & 2 != 0 } pub fn alt_key(self)->bool { self.0 & 4 != 0 } pub fn super_key(self)->bool { self.0 & 8 != 0 } }
}

pub mod event {
    use crate::{dpi::{PhysicalPosition, PhysicalSize}, keyboard::{Key, KeyLocation, PhysicalKey, ModifiersState}};
    use std::path::PathBuf;
    #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ElementState { Pressed, Released }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum MouseButton { Left, Right, Middle, Back, Forward, Other(u16) }
    #[derive(Clone, Copy, Debug)] pub enum TouchPhase { Started, Moved, Ended, Cancelled }
    #[derive(Clone, Copy, Debug)] pub enum MouseScrollDelta { LineDelta(f32,f32), PixelDelta(PhysicalPosition<f64>) }
    #[derive(Clone, Debug)] pub enum Ime { Enabled, Preedit(String, Option<(usize,usize)>), Commit(String), Disabled }
    #[derive(Clone, Debug)] pub struct KeyEvent { pub physical_key: PhysicalKey, pub state: ElementState, pub logical_key: Key, pub location: KeyLocation, pub text: Option<String>, pub repeat: bool }
    #[derive(Clone, Copy, Debug)] pub struct DeviceId;
    impl DeviceId { pub fn dummy() -> Self { Self } }
    #[derive(Clone, Debug)] pub struct Modifiers { state: ModifiersState }
    impl Modifiers { pub fn state(&self) -> ModifiersState { self.state } }
    #[derive(Clone, Debug)] pub enum WindowEvent { KeyboardInput { event: KeyEvent, is_synthetic: bool }, ModifiersChanged(Modifiers), MouseInput { device_id: DeviceId, state: ElementState, button: MouseButton }, CursorMoved { device_id: DeviceId, position: PhysicalPosition<f64> }, MouseWheel { delta: MouseScrollDelta, phase: TouchPhase }, Focused(bool), Resized(PhysicalSize<u32>), CloseRequested, RedrawRequested, Ime(Ime), DroppedFile(PathBuf), Occluded(bool) }
    #[derive(Clone, Debug)] pub enum DeviceEvent { MouseMotion { delta: (f64,f64) }, Added }
}

pub mod platform {
    pub mod modifier_supplement {
        use crate::{event::KeyEvent, keyboard::Key};
        pub trait KeyEventExtModifierSupplement { fn key_without_modifiers(&self) -> Key; }
        impl KeyEventExtModifierSupplement for KeyEvent { fn key_without_modifiers(&self) -> Key { self.logical_key.clone() } }
    }
}

/// Logs an operation which the Cafe facade cannot provide.
pub fn unsupported_log(_operation: &str) {}
/// Lifecycle hook retained for the native Cafe window shim.
pub fn cleanup_window(_id: window::WindowId) {}
