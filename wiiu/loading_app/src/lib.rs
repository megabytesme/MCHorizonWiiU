//! Cafe executable driver for Horizon's original Loading session.
//!
//! WUT owns the RPX entry, WHB process and GX2 lifetime in `main.c`; this
//! library only runs on that initialized GX2 thread.

use horizon_client::game::{
    CafeGameSession, CafeLoadingHost, CafeLoadingPresenter, CafeLoadingSession, CafeLoadingStatus,
    CafeRunOutcome,
};
use horizon_client_platform::{
    ClientDeviceEvent, ClientWindowEvent, EventLoopControl, EventLoopCtl, ImeEvent,
    PhysicalPosition,
};
use horizon_wiiu_loading_host::Gx2LoadingPresenter;
use horizon_wiiu_platform::{
    keyboard::KeyboardAdapter, mouse::{MouseAdapter, MouseOutput}, touch::TouchMapper,
    VpadState,
};
use std::cell::Cell;
use std::ffi::CString;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn OSReport(format: *const std::ffi::c_char, ...);
    fn horizon_wiiu_app_is_running() -> i32;
    fn horizon_wiiu_app_is_focused() -> i32;
    fn horizon_wiiu_swkbd_input_occupied() -> i32;
    fn horizon_swkbd_request(initial_text: *const u16, max_utf16_units: i32) -> i32;
    fn horizon_swkbd_is_open() -> i32;
    fn horizon_swkbd_take_result(
        utf8: *mut std::ffi::c_char,
        capacity: usize,
        length: *mut usize,
    ) -> i32;
    fn horizon_swkbd_close();
    fn horizon_loading_host_dimensions(dimensions: *mut u32) -> i32;
}

/// Status returned to the WUT entry point: 0 finished, 1 title is ready but
/// cannot yet hand off to Game, 2 loading/presentation failed, 3 setup failed.
const FINISHED: i32 = 0;
const READY_WITHOUT_GAME_HANDOFF: i32 = 1;
const LOADING_FAILED: i32 = 2;
const PRESENTER_FAILED: i32 = 3;
const APP_ROOT_FAILED: i32 = 4;
const LOCAL_WORLD_START_FAILED: i32 = 5;

const CAFE_APP_ROOT: &str = "/vol/external01/wiiu/apps/HorizonWiiU";

macro_rules! startup_report {
    ($stage:literal) => {{
        // Fixed static format strings keep these early startup markers free of
        // allocator and logger initialization dependencies.
        unsafe {
            OSReport(
                concat!("Horizon Wii U startup: ", $stage, "\n\0")
                    .as_ptr()
                    .cast(),
            );
        }
    }};
}

/// Called by C before WHB/GX2 initialization so all getrandom versions are
/// anchored and the Cafe system RNG is checked before platform startup.
#[no_mangle]
pub extern "C" fn horizon_wiiu_app_entropy_first() {
    horizon_wiiu_entropy::link();
}

/// Called by the WUT driver after WHB and GX2 initialization.
#[no_mangle]
pub extern "C" fn horizon_wiiu_app_run() -> i32 {
    startup_report!("Rust app entry reached");
    cafe_socket_setup_trace();
    // Cafe std's FS backend mounts the SD card at /vol/external01 when its
    // first filesystem operation initializes the FS client. Set a known app
    // working directory so Options' existing relative .local fallback and
    // game data paths resolve inside this app's isolated SD folder.
    startup_report!("set_current_dir begin");
    if std::env::set_current_dir(CAFE_APP_ROOT).is_err() {
        return APP_ROOT_FAILED;
    }
    startup_report!("set_current_dir complete");

    // Detailed startup spans identify UI and core resource stages. Cafe std
    // cannot create the optional trace file, so send them to OSConsole/serial.
    horizon_client::spans::startup::enable_log_sink();

    // SAFETY: C calls this only after WHBGfxInit and keeps GX2 alive until it
    // returns. The presenter is dropped on this same thread before C shuts GX2
    // down.
    startup_report!("GX2 presenter creation begin");
    let mut presenter = match unsafe { Gx2LoadingPresenter::new_embedded(16_384) } {
        Ok(presenter) => presenter,
        Err(_) => return PRESENTER_FAILED,
    };
    startup_report!("GX2 presenter creation complete");
    let mut host = LoadingHost::new(&mut presenter);
    startup_report!("Horizon entry call begin");
    let outcome = horizon_client::entry::main_with_cafe_host(Default::default(), &mut host);
    startup_report!("Horizon entry call returned");
    if host.local_world_start_failed {
        return LOCAL_WORLD_START_FAILED;
    }
    if host.failure.is_some() {
        return LOADING_FAILED;
    }
    if host.exited {
        return match &outcome {
            CafeRunOutcome::Loading(session) if session.status() == CafeLoadingStatus::Failed => {
                LOADING_FAILED
            }
            _ => FINISHED,
        };
    }
    match outcome {
        CafeRunOutcome::Finished => FINISHED,
        CafeRunOutcome::Loading(session) => match session.status() {
            CafeLoadingStatus::Loading => LOADING_FAILED,
            CafeLoadingStatus::ReadyForHandoff => READY_WITHOUT_GAME_HANDOFF,
            CafeLoadingStatus::LocalWorldStartFailed => LOCAL_WORLD_START_FAILED,
            CafeLoadingStatus::Failed => LOADING_FAILED,
        },
    }
}

/// Opt-in, one-shot probe for the socket setup calls ureq makes before TLS.
/// Enable only for a diagnostic build with HORIZON_CAFE_SOCKET_TRACE=1.
fn cafe_socket_setup_trace() {
    if option_env!("HORIZON_CAFE_SOCKET_TRACE") != Some("1") {
        return;
    }

    unsafe {
        OSReport("Horizon Cafe socket probe: DNS begin\n\0".as_ptr().cast());
    }
    let mut addresses = match ("telemetry.playhorizon.dev", 443).to_socket_addrs() {
        Ok(addresses) => addresses,
        Err(error) => {
            unsafe {
                OSReport(
                    "Horizon Cafe socket probe: DNS failed raw=%d\n\0"
                        .as_ptr()
                        .cast(),
                    error.raw_os_error().unwrap_or(-1),
                );
            }
            return;
        }
    };
    let Some(address) = addresses.next() else {
        unsafe {
            OSReport(
                "Horizon Cafe socket probe: DNS returned no addresses\n\0"
                    .as_ptr()
                    .cast(),
            );
        }
        return;
    };
    unsafe {
        OSReport(
            "Horizon Cafe socket probe: DNS complete\n\0"
                .as_ptr()
                .cast(),
        );
        OSReport(
            "Horizon Cafe socket probe: connect begin\n\0"
                .as_ptr()
                .cast(),
        );
    }
    let stream = match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
        Ok(stream) => stream,
        Err(error) => {
            unsafe {
                OSReport(
                    "Horizon Cafe socket probe: connect failed raw=%d\n\0"
                        .as_ptr()
                        .cast(),
                    error.raw_os_error().unwrap_or(-1),
                );
            }
            return;
        }
    };
    unsafe {
        OSReport(
            "Horizon Cafe socket probe: connect complete\n\0"
                .as_ptr()
                .cast(),
        );
    }

    if let Err(error) = stream.set_nodelay(true) {
        unsafe {
            OSReport(
                "Horizon Cafe socket probe: TCP_NODELAY failed raw=%d\n\0"
                    .as_ptr()
                    .cast(),
                error.raw_os_error().unwrap_or(-1),
            );
        }
        return;
    }
    unsafe {
        OSReport(
            "Horizon Cafe socket probe: TCP_NODELAY complete\n\0"
                .as_ptr()
                .cast(),
        );
    }

    if let Err(error) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
        unsafe {
            OSReport(
                "Horizon Cafe socket probe: read timeout failed raw=%d\n\0"
                    .as_ptr()
                    .cast(),
                error.raw_os_error().unwrap_or(-1),
            );
        }
        return;
    }
    unsafe {
        OSReport(
            "Horizon Cafe socket probe: read timeout complete\n\0"
                .as_ptr()
                .cast(),
        );
    }

    if let Err(error) = stream.set_write_timeout(None) {
        unsafe {
            OSReport(
                "Horizon Cafe socket probe: clear write timeout failed raw=%d\n\0"
                    .as_ptr()
                    .cast(),
                error.raw_os_error().unwrap_or(-1),
            );
        }
        return;
    }
    unsafe {
        OSReport(
            "Horizon Cafe socket probe: pre-TLS socket setup complete\n\0"
                .as_ptr()
                .cast(),
        );
    }
    drop(stream);

    for (index, endpoint) in [
        horizon_telemetry::DEFAULT_ENDPOINT,
        horizon_telemetry::SECONDARY_ENDPOINT,
    ]
    .into_iter()
    .enumerate()
    {
        let endpoint = endpoint.to_owned();
        let spawn = std::thread::Builder::new()
            .name("Cafe HTTPS probe".into())
            .spawn(move || match horizon_telemetry::live::fetch(&endpoint) {
                Ok(count) => unsafe {
                    OSReport(
                        "Horizon Cafe HTTPS GET endpoint=%d succeeded playing=%u lan=%u\n\0"
                            .as_ptr()
                            .cast(),
                        index as i32,
                        count.playing as u32,
                        count.lan as u32,
                    );
                },
                Err(error) => {
                    let error = CString::new(error).unwrap_or_default();
                    unsafe {
                        OSReport(
                            "Horizon Cafe HTTPS GET endpoint=%d failed: %s\n\0"
                                .as_ptr()
                                .cast(),
                            index as i32,
                            error.as_ptr(),
                        );
                    }
                }
            });
        if let Err(error) = spawn {
            unsafe {
                OSReport(
                    "Horizon Cafe HTTPS probe thread=%d failed raw=%d\n\0"
                        .as_ptr()
                        .cast(),
                    index as i32,
                    error.raw_os_error().unwrap_or(-1),
                );
            }
        }
    }
}

struct LoadingHost<'a> {
    presenter: &'a mut Gx2LoadingPresenter<'static>,
    control: Cell<EventLoopControl>,
    vpad: VpadState,
    touch: TouchMapper,
    keyboard: KeyboardAdapter,
    keyboard_events: Vec<ClientWindowEvent>,
    mouse: MouseAdapter,
    mouse_events: Vec<MouseOutput>,
    mouse_position: Option<PhysicalPosition<f64>>,
    native_keyboard_result: Vec<u8>,
    native_keyboard_active: bool,
    keyboard_initialized: bool,
    mouse_initialized: bool,
    focused: bool,
    touch_blocked_until_release: bool,
    pad_blocked_until_neutral: bool,
    touch_trace_remaining: u8,
    trace_touch_draw_this_frame: bool,
    exited: bool,
    failure: Option<String>,
    local_world_start_failed: bool,
}

impl<'a> LoadingHost<'a> {
    fn new(presenter: &'a mut Gx2LoadingPresenter<'static>) -> Self {
        Self {
            presenter,
            control: Cell::new(EventLoopControl::Poll),
            vpad: VpadState::default(),
            touch: TouchMapper::default(),
            keyboard: KeyboardAdapter::default(),
            keyboard_events: Vec::with_capacity(32),
            mouse: MouseAdapter::default(),
            mouse_events: Vec::with_capacity(16),
            mouse_position: None,
            native_keyboard_result: vec![0; 4097],
            native_keyboard_active: false,
            keyboard_initialized: false,
            mouse_initialized: false,
            focused: true,
            touch_blocked_until_release: false,
            pad_blocked_until_neutral: false,
            // Keep the serial trace finite even if a finger is held on the
            // screen and the device emits cursor events every frame.
            touch_trace_remaining: 32,
            trace_touch_draw_this_frame: false,
            exited: false,
            failure: None,
            local_world_start_failed: false,
        }
    }

    fn running(&self) -> bool {
        // SAFETY: C owns and initialized WHBProc before entering Rust.
        unsafe { horizon_wiiu_app_is_running() != 0 }
    }

    fn dispatch_event(
        &mut self,
        loading: &mut CafeLoadingSession,
        game: Option<&mut CafeGameSession>,
        event: ClientWindowEvent,
    ) {
        if let Some(game) = game {
            game.dispatch_event(self, event);
        } else {
            loading.dispatch_event(self, event);
        }
    }

    fn dispatch_pad_state(
        &mut self,
        loading: &mut CafeLoadingSession,
        game: Option<&mut CafeGameSession>,
        state: horizon_client::gamepad::PadState,
    ) {
        if let Some(game) = game {
            game.feed_pad_state(state);
        } else {
            loading.dispatch_pad_state(self, state);
        }
    }

    fn dispatch_touch(
        &mut self,
        session: &mut CafeLoadingSession,
        mut game: Option<&mut CafeGameSession>,
        release: Option<horizon_wiiu_platform::FreshTouchRelease>,
        trace: bool,
    ) -> bool {
        if self.touch_blocked_until_release {
            if release.is_none() {
                return false;
            }
            self.touch_blocked_until_release = false;
        }
        let mut dimensions = [0u32; 4];
        // SAFETY: dimensions points to four writable u32 values; C queries
        // WHB's current TV/DRC targets synchronously.
        let result = unsafe { horizon_loading_host_dimensions(dimensions.as_mut_ptr()) };
        if result != 0 {
            self.failure = Some("WHB TV/DRC colour buffers are unavailable".into());
            return false;
        }
        let viewport = [dimensions[2], dimensions[3]];
        let events = self
            .touch
            .events_with_freshness(&self.vpad, viewport, release);
        if trace && !events.is_empty() {
            startup_report!("touch event batch ready");
        }
        let mut activation_dispatched = false;
        for event in events {
            let trace_event = self.touch_trace_remaining > 0;
            if trace_event {
                self.touch_trace_remaining -= 1;
                self.trace_touch_draw_this_frame = true;
            }
            let (event_kind, x, y) = match &event {
                horizon_client_platform::ClientWindowEvent::CursorMoved { position } => {
                    ("cursor", Some(position.x), Some(position.y))
                }
                horizon_client_platform::ClientWindowEvent::MouseInput { state, .. } => {
                    let kind = match state {
                        horizon_client_platform::KeyState::Pressed => "press",
                        horizon_client_platform::KeyState::Released => "release",
                    };
                    (kind, None, None)
                }
                _ => ("other", None, None),
            };
            let activation = matches!(
                &event,
                horizon_client_platform::ClientWindowEvent::MouseInput { .. }
            );
            let dispatch_started = Instant::now();
            if trace_event {
                log::info!(
                    "Horizon Cafe touch: dispatch begin kind={} x={:?} y={:?}",
                    event_kind,
                    x,
                    y
                );
            }
            // Dispatch each event immediately with the exact poll/release
            // evidence that produced it. Pointer position precedes press.
            self.dispatch_event(session, game.as_deref_mut(), event);
            activation_dispatched |= activation;
            if trace_event {
                log::info!(
                    "Horizon Cafe touch: dispatch complete kind={} elapsed_ms={}",
                    event_kind,
                    dispatch_started.elapsed().as_millis()
                );
            }
        }
        activation_dispatched
    }

    fn release_on_exit(
        &mut self,
        session: &mut CafeLoadingSession,
        mut game: Option<&mut CafeGameSession>,
    ) {
        self.mouse.release_all_into(&mut self.mouse_events);
        self.dispatch_mouse_events(session, game.as_deref_mut());
        self.keyboard.release_all_into(&mut self.keyboard_events);
        self.dispatch_queued_keyboard(session, game.as_deref_mut());
        let release = self.touch.reset(&self.vpad);
        for event in release {
            self.dispatch_event(session, game.as_deref_mut(), event);
        }
        let neutral = self.vpad.reset();
        self.pad_blocked_until_neutral = true;
        self.dispatch_pad_state(session, game.as_deref_mut(), neutral);
        self.dispatch_event(
            session,
            game,
            horizon_client_platform::ClientWindowEvent::Focused(false),
        );
        unsafe { horizon_swkbd_close() };
        if self.keyboard_initialized {
            let result = horizon_wiiu_platform::keyboard::shutdown();
            if result != 0 {
                log::warn!("Horizon Cafe USB keyboard teardown returned {result}");
            }
            self.keyboard_initialized = false;
        }
        if self.mouse_initialized {
            let result = horizon_wiiu_platform::mouse::shutdown();
            if result != 0 {
                log::warn!("Horizon Cafe USB mouse teardown returned {result}");
            }
            self.mouse_initialized = false;
        }
    }

    fn dispatch_queued_keyboard(
        &mut self,
        session: &mut CafeLoadingSession,
        mut game: Option<&mut CafeGameSession>,
    ) {
        let mut events = std::mem::take(&mut self.keyboard_events);
        for event in events.drain(..) {
            self.dispatch_event(session, game.as_deref_mut(), event);
        }
        self.keyboard_events = events;
    }

    fn dispatch_mouse_events(
        &mut self,
        session: &mut CafeLoadingSession,
        mut game: Option<&mut CafeGameSession>,
    ) {
        if self.mouse_events.is_empty() {
            return;
        }

        let mut dimensions = [0u32; 4];
        let (width, height) = if unsafe {
            horizon_loading_host_dimensions(dimensions.as_mut_ptr())
        } == 0
        {
            (dimensions[2].max(1), dimensions[3].max(1))
        } else {
            let [width, height] = self.presenter.layout_viewport();
            (width.max(1), height.max(1))
        };
        let width = f64::from(width);
        let height = f64::from(height);
        let mut position = self.mouse_position.unwrap_or(PhysicalPosition::new(
            (width - 1.0) * 0.5,
            (height - 1.0) * 0.5,
        ));
        if self.mouse_position.is_none() {
            // Establish the DRC pointer before the first button-only report,
            // including the first report after ProcUI focus returns.
            self.dispatch_event(
                session,
                game.as_deref_mut(),
                ClientWindowEvent::CursorMoved { position },
            );
        }

        let mut events = std::mem::take(&mut self.mouse_events);
        for event in events.drain(..) {
            match event {
                MouseOutput::Device(ClientDeviceEvent::MouseMotion { delta: (dx, dy) }) => {
                    position.x = (position.x + dx).clamp(0.0, width - 1.0);
                    position.y = (position.y + dy).clamp(0.0, height - 1.0);
                    let motion = ClientDeviceEvent::MouseMotion { delta: (dx, dy) };
                    if let Some(game) = game.as_deref_mut() {
                        game.dispatch_device_event(self, motion);
                    }
                    self.dispatch_event(
                        session,
                        game.as_deref_mut(),
                        ClientWindowEvent::CursorMoved { position },
                    );
                }
                MouseOutput::Window(event) => {
                    self.dispatch_event(session, game.as_deref_mut(), event);
                }
            }
        }
        self.mouse_position = Some(position);
        self.mouse_events = events;
    }

    fn drain_mouse_input(
        &mut self,
        session: &mut CafeLoadingSession,
        game: Option<&mut CafeGameSession>,
    ) {
        self.mouse.drain_into(&mut self.mouse_events);
        self.dispatch_mouse_events(session, game);
    }

    fn discard_mouse_input(&mut self) {
        self.mouse.drain_into(&mut self.mouse_events);
        self.mouse_events.clear();
        self.mouse.release_all_into(&mut self.mouse_events);
        self.mouse_events.clear();
    }

    fn discard_keyboard_input(&mut self) {
        self.keyboard_events.clear();
        self.keyboard.drain_into(&mut self.keyboard_events);
        self.keyboard.release_all_into(&mut self.keyboard_events);
        self.keyboard_events.clear();
    }

    fn poll_native_keyboard_result(
        &mut self,
        session: &mut CafeLoadingSession,
        game: Option<&mut CafeGameSession>,
    ) {
        let mut length = 0usize;
        let result = unsafe {
            horizon_swkbd_take_result(
                self.native_keyboard_result.as_mut_ptr().cast(),
                self.native_keyboard_result.len(),
                &mut length,
            )
        };
        match result {
            0 => {}
            1 if length < self.native_keyboard_result.len() => {
                let text =
                    String::from_utf8_lossy(&self.native_keyboard_result[..length]).into_owned();
                self.dispatch_event(session, game, ClientWindowEvent::Ime(ImeEvent::Commit(text)));
            }
            2 => self.dispatch_event(session, game, ClientWindowEvent::Ime(ImeEvent::Disabled)),
            error => log::warn!("Horizon Cafe system keyboard result failed: {error}"),
        }
    }

    fn update_focus(
        &mut self,
        session: &mut CafeLoadingSession,
        mut game: Option<&mut CafeGameSession>,
    ) -> bool {
        // SAFETY: ProcUI was initialized by WHBProcInit and is advanced by
        // WHBProcIsRunning on this same main thread.
        let focused = unsafe { horizon_wiiu_app_is_focused() != 0 };
        if focused == self.focused {
            return focused;
        }
        self.focused = focused;
        if focused {
            self.touch.focus_gained(&self.vpad);
        } else {
            self.touch_blocked_until_release = true;
            for event in self.touch.focus_lost(&self.vpad) {
                self.dispatch_event(session, game.as_deref_mut(), event);
            }
            let neutral = self.vpad.reset();
            self.pad_blocked_until_neutral = true;
            self.dispatch_pad_state(session, game.as_deref_mut(), neutral);
            self.keyboard.release_all_into(&mut self.keyboard_events);
            self.dispatch_queued_keyboard(session, game.as_deref_mut());
            self.mouse.release_all_into(&mut self.mouse_events);
            self.dispatch_mouse_events(session, game.as_deref_mut());
            self.mouse_position = None;
        }
        self.dispatch_event(
            session,
            game,
            horizon_client_platform::ClientWindowEvent::Focused(focused),
        );
        focused
    }
}

impl EventLoopCtl for LoadingHost<'_> {
    fn set_control_flow(&self, control: EventLoopControl) {
        self.control.set(control);
    }

    fn open_text_input(&self, initial_text: &str, max_utf16_units: usize) -> bool {
        let mut initial_units: Vec<u16> = initial_text.encode_utf16().take(1023).collect();
        if initial_units
            .last()
            .is_some_and(|unit| (0xD800..=0xDBFF).contains(unit))
        {
            initial_units.pop();
        }
        initial_units.push(0);
        unsafe {
            horizon_swkbd_request(
                initial_units.as_ptr(),
                max_utf16_units.clamp(1, 1024) as i32,
            ) == 0
        }
    }
}

impl CafeLoadingHost for LoadingHost<'_> {
    fn drive_loading(&mut self, session: &mut CafeLoadingSession) -> horizon_client::Result<()> {
        match horizon_wiiu_platform::keyboard::initialize() {
            Ok(()) => {
                self.keyboard_initialized = true;
                startup_report!("USB keyboard initialized");
            }
            Err(error) => log::warn!("Horizon Cafe USB keyboard initialization failed: {error}"),
        }
        match horizon_wiiu_platform::mouse::initialize() {
            Ok(()) => {
                self.mouse_initialized = true;
                // Drop any reports queued by a prior host lifetime before
                // accepting new input in this session.
                self.discard_mouse_input();
                startup_report!("USB mouse initialized");
            }
            Err(error) => log::warn!("Horizon Cafe USB mouse initialization failed: {error}"),
        }
        let mut trace_focus = true;
        let mut trace_poll = true;
        let mut trace_vpad = true;
        let mut trace_pad_dispatch = true;
        let mut trace_touch_dispatch = true;
        let mut trace_first_touch_sample = true;
        let mut trace_input_failure_check = true;
        let mut trace_draw = true;
        let mut progress_report_at = Instant::now();
        let mut frames_since_progress_report = 0u32;
        let mut game_session: Option<CafeGameSession> = None;
        loop {
            if !self.running() || self.control.get() == EventLoopControl::Exit {
                self.exited = true;
                break;
            }
            if trace_focus {
                startup_report!("loading first focus update begin");
            }
            let focused = self.update_focus(session, game_session.as_mut());
            if trace_focus {
                startup_report!("loading first focus update complete");
                trace_focus = false;
            }
            if trace_poll {
                startup_report!("loading first session poll begin");
            }
            let alive = session.poll(self);
            if trace_poll {
                startup_report!("loading first session poll complete");
                trace_poll = false;
            }
            if !alive {
                self.exited = true;
                break;
            }

            // USB mouse input is independent of VPAD and CafeOS's on-screen
            // keyboard, so it remains available while native input owns touch.
            if focused {
                self.drain_mouse_input(session, game_session.as_mut());
            } else {
                self.discard_mouse_input();
            }

            if !focused {
                // WHBGfx frees foreground buffers on a ProcUI release. Keep
                // loader/menu services moving but do not touch GX2 or VPAD.
                self.discard_keyboard_input();
                std::thread::sleep(Duration::from_millis(16));
                continue;
            }

            if trace_vpad {
                startup_report!("loading first VPAD sample begin");
            }
            let (pad, fresh_sample, fresh_release, terminal_error) = match self
                .vpad
                .poll_with_evidence()
            {
                Ok((pad, fresh_sample, fresh_release)) => (pad, fresh_sample, fresh_release, false),
                // VPAD busy means no new sample. Retain the previous state and
                // never claim a physical touch release.
                Err(-4) => (self.vpad.update(None), None, None, false),
                // Terminal controller errors reset VpadState in the adapter;
                // release the mapped click, while requiring a fresh physical
                // release before a later touch starts another click.
                Err(_) => {
                    self.pad_blocked_until_neutral = true;
                    (self.vpad.reset(), None, None, true)
                }
            };
            let trace_touch_sample_this_frame =
                trace_first_touch_sample && fresh_sample.is_some() && self.vpad.touch_active();
            if trace_touch_sample_this_frame {
                startup_report!("first Cafe VPAD touch sample observed");
                if self.vpad.touch().is_some() {
                    startup_report!("first Cafe VPAD touch sample has a valid point");
                } else {
                    startup_report!("first Cafe VPAD touch sample has no valid point");
                }
                trace_first_touch_sample = false;
            }
            if trace_vpad {
                startup_report!("loading first VPAD sample complete");
                trace_vpad = false;
            }

            let swkbd_occupied = unsafe { horizon_wiiu_swkbd_input_occupied() != 0 };
            let swkbd_open = unsafe { horizon_swkbd_is_open() != 0 };
            let mut native_input_this_frame =
                self.native_keyboard_active || swkbd_occupied || swkbd_open;
            self.native_keyboard_active = swkbd_occupied || swkbd_open;
            self.poll_native_keyboard_result(session, game_session.as_mut());
            if native_input_this_frame && !self.native_keyboard_active {
                // A result closed the system keyboard. Keep the host gate only
                // if a contact is still active; TouchMapper::reset retains its
                // own release suppression for any previously seen contact.
                self.touch_blocked_until_release = self.vpad.touch_active();
                self.pad_blocked_until_neutral = true;
                // Cancel any gesture accumulated while CafeOS owned contact,
                // without changing mapper focus or the VPAD state that the
                // native-input branch will pair with this poll's marker.
                for event in self.touch.reset(&self.vpad) {
                    self.dispatch_event(session, game_session.as_mut(), event);
                }
                // Keep ProcUI focus and the poll's VPAD state intact. The
                // native-input touch branch below consumes `fresh_release`
                // once against this exact state; pad delivery resets separately.
                self.dispatch_pad_state(
                    session,
                    game_session.as_mut(),
                    horizon_client::gamepad::PadState::default(),
                );
                self.keyboard.release_all_into(&mut self.keyboard_events);
                self.dispatch_queued_keyboard(session, game_session.as_mut());
            }

            self.keyboard_events.clear();
            self.keyboard.drain_into(&mut self.keyboard_events);
            if native_input_this_frame || !focused {
                // Do not replay stale USB keystrokes after CafeOS takes over
                // the GamePad keyboard or ProcUI has moved to the background.
                self.keyboard_events.clear();
                self.keyboard.release_all_into(&mut self.keyboard_events);
                self.dispatch_queued_keyboard(session, game_session.as_mut());
            } else {
                self.dispatch_queued_keyboard(session, game_session.as_mut());
            }

            if trace_pad_dispatch {
                log::info!(
                    "Horizon: first Cafe pad sample fresh={} terminal_error={} buttons={:#010x} left={:?} right={:?} triggers={:?}",
                    fresh_sample.is_some(),
                    terminal_error,
                    pad.buttons,
                    pad.left,
                    pad.right,
                    pad.triggers
                );
                startup_report!("loading first pad dispatch begin");
            }
            if native_input_this_frame {
                // VPADStatus was already passed to CafeOS's keyboard Calc.
                // Do not also route the same pad sample through Horizon menus.
                // CafeGameSession retains its fed state between polls, so
                // explicitly clear it while CafeOS owns the controller.
                if let Some(game) = game_session.as_mut() {
                    game.feed_pad_state(horizon_client::gamepad::PadState::default());
                }
            } else if terminal_error {
                // Deliver neutral immediately to release any menu edge, but
                // do not treat an error-created default as fresh rearm proof.
                self.dispatch_pad_state(session, game_session.as_mut(), pad);
            } else if self.pad_blocked_until_neutral {
                let neutral = pad.buttons == 0 && pad.triggers.iter().all(|value| *value == 0.0);
                // Only a successful sample can rearm. Cached neutral state,
                // no-sample/busy polls, and touch-release evidence do not.
                if fresh_sample.is_some() && neutral {
                    self.pad_blocked_until_neutral = false;
                    self.dispatch_pad_state(session, game_session.as_mut(), pad);
                }
            } else {
                // Pass the actual canonical Cafe controller sample through
                // Horizon's typed menu path; no keyboard events are synthesized.
                self.dispatch_pad_state(session, game_session.as_mut(), pad);
            }
            if !native_input_this_frame && unsafe { horizon_swkbd_is_open() != 0 } {
                // The south-button sample that opens CafeOS is still delivered
                // to Horizon; suppress touch/pad dispatch for the rest of it.
                native_input_this_frame = true;
                self.native_keyboard_active = true;
                self.keyboard.release_all_into(&mut self.keyboard_events);
                self.dispatch_queued_keyboard(session, game_session.as_mut());
                // This sample opened CafeOS, but the game frame below now
                // belongs to its keyboard. Do not leave that sample latched.
                if let Some(game) = game_session.as_mut() {
                    game.feed_pad_state(horizon_client::gamepad::PadState::default());
                }
            }
            if trace_pad_dispatch {
                startup_report!("loading first pad dispatch complete");
                trace_pad_dispatch = false;
            }
            if trace_touch_sample_this_frame {
                startup_report!("first Cafe VPAD touch mapping begin");
            }
            let touch_activation_dispatched = if native_input_this_frame {
                // Keep TouchMapper's contact/release state current while the
                // CafeOS keyboard consumes the GamePad touchscreen.
                let mut dimensions = [0u32; 4];
                let viewport = if unsafe {
                    horizon_loading_host_dimensions(dimensions.as_mut_ptr())
                } == 0
                {
                    [dimensions[2], dimensions[3]]
                } else {
                    // A release marker is still valid without a drawable
                    // target; TouchMapper consumes it before scaling points.
                    [1, 1]
                };
                let _ = self
                    .touch
                    .events_with_freshness(&self.vpad, viewport, fresh_release);
                false
            } else {
                self.dispatch_touch(
                    session,
                    game_session.as_mut(),
                    fresh_release,
                    trace_touch_dispatch,
                )
            };
            if trace_touch_sample_this_frame {
                startup_report!("first Cafe VPAD touch mapping complete");
            }
            if trace_touch_dispatch && touch_activation_dispatched {
                startup_report!("first touch activation dispatched");
                trace_touch_dispatch = false;
            }
            if trace_input_failure_check {
                startup_report!("loading first input failure check begin");
            }
            if self.failure.is_some() {
                if trace_input_failure_check {
                    startup_report!("loading first input failure detected");
                }
                break;
            }
            if trace_input_failure_check {
                startup_report!("loading first input failure check complete");
                trace_input_failure_check = false;
            }

            if trace_draw {
                startup_report!("loading first draw begin");
            }
            let draw_started = Instant::now();
            let draw_result = session.draw(self.presenter);
            if trace_draw {
                startup_report!("loading first draw returned");
                trace_draw = false;
            }
            if self.trace_touch_draw_this_frame {
                self.trace_touch_draw_this_frame = false;
                log::info!(
                    "Horizon Cafe touch: current frame draw returned elapsed_ms={}",
                    draw_started.elapsed().as_millis()
                );
            }
            if let Err(error) = draw_result {
                self.failure = Some(error.to_string());
                break;
            }
            frames_since_progress_report = frames_since_progress_report.saturating_add(1);
            if progress_report_at.elapsed() >= Duration::from_secs(5) {
                log::info!(
                    "Horizon Cafe loop heartbeat: frames={} status={:?} failure={}",
                    frames_since_progress_report,
                    session.status(),
                    self.failure.is_some()
                );
                progress_report_at = Instant::now();
                frames_since_progress_report = 0;
            }
            match session.status() {
                CafeLoadingStatus::Loading => {}
                CafeLoadingStatus::ReadyForHandoff if game_session.is_none() => {
                    match session.start_local_game() {
                        Ok(Some(game)) => {
                            startup_report!("original local Game initialized; world startup begins");
                            game_session = Some(game);
                        }
                        Ok(None) => {
                            // Remote-server handoff remains on its existing path.
                        }
                        Err(error) => {
                            self.local_world_start_failed = true;
                            self.failure = Some(error.to_string());
                            log::error!("Horizon Cafe local world start failed: {error}");
                            startup_report!("original local Game handoff failed");
                            break;
                        }
                    }
                }
                CafeLoadingStatus::ReadyForHandoff => {}
                CafeLoadingStatus::LocalWorldStartFailed => {
                    self.local_world_start_failed = true;
                    self.failure = session.failure_message();
                    break;
                }
                CafeLoadingStatus::Failed => break,
            }

            if let Some(game) = &mut game_session {
                if let Err(error) = game.step(self) {
                    self.local_world_start_failed = true;
                    self.failure = Some(error.to_string());
                    log::error!("Horizon Cafe local world start failed: {error}");
                    startup_report!("original local world startup failed");
                    break;
                }
            }

            // Bound the software loop to the display cadence while honoring
            // host exit. The title callback itself synchronously presents TV
            // and DRC with WHB's own frame begin/finish calls.
            let delay = match self.control.get() {
                EventLoopControl::Exit => Duration::ZERO,
                EventLoopControl::Poll => Duration::from_millis(16),
                EventLoopControl::WaitUntil(deadline) => {
                    deadline.saturating_duration_since(Instant::now())
                }
            };
            if !delay.is_zero() {
                std::thread::sleep(delay.min(Duration::from_millis(16)));
            }
        }
        // Every way out of the host loop releases touch and controller edges
        // while the Loading session and its event loop are still alive.
        if let Some(game) = &mut game_session {
            game.shutdown();
        }
        self.release_on_exit(session, game_session.as_mut());
        Ok(())
    }
}
