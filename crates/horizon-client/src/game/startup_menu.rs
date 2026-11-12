//! The interactive title before world resources exist. Uses only the menu
//! atlas/panorama; transfers the actual screen and queued entry to Game.
use super::event_loop::EventLoopCtl;
use super::key_focus::{self, Act};
use super::*;
use horizon_client::{
    clipboard::{self, AppendSelection, SystemClipboard},
    options_ui::{Event, OptionsUi, Page},
    widgets::{Kind, Target},
};
use horizon_client_platform::{
    ClientWindowEvent as WindowEvent, ImeEvent, KeyCode, KeyLocation, KeyState as ElementState,
    LogicalKey, Modifiers, MouseButton,
};

#[derive(Default)]
pub(super) struct StartupMenu {
    title: Option<Title>,
    options: OptionsUi,
    options_open: bool,
    /// The credits from Credits & Attribution, and their last frame.
    win: Option<(horizon_client::win_screen::WinScreen, Instant)>,
    root: PathBuf,
    cursor: (f32, f32),
    pointer: (f64, f64),
    modifiers: Modifiers,
    physical_keys: HashSet<KeyCode>,
    dragging: bool,
    clipboard: SystemClipboard,
    selection: AppendSelection,
    /// The narrator's `NARRATOR_TOGGLE` toast, handed to the game with the title.
    narrator_toast: Option<horizon_client::toasts::SystemToast>,
    /// The screens' narration, handed to the game with the title (which
    /// stays the screen already opened).
    narration: horizon_client::screen_narrator::ScreenNarrator,
    /// Frames drawn, for `widgets::end_frame`.
    frames: u64,
    /// The keyboard's focus (`key_focus`: Tab, the arrows, Enter); the
    /// pointer sits on it, as in the game.
    key_focus: Option<Target>,
    /// Fractional vertical scroll from the GamePad's right stick.
    pad_scroll: f32,
    /// Last controller poll used to scale stick scroll independent of frame rate.
    pad_scroll_poll: Option<Instant>,
    /// Left-stick direction currently held for menu navigation.
    pad_navigation: Option<horizon_client::gamepad::Button>,
    /// Next time the held left stick may repeat its navigation step.
    pad_navigation_repeat_at: Option<Instant>,
    /// The focused slider's arrows change its value (`canChangeValue`).
    key_slider_edit: bool,
    /// What the last click landed on, for the next Tab or arrow.
    key_origin: Option<[f32; 4]>,
    /// The focused slider being nudged and where the keyboard put its handle.
    key_nudge: Option<([f32; 4], f32)>,
    /// A click the keyboard sends is being handled.
    key_synthetic: bool,
    /// The screen the keyboard's focus is on (`key_screen`).
    key_screen: u64,
    /// Cafe's controller keyboard for the title's active text field.
    osk: Option<horizon_client::osk::Keyboard>,
    /// CafeOS's native DRC keyboard owns input for the focused text field.
    native_osk: bool,
    /// Draw the controller keyboard at the top when its field is in the lower half.
    osk_at_top: bool,
    pub pending: Option<PendingWorldEntry>,
    pub reload_packs: bool,
    /// Browse mods' worker (`mod_browser`).
    mod_jobs: super::mod_browser::Jobs,
    pub perf: horizon_client::telemetry::Session,
}

impl StartupMenu {
    pub fn new(options: &Options, pack_error: Option<&str>) -> Result<Self> {
        let pending = if let Some(address) = &options.server {
            Some(PendingWorldEntry::Server {
                address: address.clone(),
                name: options.player_name(),
                cookies: None,
                lan: false,
            })
        } else {
            options
                .world
                .as_ref()
                .map(|folder| PendingWorldEntry::Local {
                    folder: folder.clone(),
                    name: None,
                    seed: Some(options.seed),
                    creative: options.creative,
                    create: None,
                })
        };
        let mut menu = Self {
            pending,
            ..Default::default()
        };
        menu.root = options.resources.clone();
        if let Some(error) = pack_error {
            menu.options.open_at(false, Page::Packs);
            menu.options.resource_error = Some(error.to_owned());
            menu.options_open = true;
        }
        if menu.pending.is_some() {
            menu.perf.world_opening();
        }
        Ok(menu)
    }

    /// Scroll the original title list with the Cafe GamePad's right stick,
    /// using the same deadzone, speed and fractional wheel steps as Game.
    pub(super) fn pad_stick_scroll(
        &mut self,
        state: horizon_client::gamepad::PadState,
        settings: &Settings,
        now: Instant,
        viewport: [u32; 2],
    ) {
        let seconds = self.pad_scroll_poll.map_or(0.0, |previous| {
            now.saturating_duration_since(previous)
                .as_secs_f32()
                .min(0.05)
        });
        self.pad_scroll_poll = Some(now);

        let scrollable = settings.controller.enabled
            && !self.options_open
            && self.pending.is_none()
            && self.win.is_none()
            && self
                .title
                .as_mut()
                .is_some_and(|title| !title.has_caret_field());
        let axis = if scrollable {
            horizon_client::gamepad::radial(state.right, settings.controller.look_deadzone)[1]
        } else {
            0.0
        };
        let lines = pad_scroll_lines(
            &mut self.pad_scroll,
            axis,
            settings.controller.scroll_speed,
            seconds,
        );
        if lines == 0.0 {
            return;
        }
        // The Loading title's clipped rows are drawn before the main Game
        // loop takes over its scroll-area registry. Anchor the hidden stick
        // pointer to a real visible row so Title::scroll_worlds accepts the
        // wheel step during startup too.
        let cursor = key_focus::drawn_targets()
            .into_iter()
            .find(|target| target.kind == Kind::Row)
            .map_or(self.cursor, |target| {
                (self.cursor.0, target.rect[1] + target.rect[3] / 2.0)
            });
        let batch = HudBatch::new(viewport);
        if let Some(title) = &mut self.title {
            title.stick_scroll(lines, cursor, (batch.width, batch.height));
        }
    }

    fn pad_stick_direction(
        &mut self,
        state: horizon_client::gamepad::PadState,
        deadzone: f32,
        now: Instant,
        enabled: bool,
    ) -> Option<horizon_client::gamepad::Button> {
        use horizon_client::gamepad::Button;

        #[cfg(target_os = "cafe")]
        let trace_first_direction = {
            static FIRST_PAD_DIRECTION_TRACE: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(true);
            FIRST_PAD_DIRECTION_TRACE.swap(false, std::sync::atomic::Ordering::Relaxed)
        };
        #[cfg(not(target_os = "cafe"))]
        let trace_first_direction = false;

        if trace_first_direction {
            cafe_game_stage!("first pad direction begin");
        }

        const INITIAL_REPEAT: Duration = Duration::from_millis(350);
        const REPEAT_INTERVAL: Duration = Duration::from_millis(120);
        const DIRECTION_HYSTERESIS: f32 = 0.12;

        let dpad_held = [
            Button::DPadUp,
            Button::DPadDown,
            Button::DPadLeft,
            Button::DPadRight,
        ]
        .into_iter()
        .any(|button| state.held(button));
        if trace_first_direction {
            cafe_game_stage!("first pad direction dpad scan complete");
        }
        let direction = if enabled && !dpad_held {
            if trace_first_direction {
                cafe_game_stage!("first pad direction radial begin");
            }
            let [x, y] = horizon_client::gamepad::radial(state.left, deadzone);
            if trace_first_direction {
                cafe_game_stage!("first pad direction radial complete");
            }
            let horizontal = if x == 0.0 {
                false
            } else if y == 0.0 {
                true
            } else if matches!(
                self.pad_navigation,
                Some(Button::DPadLeft | Button::DPadRight)
            ) {
                x.abs() + DIRECTION_HYSTERESIS >= y.abs()
            } else {
                x.abs() > y.abs() + DIRECTION_HYSTERESIS
            };
            if horizontal {
                Some(if x > 0.0 {
                    Button::DPadRight
                } else {
                    Button::DPadLeft
                })
            } else if y != 0.0 {
                Some(if y > 0.0 {
                    Button::DPadUp
                } else {
                    Button::DPadDown
                })
            } else {
                None
            }
        } else {
            None
        };
        if trace_first_direction {
            cafe_game_stage!("first pad direction select complete");
        }

        let Some(direction) = direction else {
            self.pad_navigation = None;
            self.pad_navigation_repeat_at = None;
            if trace_first_direction {
                cafe_game_stage!("first pad direction neutral return");
            }
            return None;
        };
        if self.pad_navigation != Some(direction) {
            self.pad_navigation = Some(direction);
            self.pad_navigation_repeat_at = Some(now + INITIAL_REPEAT);
            if trace_first_direction {
                cafe_game_stage!("first pad direction initial repeat set");
            }
            return Some(direction);
        }
        if self
            .pad_navigation_repeat_at
            .is_some_and(|repeat_at| now >= repeat_at)
        {
            self.pad_navigation_repeat_at = Some(now + REPEAT_INTERVAL);
            if trace_first_direction {
                cafe_game_stage!("first pad direction repeat update");
            }
            return Some(direction);
        }
        if trace_first_direction {
            cafe_game_stage!("first pad direction no repeat return");
        }
        None
    }

    pub(super) fn set_root(&mut self, root: PathBuf, settings: &Settings) {
        horizon_client::ui::load_metadata(&root);
        horizon_client::lang::set(horizon_client::lang::Language::load(
            &[&root],
            &settings.language,
        ));
        self.root = root;
        if self.title.is_none() {
            let mut title = Title::new(
                std::env::current_dir().unwrap_or_default().join("saves"),
                &self.root,
            );
            title.last_server = settings.file.string("lastServer");
            title.skip_multiplayer_warning = settings.file.bool("skipMultiplayerWarning");
            // Gui.addInitialScreens: the accessibility onboarding comes first.
            if settings.file.bool("onboardAccessibility") {
                title.start_onboarding();
            }
            self.title = Some(title);
        }
    }

    pub(super) fn handoff(&mut self, game: &mut Game<'_>) {
        game.title = self
            .title
            .take()
            .map(|title| title.with_user(|| game.options.player_name()));
        game.pending_entry = self.pending.take();
        game.cursor = self.cursor;
        game.modifiers = self.modifiers;
        game.physical_keys = std::mem::take(&mut self.physical_keys);
        game.clipboard = std::mem::take(&mut self.clipboard);
        game.text_selection = std::mem::take(&mut self.selection);
        game.perf = std::mem::take(&mut self.perf);
        game.narrator_toast = self.narrator_toast.take();
        game.narration = std::mem::take(&mut self.narration);
        // Preserve an existing resource-error screen opened by start_game.
        if self.options_open {
            let resource_error = game.ui.options.resource_error.take();
            game.ui.options = std::mem::take(&mut self.options);
            game.ui.options.resource_error =
                game.ui.options.resource_error.take().or(resource_error);
            game.screen = Screen::Options;
            game.options_from_title = true;
            game.dragging = self.dragging;
        }
        if let Some(focus) = self.key_focus.take() {
            game.key_focus_adopt(focus, self.key_slider_edit);
        }
    }

    /// After a frame is drawn: its widgets become the last frame's, and the
    /// open screen's narration reads them (`Gui.update`'s
    /// `handleDelayedNarration`). `gui` is the GUI size.
    pub(super) fn frame_drawn(&mut self, gui: (f32, f32)) {
        self.frames += 1;
        horizon_client::widgets::end_frame(self.frames);
        // The keyboard's focus follows its widget, or starts again on a new
        // screen.
        if let Some(focus) = self.key_focus {
            let targets = key_focus::keyboard_targets(
                key_focus::on_screen(key_focus::drawn_targets(), gui),
                false,
            );
            let screen = self.key_screen();
            self.key_focus = if screen == self.key_screen {
                key_focus::follow(focus, &targets)
            } else {
                // Screen.setInitialFocus: a forward Tab from nothing.
                self.key_screen = screen;
                key_focus::tab_step(&targets, None, true).map(|i| targets[i])
            };
            if let Some(now) = self.key_focus
                && now.rect != focus.rect
            {
                self.cursor = now.center();
                self.key_nudge = None;
            }
        }
        self.narration
            .frame(gui, self.cursor, self.key_focus.map(|t| t.rect));
    }

    /// Which screen is open, as far as the keyboard's focus cares.
    fn key_screen(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (self.options_open, self.win.is_some()).hash(&mut hasher);
        std::mem::discriminant(&self.options.page()).hash(&mut hasher);
        self.title.as_ref().map(Title::screen_key).hash(&mut hasher);
        hasher.finish()
    }

    /// The keyboard lets go of the focus (the real mouse is used): it stays
    /// where the next Tab or arrow goes on from. Whether it had it.
    fn key_focus_release(&mut self) -> bool {
        let Some(focus) = self.key_focus.take() else {
            return false;
        };
        self.key_origin = Some(focus.rect);
        self.key_nudge = None;
        true
    }

    fn queue_entry(&mut self, action: Action, player_name: String) {
        let entry = match action {
            Action::Play { folder } => PendingWorldEntry::Local {
                folder,
                name: None,
                seed: None,
                creative: false,
                create: None,
            },
            Action::Create {
                folder,
                name,
                seed,
                creative,
                options,
            } => PendingWorldEntry::Local {
                folder,
                name: Some(name),
                seed,
                creative,
                create: Some(Box::new(options)),
            },
            Action::Connect { address, lan } => PendingWorldEntry::Server {
                address,
                name: player_name,
                cookies: None,
                lan,
            },
            _ => return,
        };
        self.pending = Some(entry);
        self.perf.world_opening();
    }

    fn shortcut(
        &mut self,
        event: &horizon_client_platform::KeyboardEvent,
    ) -> Option<clipboard::EditShortcut> {
        let edit = if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        };
        clipboard::edit_shortcut(
            clipboard::shortcut_letter(
                event.physical_key.unwrap_or(KeyCode::Other),
                event.logical_text.as_deref(),
            ),
            edit,
            self.modifiers.shift_key(),
            self.modifiers.alt_key(),
        )
    }

    /// `Screen.keyPressed` on the title screens: the action a key starts (Enter confirms, see `confirm_key` in title.rs).
    fn title_key(
        &mut self,
        event: &horizon_client_platform::KeyboardEvent,
        key: Option<KeyCode>,
        viewport: (f32, f32),
    ) -> Action {
        let shortcut = self.shortcut(event);
        let Some(title) = &mut self.title else {
            return Action::None;
        };
        if let Some(shortcut) = shortcut {
            if title.has_caret_field() {
                title.field_shortcut(shortcut, &mut self.clipboard);
            } else if let Some((id, value, max)) = title.focused_field()
                && self
                    .selection
                    .apply(id, value, shortcut, &mut self.clipboard, |v, t| {
                        clipboard::append_limited(v, t, max)
                    })
            {
                title.search_changed();
            }
            return Action::None;
        }
        // `TabNavigationBar.keyPressed` comes before the open tab's own
        // widgets: Ctrl plus a digit or Tab picks a tab instead of reaching
        // them.
        let ctrl = if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        };
        if let Some(key) = key
            && title
                .tab_bar_key(key, ctrl, self.modifiers.shift_key())
                .is_some()
        {
            return Action::None;
        }
        match key {
            Some(KeyCode::Escape) => title.escape(),
            Some(KeyCode::F5) => title.refresh_servers(),
            Some(KeyCode::Tab) => {
                title.tab_dir(self.modifiers.shift_key(), viewport);
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter) => {
                return title.confirm_key_size(viewport).0;
            }
            Some(KeyCode::Space)
                if !title.has_caret_field()
                    && (title.list_focused() || title.focused_button.is_some()) =>
            {
                return title.confirm_key_size(viewport).0;
            }
            Some(
                key @ (KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::End),
            ) if title.has_caret_field() => {
                title.field_key(
                    key,
                    self.modifiers.control_key(),
                    self.modifiers.shift_key(),
                );
            }
            Some(
                key @ (KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown),
            ) => {
                title.arrow_key(key, viewport, self.modifiers.shift_key());
            }
            _ => {
                let cleared = !title.has_caret_field()
                    && title
                        .focused_field()
                        .is_some_and(|(id, value, _)| self.selection.take(id, value));
                if cleared {
                    title.search_changed();
                }
                if key == Some(KeyCode::Backspace) {
                    if !cleared {
                        title.backspace();
                    }
                } else if let Some(text) = &event.text {
                    title.type_text(text);
                }
            }
        }
        Action::None
    }
}

/// A panic's message: its `&str` or `String` payload.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("Box<dyn Any>")
}

/// Each generation has its own channel and progress. Pack changes wait for the
/// old tail before starting another generation, including cache/global writes.
pub(super) fn spawn_loader(
    options: &Options,
    settings: &Settings,
    vanilla_root: &std::path::Path,
    started: Instant,
    progress: &Progress,
) -> Result<(Receiver<LoaderMsg>, JoinHandle<()>)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let settings = settings.clone();
    let vanilla_root = vanilla_root.to_path_buf();
    let dev_mods = options.dev_mods.clone();
    let world = launch_world(options).map(str::to_owned);
    let progress = progress.clone();
    let handle = std::thread::Builder::new()
        .name("resource-loader".into())
        .spawn(move || {
            let mut pack_error = None;
            let root = packs::startup_root(&vanilla_root, &settings.resource_packs, &[])
                .unwrap_or_else(|error| {
                    pack_error = Some(error.to_string());
                    vanilla_root.clone()
                });
            let root = mods::content_root(&root, &settings.mods.enabled, &dev_mods);
            let result = load_resources(
                root,
                world.as_deref(),
                &vanilla_root,
                &settings,
                started,
                true,
                false,
                &progress,
                &mut |gui, root| {
                    let _ = sender.send(LoaderMsg::Ui(gui, root.to_path_buf()));
                },
            )
            .map(|mut loaded| {
                loaded.pack_error = loaded.pack_error.or(pack_error);
                loaded
            })
            .map_err(|error| error.to_string());
            let _ = sender.send(LoaderMsg::Done(result));
        })?;
    Ok((receiver, handle))
}

impl Loading {
    pub(super) fn poll_menu_resources(&mut self) -> Result<()> {
        // Once `Done` arrived the generation is over: the caller may have taken
        // its result, and the finished loader's channel is closed.
        if self.loaded.is_none() && !self.delivered {
            loop {
                match self.receiver.try_recv() {
                    Ok(LoaderMsg::Ui(gui, root)) => self.set_ui(gui, root),
                    Ok(LoaderMsg::Done(result)) => {
                        self.loaded = Some(result);
                        self.delivered = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // The loader sends `Done` before it returns: it panicked.
                        let reason = match self.handle.take().map(JoinHandle::join) {
                            Some(Err(payload)) => panic_text(payload.as_ref()).to_owned(),
                            Some(Ok(())) => "it returned without a result".into(),
                            None => "no loader thread was running".into(),
                        };
                        return Err(
                            format!("resource loader stopped before finishing: {reason}").into(),
                        );
                    }
                }
            }
        }
        self.drive_menu_mod_browser();
        self.restart_menu_resources()
    }
    /// The startup menu's Browse mods, as `Game::drive_mod_browser`.
    fn drive_menu_mod_browser(&mut self) {
        let menu = &mut self.menu;
        if menu
            .mod_jobs
            .poll(&mut menu.options.mod_browser, &mut self.settings.mods)
        {
            self.settings.save(&Game::options_path());
        }
    }
    pub(super) fn apply_menu_settings(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            horizon_client::presentation::set_gamma(renderer, self.settings.gamma);
            horizon_client::presentation::set_present_mode(
                renderer,
                self.settings.vsync,
                self.settings.horizon.present_mode,
            );
        }
        horizon_client::hud::GUI_SCALE.store(self.settings.gui_scale, Ordering::Relaxed);
        super::narrator::sync(&self.settings);
        if let Some(gui) = &mut self.gui {
            gui.atlas.font.uniform = self.settings.force_unicode;
            gui.atlas.font.japanese = self.settings.japanese_glyphs;
        }
        #[cfg(not(target_os = "cafe"))]
        if let Some(window) = self.window.clone() {
            let monitor = window
                .current_monitor()
                .or_else(|| window.primary_monitor());
            if let Some(modes) = self.video_modes.refresh(
                monitor,
                horizon_client_input::desktop_adapter::monitor_video_modes,
            ) {
                self.settings.video_modes = modes;
            }
            let preferred = self
                .settings
                .file
                .fullscreen_resolution
                .as_deref()
                .and_then(horizon_client::window::VideoMode::read);
            self.fullscreen.apply(
                &window,
                self.settings.fullscreen,
                self.settings.file.bool("exclusiveFullscreen"),
                preferred,
                false,
            );
        }
        self.update_menu_cursor();
    }

    fn update_menu_cursor(&mut self) {
        let batch = HudBatch::new(self.menu_size());
        self.menu.cursor = (
            self.menu.pointer.0 as f32 / batch.scale,
            self.menu.pointer.1 as f32 / batch.scale,
        );
    }

    fn menu_size(&self) -> [u32; 2] {
        self.renderer.as_ref().map_or(self.viewport, |r| {
            horizon_client::presentation::viewport_size(r)
        })
    }

    fn save_menu_address(&mut self) {
        if self.menu.title.as_ref().is_some_and(|title| {
            title.last_server != self.settings.file.string("lastServer")
                || title.skip_multiplayer_warning
                    != self.settings.file.bool("skipMultiplayerWarning")
        }) {
            self.save_menu_settings();
        }
    }

    fn save_menu_settings(&mut self) {
        if let Some(title) = &self.menu.title {
            self.settings.file.set(
                "lastServer",
                horizon_client::options_file::V::Str(title.last_server.clone()),
            );
            self.settings.file.set(
                "skipMultiplayerWarning",
                horizon_client::options_file::V::Bool(title.skip_multiplayer_warning),
            );
        }
        self.settings.save(&Game::options_path());
    }

    fn open_menu_options(&mut self, page: Page) {
        self.menu.options.open_at(false, page);
        self.menu.options.languages = horizon_client::lang::available(&[&self.menu.root]);
        self.menu.options.pack_infos = packs::pack_infos(&self.vanilla_root, &[]);
        self.menu.options.pack_selection = self.settings.resource_packs.clone();
        self.menu.options.pack_screen = None;
        self.menu.options_open = true;
    }

    fn menu_option_events(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Changed => {
                    self.save_menu_settings();
                    self.apply_menu_settings();
                }
                Event::SliderDrag => {
                    self.apply_menu_settings();
                }
                Event::Close if self.menu.options.resource_error.is_some() => {}
                Event::Close => {
                    self.menu.options_open = false;
                    self.menu.dragging = false;
                    self.save_menu_settings();
                }
                Event::Language(code) => {
                    horizon_client::lang::set(horizon_client::lang::Language::load(
                        &[&self.menu.root],
                        &code,
                    ));
                    self.save_menu_settings();
                }
                Event::Packs(_) => {
                    self.save_menu_settings();
                    self.menu.reload_packs = true;
                }
                Event::OpenPackFolder => {
                    let dir = packs::packs_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    packs::open_folder(&dir);
                }
                Event::DownloadPack(i) => {
                    let dir = packs::packs_dir();
                    if let Some(screen) = &mut self.menu.options.pack_screen
                        && let Some(browser) = &mut screen.browser
                    {
                        browser.download(i, &dir, horizon_core::VANILLA_VERSION);
                    }
                }
                Event::DataPacks(ids) => {
                    let resources = self.vanilla_root.clone();
                    if let Some(title) = &mut self.menu.title {
                        title.offer_data_packs(&resources, ids);
                    }
                }
                Event::OpenDataPackFolder(dir) => {
                    let _ = std::fs::create_dir_all(&dir);
                    packs::open_folder(&dir);
                }
                Event::OpenModFolder => {
                    let dir = Game::game_dir().join(horizon_client::mod_list::DIR);
                    let _ = std::fs::create_dir_all(&dir);
                    packs::open_folder(&dir);
                }
                Event::BrowseMods => {
                    let ui = &mut self.menu.options.mod_browser;
                    self.menu.mod_jobs.browse(ui, Game::game_dir());
                }
                Event::InstallMod(id) => {
                    let ui = &mut self.menu.options.mod_browser;
                    self.menu.mod_jobs.install(ui, Game::game_dir(), id);
                }
                Event::OpenModPage(url) => packs::open_uri(&url),
                Event::ListShaderPacks => {
                    self.settings.shader_packs =
                        horizon_client::shader_pack_list::scan(&Game::shader_packs_dir());
                }
                Event::ShaderPack => self.save_menu_settings(),
                Event::OpenShaderOptions => {
                    self.settings.shader_options = super::shader_packs::read_shader_options(
                        &self.settings.horizon.shader_pack,
                        &self.settings.language,
                        &horizon_shaders::Macros::standard(),
                    );
                }
                Event::ShaderOptions => {
                    if let Some(options) = self.settings.shader_options.take() {
                        super::shader_packs::write_shader_options(&options);
                    }
                }
                Event::NativePackOptions(i) => {
                    if let Some(pack) = self.settings.shader_packs.get(i) {
                        pack.save_values(&Game::shader_packs_dir());
                    }
                }
                Event::OpenShaderPackFolder => {
                    let dir = Game::shader_packs_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    packs::open_folder(&dir);
                }
                Event::Credits => {
                    if let Some(gui) = &self.gui {
                        let name = self.options.as_ref().map_or_else(
                            || horizon_client::offline_name::generate(0),
                            Options::player_name,
                        );
                        let screen = horizon_client::win_screen::WinScreen::new(
                            false,
                            &self.menu.root,
                            &name,
                            &gui.atlas.font,
                        );
                        self.menu.win = Some((screen, Instant::now()));
                    }
                }
                Event::Narrator => super::narrator::update_status(
                    self.settings.narrator(),
                    &mut self.menu.narrator_toast,
                ),
                // The startup menu has no screen stack for ConfirmLinkScreen;
                // the link opens directly, as the menu's other links do.
                Event::AccessibilityHelp => {
                    super::packs::open_uri(super::options::ACCESSIBILITY_HELP_LINK)
                }
                // Mods' settings pages open only in a running game.
                // The HUD editor needs a world: its button is inactive here.
                Event::NativeSettings(_) | Event::NativeButton(..) | Event::HudEditor => {}
                // The startup menu shares the sound engine with the game, so
                // ordinary clicks and category slider previews work here too.
                Event::ButtonClick => {
                    if let Some(audio) = &mut self.audio {
                        audio.play_button_click();
                    }
                }
                Event::Volume(source, volume) => {
                    if let Some(audio) = &mut self.audio {
                        audio.preview(source, volume);
                    }
                }
                // The startup menu runs before the game opens a control socket,
                // and has no music tracks to list or play.
                Event::CopyAgentDetails
                | Event::ToggleRecording
                | Event::ExportRepro
                | Event::ListMusicTracks
                | Event::PlayMusicTrack(..)
                => {}
                Event::OpenStudioFolder => {
                    let dir = std::path::PathBuf::from("recordings");
                    let _ = std::fs::create_dir_all(&dir);
                    packs::open_folder(&dir);
                }
            }
        }
    }

    /// `Game::narrator_hotkey` on the startup menu's screens.
    fn menu_narrator_hotkey(&mut self, event: &horizon_client_platform::KeyboardEvent) {
        let letter = clipboard::shortcut_letter(
            event.physical_key.unwrap_or(KeyCode::Other),
            event.logical_text.as_deref(),
        );
        if letter != Some('b') {
            return;
        }
        let menu = &mut self.menu;
        let control_with_quirk = if cfg!(target_os = "macos") {
            menu.modifiers.super_key()
        } else {
            menu.modifiers.control_key()
        };
        let edit_box_focused = if menu.options_open {
            menu.options.typing()
        } else if menu.win.is_some() || menu.pending.is_some() {
            false
        } else {
            menu.title
                .as_mut()
                .is_some_and(|t| t.has_caret_field() || t.focused_field().is_some())
        };
        let hotkey = super::narrator::is_hotkey(
            event.state == horizon_client_platform::KeyState::Pressed,
            control_with_quirk,
            letter,
            edit_box_focused,
            horizon_narrator::is_active(),
            self.settings.file.bool("narratorHotkey"),
        );
        if !hotkey {
            return;
        }
        let was_disabled = super::narrator::cycle_status(&mut self.settings);
        super::narrator::update_status(self.settings.narrator(), &mut self.menu.narrator_toast);
        self.save_menu_settings();
        self.menu
            .narration
            .status_changed(was_disabled, horizon_narrator::screen::Trigger::Keyboard);
    }

    fn menu_options_click(&mut self, drag: bool, right: bool) {
        let size = self.menu_size();
        let batch = HudBatch::new(size);
        let Some(gui) = &self.gui else { return };
        self.menu.options.shift = self.menu.modifiers.shift_key();
        let events = self.menu.options.click(
            &mut self.settings,
            &gui.atlas,
            self.menu.cursor.0,
            self.menu.cursor.1,
            (batch.width, batch.height),
            [size[0] as i32, size[1] as i32],
            drag,
            right,
        );
        self.menu_option_events(events);
    }

    /// The Realms screen's join, once its address came back: queued like a click on a server.
    pub(super) fn poll_menu_realms(&mut self, event_loop: &dyn EventLoopCtl) {
        if let Some(action) = self
            .menu
            .title
            .as_mut()
            .and_then(Title::take_connect_request)
        {
            self.menu_action(event_loop, action);
        }
    }

    fn menu_action(&mut self, event_loop: &dyn EventLoopCtl, action: Action) {
        match action {
            Action::Quit => event_loop.exit(),
            Action::Options => self.open_menu_options(Page::Main),
            Action::Accessibility => self.open_menu_options(Page::Accessibility),
            Action::Language => self.open_menu_options(Page::Language),
            Action::Sounds => self.open_menu_options(Page::Sound),
            Action::Credits => self.open_menu_options(Page::Credits),
            Action::OpenUri(uri) => super::packs::open_uri(&uri),
            Action::CopyText(text) => {
                use horizon_client::clipboard::Clipboard;
                self.menu.clipboard.set(&text);
            }
            Action::DataPacks { dir, selected } => {
                let resources = self.vanilla_root.clone();
                self.menu.options.open_data_packs(dir, selected, resources);
                self.menu.options_open = true;
            }
            action => {
                let name = self
                    .options
                    .as_ref()
                    .map_or_else(|| "Player".into(), |o| o.player_name());
                self.menu.queue_entry(action, name);
            }
        }
        self.save_menu_address();
    }

    /// The real mouse before the screen takes it: a move, a press or a
    /// scroll hands the pointer back from the keyboard (a press or a scroll
    /// where the mouse left it), and a left press focuses what it lands on.
    fn menu_key_focus_mouse(&mut self, event: &WindowEvent, trace_cafe_touch_press: bool) {
        match event {
            WindowEvent::CursorMoved { .. } => {
                self.menu.key_focus_release();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if trace_cafe_touch_press {
                    cafe_game_stage!("first Cafe touch press key focus release begin");
                }
                let released_focus = self.menu.key_focus_release();
                if trace_cafe_touch_press {
                    log::info!(
                        "Horizon Cafe touch: key focus release returned had_focus={released_focus}"
                    );
                    cafe_game_stage!("first Cafe touch press key focus release complete");
                }
                if released_focus {
                    if trace_cafe_touch_press {
                        cafe_game_stage!("first Cafe touch press cursor update begin");
                    }
                    self.update_menu_cursor();
                    if trace_cafe_touch_press {
                        cafe_game_stage!("first Cafe touch press cursor update complete");
                    }
                }
                if *state == ElementState::Pressed && *button == MouseButton::Left {
                    #[cfg(target_os = "cafe")]
                    {
                        // GamePad touch already supplies the pointer click. Do
                        // not run the desktop mouse-to-key-focus hit-test here;
                        // Cafe can hang in that target scan. No focus origin is
                        // available without the hit-test, so discard any stale one.
                        self.menu.key_origin = None;
                        if trace_cafe_touch_press {
                            cafe_game_stage!("first Cafe touch press focus target hit-test skipped");
                        }
                    }
                    #[cfg(not(target_os = "cafe"))]
                    {
                        let (cx, cy) = self.menu.cursor;
                        let targets = self.menu_key_targets_with_trace(trace_cafe_touch_press);
                        if trace_cafe_touch_press {
                            log::info!(
                                "Horizon Cafe touch: focus target hit-test begin cursor=({cx},{cy}) targets={}",
                                targets.len()
                            );
                            cafe_game_stage!("first Cafe touch press focus target hit-test begin");
                        }
                        let key_origin = targets
                            .into_iter()
                            .find(|t| {
                                let [x, y, w, h] = t.rect;
                                cx >= x && cy >= y && cx < x + w && cy < y + h
                            })
                            .map(|t| t.rect);
                        if trace_cafe_touch_press {
                            log::info!(
                                "Horizon Cafe touch: focus target hit-test complete selected={key_origin:?}"
                            );
                            cafe_game_stage!("first Cafe touch press focus target hit-test complete");
                        }
                        self.menu.key_origin = key_origin;
                    }
                }
            }
            WindowEvent::MouseWheel { .. } if self.menu.key_focus_release() => {
                self.update_menu_cursor();
            }
            _ => {}
        }
    }
    /// What the keyboard can focus on the screen last drawn.
    fn menu_key_targets(&self) -> Vec<Target> {
        self.menu_key_targets_with_trace(false)
    }

    fn menu_key_targets_with_trace(&self, trace_cafe_touch_press: bool) -> Vec<Target> {
        if trace_cafe_touch_press {
            cafe_game_stage!("first Cafe touch press focus viewport begin");
        }
        let batch = HudBatch::new(self.menu_size());
        if trace_cafe_touch_press {
            cafe_game_stage!("first Cafe touch press focus viewport complete");
            cafe_game_stage!("first Cafe touch press drawn targets begin");
        }
        let drawn = key_focus::drawn_targets();
        if trace_cafe_touch_press {
            log::info!("Horizon Cafe touch: drawn focus targets={}", drawn.len());
            cafe_game_stage!("first Cafe touch press drawn targets complete");
            cafe_game_stage!("first Cafe touch press on-screen filter begin");
        }
        let on_screen = key_focus::on_screen(drawn, (batch.width, batch.height));
        if trace_cafe_touch_press {
            log::info!(
                "Horizon Cafe touch: on-screen focus targets={}",
                on_screen.len()
            );
            cafe_game_stage!("first Cafe touch press on-screen filter complete");
            cafe_game_stage!("first Cafe touch press keyboard filter begin");
        }
        let targets = key_focus::keyboard_targets(on_screen, false);
        if trace_cafe_touch_press {
            log::info!(
                "Horizon Cafe touch: keyboard focus targets={}",
                targets.len()
            );
            cafe_game_stage!("first Cafe touch press keyboard filter complete");
        }
        targets
    }
    /// `Screen.keyPressed`'s focus navigation on the title shown while
    /// resources load, as `Game::key_focus_key` does it in the game (the
    /// pointer sits on the focus; a step past a list's visible rows doesn't
    /// scroll it here). Whether the key was used.
    fn menu_key_focus(&mut self, event_loop: &dyn EventLoopCtl, key: KeyCode) -> bool {
        let Some(nav) = key_focus::nav_of(key, self.menu.modifiers.shift_key()) else {
            return false;
        };
        self.menu_navigation(event_loop, nav, false)
    }

    /// Use the title's existing focus and click path for a typed controller
    /// button. This keeps Cafe input out of the keyboard event stream.
    pub(super) fn menu_pad_button(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        button: horizon_client::gamepad::Button,
        down: bool,
    ) {
        use horizon_client::gamepad::Button;
        use key_focus::Nav;

        if !down {
            return;
        }
        if let Some(keyboard) = &mut self.menu.osk {
            let output = keyboard.button(button);
            if let Some(output) = output
                && let Err(error) = self.menu_osk_output(event_loop, output)
            {
                log::warn!("Horizon: startup on-screen keyboard: {error}");
            }
            return;
        }
        #[cfg(target_os = "cafe")]
        if button == Button::South && self.menu_text_field_focused() {
            self.menu_open_osk(event_loop);
            return;
        }
        let nav = match button {
            Button::DPadUp => Nav::Arrow(Button::DPadUp),
            Button::DPadDown => Nav::Arrow(Button::DPadDown),
            Button::DPadLeft => Nav::Arrow(Button::DPadLeft),
            Button::DPadRight => Nav::Arrow(Button::DPadRight),
            Button::South if self.menu.key_focus.is_none() && self.menu.key_origin.is_none() => {
                Nav::Tab { forward: true }
            }
            Button::South => Nav::Select,
            Button::LeftBumper => Nav::Tab { forward: false },
            Button::RightBumper => Nav::Tab { forward: true },
            _ => return,
        };
        self.menu_navigation(event_loop, nav, true);
    }

    /// Whether East/Start should be sent to the title's open controller keyboard
    /// instead of the normal menu Escape action.
    pub(super) fn menu_osk_open(&self) -> bool {
        self.menu.osk.is_some() || self.menu.native_osk
    }

    #[cfg(target_os = "cafe")]
    fn menu_text_field_focused(&mut self) -> bool {
        if self.menu.options_open {
            self.menu.options.typing()
        } else {
            self.menu
                .title
                .as_mut()
                .is_some_and(|title| title.has_caret_field() || title.focused_field().is_some())
        }
    }

    #[cfg(target_os = "cafe")]
    fn menu_open_osk(&mut self, event_loop: &dyn EventLoopCtl) {
        if !self.menu_text_field_focused() || self.menu_osk_open() {
            return;
        }

        let (initial, max_utf16_units) = if self.menu.options_open {
            (String::new(), 1024usize)
        } else {
            self.menu
                .title
                .as_mut()
                .and_then(|title| {
                    title
                        .focused_field()
                        .map(|(_, value, max)| (value.clone(), max))
                })
                .unwrap_or_else(|| (String::new(), 1024))
        };
        if self.settings.controller.keyboard == horizon_client::gamepad::TextKeyboard::Auto
            && event_loop.open_text_input(&initial, max_utf16_units.clamp(1, 1024))
        {
            self.menu.native_osk = true;
            if self.menu.options_open {
                if let Some(gui) = &self.gui {
                    self.menu.options.shortcut(
                        &gui.atlas,
                        horizon_client::clipboard::EditShortcut::SelectAll,
                        &mut self.menu.clipboard,
                    );
                }
            } else if let Some(title) = &mut self.menu.title {
                title.field_shortcut(
                    horizon_client::clipboard::EditShortcut::SelectAll,
                    &mut self.menu.clipboard,
                );
            }
        } else {
            let batch = HudBatch::new(self.menu_size());
            self.menu.osk_at_top = self.menu.cursor.1 > batch.height / 2.0;
            self.menu.osk = Some(horizon_client::osk::Keyboard::default());
        }
    }

    fn menu_osk_output(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        output: horizon_client::osk::Output,
    ) -> Result<()> {
        use horizon_client::osk::Output;

        match output {
            Output::Text(text) => {
                if self.menu.options_open {
                    if let Some(gui) = &self.gui {
                        self.menu.options.type_text(&gui.atlas, &text);
                    }
                } else if let Some(title) = &mut self.menu.title {
                    title.type_text(&text);
                }
            }
            Output::Backspace => {
                if self.menu.options_open {
                    if let Some(gui) = &self.gui {
                        let events = self.menu.options.key(
                            &mut self.settings,
                            &gui.atlas,
                            KeyCode::Backspace,
                            None,
                        );
                        self.menu_option_events(events);
                    }
                } else if let Some(title) = &mut self.menu.title {
                    title.backspace();
                }
            }
            Output::Enter => {
                self.menu.osk = None;
                if self.menu.options_open {
                    return Ok(());
                }
                let size = HudBatch::new(self.menu_size());
                let action = self.menu.title.as_mut().map_or(Action::None, |title| {
                    title.confirm_key_size((size.width, size.height)).0
                });
                self.menu_action(event_loop, action);
            }
            Output::Close => self.menu.osk = None,
        }
        Ok(())
    }

    fn menu_ime_commit(&mut self, text: &str) {
        let replace = std::mem::take(&mut self.menu.native_osk);
        if self.menu.options_open {
            if let Some(gui) = &self.gui {
                if replace {
                    self.menu.options.shortcut(
                        &gui.atlas,
                        horizon_client::clipboard::EditShortcut::SelectAll,
                        &mut self.menu.clipboard,
                    );
                }
                self.menu.options.type_text(&gui.atlas, text);
            }
        } else if let Some(title) = &mut self.menu.title {
            if replace {
                title.field_shortcut(
                    horizon_client::clipboard::EditShortcut::SelectAll,
                    &mut self.menu.clipboard,
                );
            }
            title.type_text(text);
        }
    }

    fn menu_ime_disabled(&mut self) {
        if !std::mem::take(&mut self.menu.native_osk) {
            return;
        }
        if self.menu.options_open {
            if let Some(gui) = &self.gui {
                let events = self.menu.options.key(
                    &mut self.settings,
                    &gui.atlas,
                    KeyCode::ArrowRight,
                    None,
                );
                self.menu_option_events(events);
            }
        } else if let Some(title) = &mut self.menu.title {
            title.field_key(KeyCode::ArrowRight, false, false);
        }
    }

    pub(super) fn pad_stick_navigation_button(
        &mut self,
        state: horizon_client::gamepad::PadState,
        now: Instant,
    ) -> Option<horizon_client::gamepad::Button> {
        #[cfg(target_os = "cafe")]
        let trace_first_navigation = {
            static FIRST_PAD_NAVIGATION_TRACE: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(true);
            FIRST_PAD_NAVIGATION_TRACE.swap(false, std::sync::atomic::Ordering::Relaxed)
        };
        #[cfg(not(target_os = "cafe"))]
        let trace_first_navigation = false;

        if trace_first_navigation {
            cafe_game_stage!("first pad navigation wrapper begin");
        }
        let options_capturing = self.menu.options_open
            && (self.menu.options.selected_key.is_some()
                || self.menu.options.selected_pad_action.is_some());
        if trace_first_navigation {
            cafe_game_stage!("first pad navigation options check complete");
        }
        let enabled = self.settings.controller.enabled
            && self.gui.is_some()
            && self.menu.pending.is_none()
            && self.menu.win.is_none()
            && !options_capturing;
        if trace_first_navigation {
            cafe_game_stage!("first pad navigation enabled check complete");
        }
        let direction = self.menu.pad_stick_direction(
            state,
            self.settings.controller.move_deadzone,
            now,
            enabled,
        );
        if trace_first_navigation {
            cafe_game_stage!("first pad navigation direction call complete");
        }
        direction
    }

    /// Route the GamePad's Back/Menu controls through the same Escape handler
    /// used by the existing title, options, loading, and win screens.
    pub(super) fn menu_pad_escape(&mut self, event_loop: &dyn EventLoopCtl) {
        let was_synthetic = std::mem::replace(&mut self.menu.key_synthetic, true);
        for state in [ElementState::Pressed, ElementState::Released] {
            let keyboard = horizon_client_platform::KeyboardEvent {
                physical_key: Some(KeyCode::Escape),
                logical_key: LogicalKey::Named("Escape".into()),
                location: KeyLocation::Standard,
                state,
                logical_text: None,
                text: None,
                unmodified_text: None,
                repeat: false,
            };
            let event = WindowEvent::KeyboardInput {
                event: keyboard.clone(),
                is_synthetic: true,
            };
            if let Err(error) =
                self.menu_event(event_loop, event, Some(KeyCode::Escape), Some(keyboard))
            {
                log::warn!("Horizon: GamePad back action: {error}");
            }
        }
        self.menu.key_synthetic = was_synthetic;
    }

    fn menu_navigation(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        nav: key_focus::Nav,
        gamepad: bool,
    ) -> bool {
        let options = &self.menu.options;
        if self.gui.is_none()
            || self.menu.pending.is_some()
            || self.menu.win.is_some()
            || (self.menu.options_open
                && (options.selected_key.is_some() || options.selected_pad_action.is_some()))
        {
            return false;
        }
        let targets = self.menu_key_targets();
        if targets.is_empty() {
            return false;
        }
        let find = |rect: [f32; 4]| {
            targets
                .iter()
                .find(|t| t.rect.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.5))
                .copied()
        };
        let mut slider_edit = self.menu.key_slider_edit;
        let mut focus = self.menu.key_focus.and_then(|f| find(f.rect));
        if focus.is_none()
            && let Some(clicked) = self.menu.key_origin.and_then(find)
        {
            focus = Some(clicked);
            slider_edit = true;
        }
        let (typing, rows_take_enter) = if self.menu.options_open {
            (self.menu.options.typing(), false)
        } else if let Some(title) = &mut self.menu.title {
            (
                title.has_caret_field() || title.focused_field().is_some(),
                title.list_focused(),
            )
        } else {
            (false, false)
        };
        let batch = HudBatch::new(self.menu_size());
        let ctx = key_focus::Context {
            slider_edit: slider_edit
                || (gamepad
                    && matches!(
                        nav,
                        key_focus::Nav::Arrow(
                            horizon_client::gamepad::Button::DPadLeft
                                | horizon_client::gamepad::Button::DPadRight
                        )
                    )),
            typing,
            rows_take_enter: rows_take_enter && !gamepad,
            gui: (batch.width, batch.height),
        };
        let act = key_focus::act(nav, focus, &targets, ctx);
        if act == Act::Pass {
            return false;
        }
        if self.menu.key_focus.is_none() {
            self.menu.key_screen = self.menu.key_screen();
        }
        self.menu.key_origin = None;
        self.menu.key_focus = focus;
        self.menu.key_slider_edit = slider_edit;
        match (act, focus) {
            (Act::Focus(i), _) => {
                self.menu_key_focus_on(event_loop, targets[i]);
                self.menu.key_slider_edit = key_focus::slider_edit_on_focus(nav);
            }
            (Act::Step(button), Some(focus)) => {
                let rects: Vec<[f32; 4]> = targets.iter().map(|t| t.rect).collect();
                if let Some(i) =
                    horizon_client::gamepad::focus_step_rect(focus.rect, button, &rects)
                {
                    self.menu_key_focus_on(event_loop, targets[i]);
                }
            }
            (Act::Press, Some(focus)) => self.menu_key_click(event_loop, focus.center()),
            (Act::ToggleSlider, _) => self.menu.key_slider_edit = !slider_edit,
            (
                Act::Nudge(sign),
                Some(
                    focus @ Target {
                        kind: Kind::Slider { value, track },
                        ..
                    },
                ),
            ) => {
                let from = match self.menu.key_nudge {
                    Some((rect, x)) if rect == focus.rect => x,
                    _ => horizon_client::gamepad::slider_x(track, value),
                };
                let x = (from + sign).clamp(track[0], track[0] + track[1]);
                self.menu.key_nudge = Some((focus.rect, x));
                self.menu_key_click(event_loop, (x, focus.center().1));
            }
            _ => {}
        }
        if let Some(Target {
            kind: Kind::Slider { .. },
            ..
        }) = self.menu.key_focus
        {
            use horizon_client::screen_narrator::InputType;
            self.menu.narration.set_input(if self.menu.key_slider_edit {
                InputType::KeyboardTab
            } else {
                InputType::KeyboardArrow
            });
        }
        true
    }

    /// The keyboard's focus lands on `target`, the pointer with it (see
    /// `Game::key_focus_landed`).
    fn menu_key_focus_on(&mut self, event_loop: &dyn EventLoopCtl, target: Target) {
        let left = self.menu.key_focus.replace(target);
        self.menu.key_nudge = None;
        self.menu.key_slider_edit = false;
        self.menu.cursor = target.center();
        if left.is_some_and(|l| l.rect == target.rect) {
            return;
        }
        if left.is_some_and(|l| l.kind == Kind::Field)
            && target.kind != Kind::Field
            && !self.menu.options_open
            && let Some(title) = &mut self.menu.title
        {
            title.blur_field();
        }
        match target.kind {
            Kind::Field => self.menu_key_click(event_loop, key_focus::field_click(target.rect)),
            Kind::Row if !self.menu.options_open => {
                let batch = HudBatch::new(self.menu_size());
                if let Some(title) = &mut self.menu.title {
                    title.focus_world_row(target.center(), (batch.width, batch.height));
                }
            }
            _ => {}
        }
    }

    /// A left click at `at` (GUI pixels), the pointer staying there.
    fn menu_key_click(&mut self, event_loop: &dyn EventLoopCtl, at: (f32, f32)) {
        self.menu.cursor = at;
        let was = std::mem::replace(&mut self.menu.key_synthetic, true);
        for state in [ElementState::Pressed, ElementState::Released] {
            let click = WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
            };
            if let Err(e) = self.menu_event(event_loop, click, None, None) {
                log::warn!("Horizon: keyboard focus click: {e}");
            }
        }
        self.menu.key_synthetic = was;
    }

    pub(super) fn menu_event(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        event: WindowEvent,
        key: Option<KeyCode>,
        keyboard: Option<horizon_client_platform::KeyboardEvent>,
    ) -> Result<()> {
        #[cfg(target_os = "cafe")]
        let trace_cafe_touch_press = {
            static FIRST_TOUCH_PRESS: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(true);
            !self.menu.key_synthetic
                && matches!(
                    &event,
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    }
                )
                && FIRST_TOUCH_PRESS.swap(false, std::sync::atomic::Ordering::Relaxed)
        };
        #[cfg(not(target_os = "cafe"))]
        let trace_cafe_touch_press = false;

        #[cfg(target_os = "cafe")]
        if let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        } = &event
            && self.menu.osk.is_some()
        {
            let Some(gui) = &self.gui else {
                return Ok(());
            };
            let (cursor, size) = {
                let mut batch = HudBatch::new(self.menu_size());
                let ui =
                    horizon_client::ui_theme::Ui::new(&gui.atlas, &mut batch, self.menu.cursor);
                (ui.cursor, (ui.w, ui.h))
            };
            let keyboard_press = self
                .menu
                .osk
                .as_mut()
                .map(|keyboard| keyboard.pointer_press(cursor, size, self.menu.osk_at_top));
            if let Some((true, output)) = keyboard_press {
                if let Some(output) = output {
                    self.menu_osk_output(event_loop, output)?;
                }
                return Ok(());
            }
        }

        if trace_cafe_touch_press {
            cafe_game_stage!("first Cafe touch press entered title menu handler");
        }
        if let (Some(event), Some(key)) = (keyboard.as_ref(), key) {
            if event.state == horizon_client_platform::KeyState::Pressed {
                self.menu.physical_keys.insert(key);
            } else {
                self.menu.physical_keys.remove(&key);
            }
        }
        if let Some(keyboard) = keyboard.as_ref() {
            self.menu_narrator_hotkey(keyboard);
        }
        // The real mouse takes the pointer back from the keyboard's focus.
        let synthetic = self.menu.key_synthetic;
        if !synthetic {
            self.menu_key_focus_mouse(&event, trace_cafe_touch_press);
        }
        if trace_cafe_touch_press {
            cafe_game_stage!("first Cafe touch press focus handoff complete");
        }
        // Fullscreen (F11) is the window's: not the screen's input; the
        // keyboard's own clicks aren't the mouse.
        let f11 = key == Some(KeyCode::F11);
        if !f11 && !synthetic {
            super::narrator::narrate_input(&mut self.menu.narration, &event, self.menu.cursor, key);
        }
        if trace_cafe_touch_press {
            cafe_game_stage!("first Cafe touch press narration complete");
        }
        // Screen.keyPressed: Tab, the arrows and Enter move and press the
        // focus, unless the focused widget or the screen keeps them.
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
            && key_event.state == ElementState::Pressed
            && let Some(key) = key
            && self.menu_key_focus(event_loop, key)
        {
            return Ok(());
        }
        match event {
            WindowEvent::CloseRequested => {
                if !self.settings.quit_shortcuts
                    && horizon_client::input::is_quit_shortcut_down(
                        self.menu.modifiers,
                        &self.menu.physical_keys,
                    )
                {
                    return Ok(());
                }
                self.save_menu_settings();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.viewport = [size.width, size.height];
                if let Some(r) = &mut self.renderer {
                    horizon_client::presentation::resize(r, size.width, size.height);
                }
                self.apply_menu_settings();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.menu.modifiers = modifiers;
            }
            WindowEvent::Ime(ImeEvent::Commit(text)) => self.menu_ime_commit(&text),
            WindowEvent::Ime(ImeEvent::Disabled) => self.menu_ime_disabled(),
            WindowEvent::Ime(ImeEvent::Enabled | ImeEvent::Preedit(_, _)) => {}
            // `PackSelectionScreen.onFilesDrop`: a pack dropped on the window
            // joins the Create New World screen's data packs.
            WindowEvent::DroppedFile(path) => {
                if let Some(title) = &mut self.menu.title {
                    title.drop_file(path);
                }
            }
            WindowEvent::Focused(false) => {
                self.menu.modifiers = Modifiers::empty();
                self.menu.physical_keys.clear();
                self.menu.dragging = false;
                if let Some(title) = &mut self.menu.title {
                    title.release();
                    title.release_field();
                }
                let events = self.menu.options.release();
                self.menu_option_events(events);
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && self.menu.pending.is_some() =>
            {
                if key == Some(KeyCode::Escape) {
                    self.menu.pending = None;
                    self.menu.perf.cancel_world_opening();
                }
            }
            _ if self.gui.is_none() || self.menu.pending.is_some() => {}
            WindowEvent::KeyboardInput { event, .. } if self.menu.win.is_some() => {
                if let Some(code) = key {
                    let pressed = event.state == ElementState::Pressed;
                    if code == KeyCode::Escape && pressed {
                        self.menu.win = None;
                    } else if let Some(key) = horizon_client::win_screen::Key::from_code(code)
                        && let Some((win, _)) = &mut self.menu.win
                    {
                        win.key(key, pressed);
                    }
                }
            }
            WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. }
                if self.menu.win.is_some() => {}
            WindowEvent::CursorMoved { position, .. } => {
                self.menu.pointer = (position.x, position.y);
                self.update_menu_cursor();
                if self.menu.options_open && self.menu.dragging {
                    self.menu_options_click(true, false);
                } else {
                    let size = self.menu_size();
                    let Some(title) = &mut self.menu.title else {
                        return Ok(());
                    };
                    let b = HudBatch::new(size);
                    title.drag(self.menu.cursor, (b.width, b.height));
                    title.drag_field(&self.gui.as_ref().unwrap().atlas, self.menu.cursor.0);
                }
            }
            WindowEvent::MouseInput { state, button, .. } if self.menu.options_open => {
                if state == ElementState::Pressed && self.menu.options.selected_key.is_some() {
                    let events = self.menu.options.bind_mouse(&mut self.settings, button);
                    self.menu_option_events(events);
                } else if matches!(button, MouseButton::Left | MouseButton::Right) {
                    if state == ElementState::Pressed {
                        self.menu.dragging = button == MouseButton::Left;
                        self.menu_options_click(false, button == MouseButton::Right);
                        #[cfg(target_os = "cafe")]
                        self.menu_open_osk(event_loop);
                    } else if button == MouseButton::Left {
                        self.menu.dragging = false;
                        let events = self.menu.options.release();
                        self.menu_option_events(events);
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if trace_cafe_touch_press {
                    cafe_game_stage!("first Cafe touch press left-button branch begin");
                }
                let batch = HudBatch::new(self.menu_size());
                if let (Some(title), Some(gui)) = (&mut self.menu.title, &self.gui) {
                    if trace_cafe_touch_press {
                        cafe_game_stage!("first Cafe touch press title resources available");
                    }
                    if state == ElementState::Released {
                        title.release();
                        title.release_field();
                    } else {
                        if trace_cafe_touch_press {
                            cafe_game_stage!("first Cafe touch press Title::click begin");
                        }
                        let (action, activated) = title.click(
                            self.menu.cursor.0,
                            self.menu.cursor.1,
                            (batch.width, batch.height),
                        );
                        if trace_cafe_touch_press {
                            log::info!(
                                "Horizon Cafe touch: Title::click returned cursor={:?} viewport={:?} activated={} action={action:?}",
                                self.menu.cursor,
                                [batch.width, batch.height],
                                activated
                            );
                            cafe_game_stage!("first Cafe touch press Title::click complete");
                        }
                        title.press_field(
                            &gui.atlas,
                            self.menu.cursor,
                            (batch.width, batch.height),
                            self.menu.modifiers.shift_key(),
                        );
                        if trace_cafe_touch_press {
                            cafe_game_stage!("first Cafe touch press field handling complete");
                        }
                        title.press_rules(
                            &gui.atlas,
                            self.menu.cursor,
                            (batch.width, batch.height),
                        );
                        if trace_cafe_touch_press {
                            cafe_game_stage!("first Cafe touch press rules handling complete");
                            cafe_game_stage!("first Cafe touch press menu action begin");
                        }
                        self.menu_action(event_loop, action);
                        #[cfg(target_os = "cafe")]
                        if state == ElementState::Pressed {
                            self.menu_open_osk(event_loop);
                        }
                        if trace_cafe_touch_press {
                            cafe_game_stage!("first Cafe touch press menu action complete");
                        }
                    }
                } else if trace_cafe_touch_press {
                    cafe_game_stage!("first Cafe touch press title resources unavailable");
                }
            }
            // AbstractSelectionList.mouseClicked: a world row takes a click
            // of any button, so the middle one selects it without joining.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                let batch = HudBatch::new(self.menu_size());
                if let Some(title) = &mut self.menu.title {
                    title.select_world_at(
                        self.menu.cursor.0,
                        self.menu.cursor.1,
                        (batch.width, batch.height),
                    );
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let raw = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 30.0) as f32,
                };
                let amount = if self.settings.discrete_scroll {
                    horizon_client::mouse::java_signum(f64::from(raw)) as f32
                } else {
                    raw
                } * self.settings.mouse_wheel_sensitivity;
                let size = self.menu_size();
                let batch = HudBatch::new(size);
                if self.menu.options_open {
                    let events = self.menu.options.wheel(
                        &mut self.settings,
                        (batch.width, batch.height),
                        self.menu.cursor,
                        [size[0] as i32, size[1] as i32],
                        amount,
                    );
                    self.menu_option_events(events);
                } else if let Some(title) = &mut self.menu.title {
                    title.scroll_worlds(amount, self.menu.cursor, (batch.width, batch.height));
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if key == Some(KeyCode::F11) {
                    self.settings.fullscreen = !self.settings.fullscreen;
                    self.apply_menu_settings();
                    self.save_menu_settings();
                } else if self.menu.options_open {
                    let Some(keyboard) = keyboard.as_ref() else {
                        return Ok(());
                    };
                    let shortcut = self.menu.shortcut(keyboard);
                    let atlas = &self.gui.as_ref().unwrap().atlas;
                    let handled = shortcut.is_some_and(|s| {
                        self.menu
                            .options
                            .shortcut(atlas, s, &mut self.menu.clipboard)
                    });
                    if !handled && let Some(key) = key {
                        let events = self.menu.options.key(
                            &mut self.settings,
                            atlas,
                            key,
                            event.text.as_deref(),
                        );
                        self.menu_option_events(events);
                    }
                } else {
                    let Some(keyboard) = keyboard.as_ref() else {
                        return Ok(());
                    };
                    let batch = HudBatch::new(self.menu_size());
                    let action = self
                        .menu
                        .title_key(keyboard, key, (batch.width, batch.height));
                    self.menu_action(event_loop, action);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn draw_menu(&mut self, batch: &mut HudBatch) {
        let logs = horizon_client::logging::recent_lines();
        let Some(gui) = &self.gui else {
            // The pack's screens do not exist yet: the boot screen holds the
            // window, the wordmark and the console's lines while they load.
            horizon_client::ui_theme::draw_boot(&self.boot, batch, &logs);
            return;
        };
        if self.menu.pending.is_some() {
            horizon_client::level_loading::draw_loading_overlay(
                batch,
                &gui.atlas,
                self.progress.line(),
                self.progress.fraction(),
                self.settings.file.bool("darkMojangStudiosBackground"),
            );
        } else if let Some((win, frame)) = &mut self.menu.win {
            let now = Instant::now();
            // 20 ticks a second: the startup menu runs no tick rate.
            let delta_ticks = now.duration_since(*frame).as_secs_f32() * 20.0;
            *frame = now;
            horizon_client::widgets::begin_screen(|| win.screen_info());
            win.draw(batch, &gui.atlas, delta_ticks, Default::default());
            if win.finished(batch.height) {
                self.menu.win = None;
            }
        } else if self.menu.options_open {
            self.menu
                .options
                .draw(&self.settings, batch, &gui.atlas, self.menu.cursor);
        } else if let Some(title) = &mut self.menu.title {
            title.poll_backup();
            if std::mem::take(&mut title.onboarding_finished) {
                // Options.onboardingAccessibilityFinished.
                self.settings.file.set(
                    "onboardAccessibility",
                    horizon_client::options_file::V::Bool(false),
                );
                self.settings.save(&Game::options_path());
            }
            let _ = title.update_toast(self.settings.notification_display_time);
            for found in title.lan_found_narrations((batch.width, batch.height)) {
                horizon_narrator::say_system_queued(&found.plain());
            }
            let items = match &self.loaded {
                Some(Ok(loaded)) => Some(&*loaded.pack.item_renderer),
                _ => None,
            };
            title.draw_with_items(
                &gui.atlas,
                items,
                batch,
                self.menu.cursor,
                self.settings.file.bool("hideSplashTexts"),
                self.settings.file.bool("hideServerAddress"),
            );
        }
        #[cfg(target_os = "cafe")]
        {
            let text_field_active = self.menu.pending.is_none()
                && self.menu.win.is_none()
                && if self.menu.options_open {
                    self.menu.options.typing()
                } else {
                    self.menu.title.as_mut().is_some_and(|title| {
                        title.has_caret_field() || title.focused_field().is_some()
                    })
                };
            if !text_field_active {
                self.menu.osk = None;
                self.menu.native_osk = false;
            }
            if let (Some(keyboard), Some(gui)) = (&self.menu.osk, &self.gui) {
                let mut ui = horizon_client::ui_theme::Ui::new(&gui.atlas, batch, self.menu.cursor);
                let glyphs = self
                    .settings
                    .controller
                    .glyphs
                    .resolve(horizon_client::gamepad::GlyphStyle::Nintendo);
                keyboard.draw(
                    &mut ui,
                    self.menu.osk_at_top,
                    glyphs,
                    glyphs == horizon_client::gamepad::GlyphStyle::Nintendo
                        && self.settings.controller.nintendo_letters,
                );
                ui.finish();
            }
        }
        if let Some(focus) = self.menu.key_focus
            && self.menu.pending.is_none()
            && self.menu.win.is_none()
        {
            key_focus::draw_outline(&gui.atlas, batch, focus);
        }
        // Over every screen, as the game's; no sound before the sound engine.
        let title_toast = self.menu.title.as_ref().is_some_and(|t| t.has_toast());
        super::narrator::draw_toast(
            &mut self.menu.narrator_toast,
            u32::from(title_toast),
            f64::from(self.settings.notification_display_time),
            &gui.atlas,
            batch,
            false,
        );
    }

    pub(super) fn restart_menu_resources(&mut self) -> Result<()> {
        if !self.menu.reload_packs || self.loaded.is_none() {
            return Ok(());
        }
        if let Some(Ok(loaded)) = &mut self.loaded {
            if let Some(loader) = &loaded.rest_loader
                && !loader.ready()?
            {
                return Ok(());
            }
            if let Some(loader) = loaded.rest_loader.take()
                && let Err(error) = loader.finish()
            {
                log::warn!("Horizon: replaced resource generation failed: {error}");
            }
        }
        self.loaded = None;
        self.delivered = false;
        if let Some(handle) = self.handle.take() {
            handle.join().map_err(|payload| {
                format!("resource loader panicked: {}", panic_text(payload.as_ref()))
            })?;
        }
        self.progress = Progress::new();
        let options = self.options.as_ref().ok_or("loading already finished")?;
        let (receiver, handle) = spawn_loader(
            options,
            &self.settings,
            &self.vanilla_root,
            self.started,
            &self.progress,
        )?;
        self.receiver = receiver;
        self.handle = Some(handle);
        self.menu.reload_packs = false;
        Ok(())
    }
}

fn pad_scroll_lines(remainder: &mut f32, axis: f32, speed: f32, seconds: f32) -> f32 {
    if axis == 0.0 {
        *remainder = 0.0;
        return 0.0;
    }
    *remainder += axis * speed * seconds;
    let lines = remainder.trunc();
    *remainder -= lines;
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_scroll_accumulates_fractional_wheel_lines_and_resets_on_release() {
        let mut remainder = 0.0;
        let axis = horizon_client::gamepad::radial([0.0, 0.6], 0.2)[1];
        assert!((axis - 0.5).abs() < 0.0001);
        for _ in 0..3 {
            assert_eq!(pad_scroll_lines(&mut remainder, axis, 12.0, 0.05), 0.0);
        }
        assert_eq!(pad_scroll_lines(&mut remainder, axis, 12.0, 0.05), 1.0);
        assert!((remainder - 0.2).abs() < 0.0001);

        assert_eq!(pad_scroll_lines(&mut remainder, 0.0, 12.0, 0.05), 0.0);
        assert_eq!(remainder, 0.0);
    }

    #[test]
    fn pad_scroll_is_suppressed_while_settings_are_open() {
        use horizon_client::gamepad::PadState;

        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        loading.menu.options_open = true;
        loading.menu.pad_scroll = 0.75;
        let mut settings = Settings::default();
        settings.controller.enabled = true;
        let mut state = PadState::default();
        state.right = [0.0, 1.0];
        loading
            .menu
            .pad_stick_scroll(state, &settings, Instant::now(), [1280, 720]);
        assert_eq!(loading.menu.pad_scroll, 0.0);
    }

    #[test]
    fn left_stick_navigation_repeats_and_resets_for_dpad_and_deadzone() {
        use horizon_client::gamepad::{Button, PadState};

        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        let settings = Settings::default();
        let deadzone = settings.controller.move_deadzone;
        let now = Instant::now();
        let mut state = PadState::default();
        state.left = [0.0, 1.0];

        assert_eq!(
            loading.menu.pad_stick_direction(state, deadzone, now, true),
            Some(Button::DPadUp)
        );
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(349),
                true,
            ),
            None
        );
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(350),
                true,
            ),
            Some(Button::DPadUp)
        );
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(470),
                true,
            ),
            Some(Button::DPadUp)
        );

        state.left = [1.0, 0.0];
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(471),
                true,
            ),
            Some(Button::DPadRight)
        );
        state.set(Button::DPadLeft, true);
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(480),
                true,
            ),
            None
        );
        state.set(Button::DPadLeft, false);
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(481),
                true,
            ),
            Some(Button::DPadRight)
        );
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(482),
                false,
            ),
            None
        );
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(483),
                true,
            ),
            Some(Button::DPadRight)
        );
        state.left = [0.0, 0.0];
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(484),
                true,
            ),
            None
        );
    }

    #[test]
    fn left_stick_keeps_its_axis_when_diagonal_samples_are_noisy() {
        use horizon_client::gamepad::{Button, PadState};

        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        let deadzone = Settings::default().controller.move_deadzone;
        let now = Instant::now();
        let mut state = PadState::default();
        state.left = [1.0, 0.0];
        assert_eq!(
            loading.menu.pad_stick_direction(state, deadzone, now, true),
            Some(Button::DPadRight)
        );

        state.left = [0.70, 0.72];
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(10),
                true,
            ),
            None,
            "small diagonal noise should not switch an active horizontal navigation"
        );
        state.left = [0.45, 0.89];
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(20),
                true,
            ),
            Some(Button::DPadUp)
        );
        state.left = [0.70, 0.72];
        assert_eq!(
            loading.menu.pad_stick_direction(
                state,
                deadzone,
                now + Duration::from_millis(30),
                true,
            ),
            None,
            "the new vertical direction should also survive tie noise"
        );
    }

    #[test]
    fn cafe_left_stick_moves_existing_title_focus_targets() {
        use horizon_client::gamepad::{Button, PadState};
        use horizon_client::title::Mode;

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join(format!(
            "horizon-cafe-left-stick-nav-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        session.loading.set_ui(gui(&root), root.clone());
        session.loading.menu.title.as_mut().unwrap().mode = Mode::Main;
        let title_screen = session.loading.menu.title.as_ref().unwrap().screen_key();
        horizon_client::widgets::set_action_screen(title_screen);
        draw_menu_once(&mut session.loading);
        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));

        let targets = session.loading.menu_key_targets();
        let rects: Vec<_> = targets.iter().map(|target| target.rect).collect();
        let (from, direction, to) = targets
            .iter()
            .find_map(|from| {
                [
                    Button::DPadUp,
                    Button::DPadDown,
                    Button::DPadLeft,
                    Button::DPadRight,
                ]
                .into_iter()
                .find_map(|direction| {
                    horizon_client::gamepad::focus_step_rect(from.rect, direction, &rects)
                        .map(|next| (*from, direction, targets[next]))
                })
            })
            .expect("title menu has at least two navigable targets");
        session.loading.menu.key_focus = Some(from);
        session.loading.menu.key_screen = session.loading.menu.key_screen();

        let mut state = PadState::default();
        state.left = match direction {
            Button::DPadUp => [0.0, 1.0],
            Button::DPadDown => [0.0, -1.0],
            Button::DPadLeft => [-1.0, 0.0],
            Button::DPadRight => [1.0, 0.0],
            _ => unreachable!(),
        };
        session.dispatch_pad_state(&control, state);
        assert_eq!(session.loading.menu.key_focus, Some(to));
        session.dispatch_pad_state(&control, state);
        assert_eq!(session.loading.menu.key_focus, Some(to));

        drop(session);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cafe_right_stick_scrolls_the_original_title_world_list() {
        use horizon_client::gamepad::PadState;
        use horizon_client::title::{Mode, WorldEntry};

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join(format!(
            "horizon-cafe-pad-scroll-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        session.loading.set_ui(gui(&root), root.clone());
        let mut title = Title::new(root.join("saves"), &root);
        title.mode = Mode::Select {
            worlds: (0..12)
                .map(|i| WorldEntry {
                    folder: format!("world{i}"),
                    name: format!("World {i}"),
                    game_mode: 0,
                    hardcore: false,
                    modified: std::time::SystemTime::UNIX_EPOCH,
                    commands: false,
                    version: "26.3".into(),
                    locked: false,
                    corrupted: false,
                    incompatible: None,
                    file_fixing: false,
                    downgrade: false,
                })
                .collect(),
            selected: Some(0),
        };
        session.loading.menu.title = Some(title);
        session.loading.settings.controller.enabled = true;
        draw_menu_once(&mut session.loading);
        let row_rects = |loading: &Loading| {
            loading
                .menu_key_targets()
                .into_iter()
                .filter(|target| target.kind == Kind::Row)
                .map(|target| target.rect)
                .collect::<Vec<_>>()
        };
        let before = row_rects(&session.loading);
        assert!(
            !before.is_empty(),
            "world list has visible controller targets"
        );

        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));
        let mut state = PadState::default();
        state.right = [0.0, -1.0];
        for _ in 0..3 {
            session.dispatch_pad_state(&control, state);
            std::thread::sleep(Duration::from_millis(50));
        }
        draw_menu_once(&mut session.loading);
        let after = row_rects(&session.loading);
        assert_ne!(before, after, "right-stick wheel steps move the title rows");

        drop(session);
        let _ = std::fs::remove_dir_all(root);
    }

    fn gui(root: &std::path::Path) -> UiResources {
        UiResources {
            atlas: HudAtlas::fonts(root).unwrap(),
            panorama: Arc::new(horizon_client::gfx::panorama::Faces::colour([
                96, 110, 111, 255,
            ])),
        }
    }

    fn loading(receiver: Receiver<LoaderMsg>) -> Loading {
        Loading {
            window: None,
            renderer: None,
            pad_edges: Default::default(),
            viewport: [1280, 720],
            boot: horizon_client::hud::HudAtlas::boot(),
            gui: None,
            audio: None,
            settings: Settings::default(),
            fullscreen: horizon_client::window::FullscreenState::new(false),
            video_modes: Default::default(),
            options: None,
            vanilla_root: PathBuf::new(),
            pack_error: None,
            started: Instant::now(),
            panorama_rotation: Default::default(),
            progress: Progress::new(),
            receiver,
            loaded: None,
            delivered: false,
            handle: None,
            shown: false,
            ui_at: None,
            title_drawn: false,
            menu: StartupMenu::default(),
            error: None,
        }
    }

    fn draw_menu_once(loading: &mut Loading) {
        loading
            .draw_with([854, 480], |_, _| Ok(false), false, |_| {})
            .unwrap();
    }

    struct GuiScaleRestore(u32);

    impl GuiScaleRestore {
        fn new() -> Self {
            Self(horizon_client::hud::GUI_SCALE.load(Ordering::Relaxed))
        }
    }

    impl Drop for GuiScaleRestore {
        fn drop(&mut self) {
            horizon_client::hud::GUI_SCALE.store(self.0, Ordering::Relaxed);
        }
    }

    #[test]
    fn loading_start_uses_the_real_menu_and_reports_loader_failure() {
        const CHILD: &str = "HORIZON_WIIU_TEST_LOADING_CHILD";
        const SUCCESS: &str = "HORIZON_WIIU_REAL_LOADER_JOINED";
        if std::env::var_os(CHILD).as_deref() != Some(std::ffi::OsStr::new("1")) {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let temp = std::env::temp_dir().canonicalize().unwrap();
            let scratch = temp.join(format!(
                "horizon-startup-test-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir(&scratch).unwrap();
            let scratch = scratch.canonicalize().unwrap();
            assert_eq!(scratch.parent(), Some(temp.as_path()));
            struct Scratch(PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
            let scratch = Scratch(scratch);
            for name in ["horizon", "launcher", "data", "config"] {
                std::fs::create_dir(scratch.0.join(name)).unwrap();
            }
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "--nocapture",
                    "game::startup_menu::tests::loading_start_uses_the_real_menu_and_reports_loader_failure",
                ])
                .current_dir(&scratch.0)
                .env(CHILD, "1")
                .env("HORIZON_HOME", scratch.0.join("horizon"))
                .env("HORIZON_LAUNCHER_HOME", scratch.0.join("launcher"))
                .env("XDG_DATA_HOME", scratch.0.join("data"))
                .env("XDG_CONFIG_HOME", scratch.0.join("config"))
                .env("HORIZON_LAUNCHER_TRASH", scratch.0.join("trash"))
                .env("HORIZON_LAUNCHER_CLIPBOARD", scratch.0.join("clipboard"))
                .env("HORIZON_LAUNCHER_DESKTOP", scratch.0.join("desktop"))
                .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
                .env("HORIZON_NO_AUDIO", "1")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "isolated startup test failed:\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains(SUCCESS));
            return;
        }
        // Never let an inherited child sentinel direct the real loader at a
        // player's working directory or homes.
        let scratch = std::env::current_dir().unwrap().canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(scratch.parent(), Some(temp.as_path()));
        assert!(
            scratch
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("horizon-startup-test-")
        );
        for (var, name) in [
            ("HORIZON_HOME", "horizon"),
            ("HORIZON_LAUNCHER_HOME", "launcher"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CONFIG_HOME", "config"),
            ("HORIZON_LAUNCHER_TRASH", "trash"),
            ("HORIZON_LAUNCHER_CLIPBOARD", "clipboard"),
            ("HORIZON_LAUNCHER_DESKTOP", "desktop"),
        ] {
            assert_eq!(
                std::env::var_os(var).map(PathBuf::from),
                Some(scratch.join(name))
            );
        }
        assert_eq!(
            std::env::var("HORIZON_LAUNCHER_NO_SELF_UPDATE").as_deref(),
            Ok("1")
        );
        assert_eq!(std::env::var("HORIZON_NO_AUDIO").as_deref(), Ok("1"));
        let root = scratch.join("missing-resources");
        assert!(!root.exists());
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.world = Some("TestWorld".into());
        options.seed = 42;
        options.creative = true;
        let started = Instant::now();
        let mut loading = Loading::start(
            options,
            Settings::default(),
            root.clone(),
            Some("pack error".into()),
            started,
        )
        .unwrap();
        assert_eq!(loading.vanilla_root, root);
        assert_eq!(loading.started, started);
        assert!(loading.window.is_none());
        assert!(loading.renderer.is_none());
        assert!(loading.menu.options_open);
        assert_eq!(
            loading.menu.options.resource_error.as_deref(),
            Some("pack error")
        );
        assert!(matches!(
            &loading.menu.pending,
            Some(PendingWorldEntry::Local { folder, seed: Some(42), creative: true, .. })
                if folder == "TestWorld"
        ));

        loading.handle.take().unwrap().join().unwrap();
        let result = loading
            .receiver
            .try_iter()
            .find_map(|message| match message {
                LoaderMsg::Done(result) => Some(result),
                LoaderMsg::Ui(..) => None,
            });
        assert!(matches!(result, Some(Err(_))));
        println!("{SUCCESS}");
    }

    #[test]
    fn the_world_renderer_waits_for_the_first_title_frame() {
        let (_send, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        // No menu bundle yet: nothing to wait for.
        assert!(loading.ready_for_handoff());
        // The title artwork is up but no frame of it was presented.
        loading.ui_at = Some(Instant::now());
        assert!(!loading.ready_for_handoff());
        loading.title_drawn = true;
        assert!(loading.ready_for_handoff());
        // A window never asked to redraw doesn't hold the start for long.
        loading.title_drawn = false;
        loading.ui_at = Instant::now().checked_sub(Duration::from_secs(2));
        assert!(!loading.ready_for_handoff());
        assert!(loading.ready_for_handoff_with_window(true));
    }

    #[test]
    fn artwork_switches_from_boot_to_the_received_title_resources() {
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        let (atlas, panorama) = loading.artwork();
        assert!(std::ptr::eq(atlas, &loading.boot));
        assert!(panorama.is_none());

        let root = std::env::temp_dir().join("horizon-startup-menu-artwork-test");
        let mut resources = gui(&root);
        resources.atlas.revision = 42;
        let expected_panorama = resources.panorama.clone();
        loading.set_ui(resources, root);

        let (atlas, panorama) = loading.artwork();
        assert_eq!(atlas.revision, 42);
        assert!(Arc::ptr_eq(panorama.unwrap(), &expected_panorama));
    }

    #[test]
    fn platform_presenter_receives_the_original_title_frame_before_handoff() {
        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }
        struct Presenter {
            accepts: bool,
            vertices: usize,
            title: bool,
            size: [f32; 2],
        }
        impl horizon_client::presentation::PresentationBackend for Presenter {
            fn viewport_size(&self) -> [u32; 2] {
                [639, 479]
            }
            fn present_frame(&mut self, frame: &Frame<'_>) -> Result<bool> {
                self.vertices = frame.hud.vertices.len();
                self.title = frame.title;
                self.size = [frame.hud.width, frame.hud.height];
                Ok(self.accepts)
            }
            fn resize(&mut self, _: u32, _: u32) {}
            fn release_surface(&mut self) {}
            fn recreate_surface(&mut self) -> Result<()> {
                Ok(())
            }
            fn set_present_mode(&mut self, _: bool, _: horizon_client::graphics::PresentMode) {}
            fn set_gamma(&mut self, _: f32) {}
        }

        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        let root = std::env::temp_dir().join("horizon-startup-menu-presenter-test");
        loading.set_ui(gui(&root), root);
        loading.loaded = Some(Err("loader failure".into()));
        loading.ui_at = Instant::now().checked_sub(Duration::from_secs(2));
        let mut presenter = Presenter {
            accepts: false,
            vertices: 0,
            title: false,
            size: [0.0; 2],
        };
        loading.draw_on(&mut presenter);
        assert!(presenter.title);
        assert!(presenter.vertices > 0);
        assert_eq!(presenter.size, [639.0, 479.0]);
        assert!(!loading.title_drawn);
        assert!(loading.take_ready().is_none());

        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));
        loading.dispatch_platform_window_event(
            &control,
            WindowEvent::Resized(horizon_client_platform::PhysicalSize::new(600, 400)),
        );
        assert_eq!(loading.menu_size(), [600, 400]);
        loading.dispatch_platform_window_event(
            &control,
            WindowEvent::CursorMoved {
                position: horizon_client_platform::PhysicalPosition::new(120.0, 90.0),
            },
        );
        assert_eq!(loading.menu.cursor, (120.0, 90.0));

        presenter.accepts = true;
        loading.draw_on(&mut presenter);
        assert!(loading.title_drawn);
        assert!(loading.ready_for_handoff());
        assert!(matches!(loading.take_ready(), Some(Err(ref e)) if e == "loader failure"));
    }

    #[test]
    fn cafe_pad_routes_navigation_and_activation_through_the_title_menu() {
        use horizon_client::gamepad::{Button, GlyphStyle, PadState};
        use horizon_client::title::Mode;

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join("horizon-cafe-pad-menu-test");
        session.loading.set_ui(gui(&root), root);
        // This regression is specifically for the main-title Options action;
        // don't let a user's onboarding preference choose another title mode.
        session.loading.menu.title.as_mut().unwrap().mode = Mode::Main;
        let title_screen = session.loading.menu.title.as_ref().unwrap().screen_key();
        horizon_client::widgets::set_action_screen(title_screen);
        draw_menu_once(&mut session.loading);
        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));

        let targets = session.loading.menu_key_targets();
        let rects: Vec<_> = targets.iter().map(|target| target.rect).collect();
        let (from, direction, to) = targets
            .iter()
            .find_map(|from| {
                [
                    Button::DPadUp,
                    Button::DPadDown,
                    Button::DPadLeft,
                    Button::DPadRight,
                ]
                .into_iter()
                .find_map(|direction| {
                    horizon_client::gamepad::focus_step_rect(from.rect, direction, &rects)
                        .map(|next| (*from, direction, targets[next]))
                })
            })
            .expect("title menu has at least two navigable targets");
        session.loading.menu.key_focus = Some(from);
        session.loading.menu.key_screen = session.loading.menu.key_screen();
        let mut state = PadState::default();
        state.set(direction, true);
        session.dispatch_pad_state(&control, state);
        assert_eq!(session.loading.menu.key_focus, Some(to));

        let options_rect = horizon_client::widgets::last_actions(title_screen)
            .into_iter()
            .find(|action| {
                action.action == horizon_client::widgets::Action::Options && action.enabled
            })
            .expect("main title declares its Options action")
            .rect;
        let options_target = session
            .loading
            .menu_key_targets()
            .into_iter()
            .find(|target| {
                target
                    .rect
                    .iter()
                    .zip(options_rect)
                    .all(|(a, b)| (a - b).abs() < 0.5)
            })
            .unwrap_or_else(|| {
                panic!(
                    "main-menu Options rect {options_rect:?} is missing from focus targets {:?}",
                    session.loading.menu_key_targets(),
                )
            });
        session.loading.menu.key_focus = Some(options_target);
        session.loading.menu.key_screen = session.loading.menu.key_screen();
        session.loading.settings.controller.glyphs = GlyphStyle::Nintendo;
        session.loading.settings.controller.nintendo_letters = true;
        let mut state = PadState::default();
        state.set(Button::East, true); // The printed Nintendo A maps to South.
        session.dispatch_pad_state(&control, state);

        assert!(session.loading.menu.options_open);
        draw_menu_once(&mut session.loading);
        session.loading.menu.key_focus = None;
        session.dispatch_pad_state(&control, state);
        assert!(session.loading.menu.key_focus.is_none());
        session.dispatch_pad_state(&control, PadState::default());
        session.dispatch_pad_state(&control, state);
        assert!(session.loading.menu.key_focus.is_some());
        assert!(session.loading.menu.physical_keys.is_empty());
        assert!(!session.loading.menu.key_synthetic);
    }

    #[test]
    fn cafe_pad_b_and_start_share_escape_and_cancel_pending_entries() {
        use horizon_client::gamepad::{Button, GlyphStyle, PadState};
        use horizon_client::multiplayer::Screen as MultiplayerScreen;
        use horizon_client::title::Mode;

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join(format!(
            "horizon-cafe-pad-back-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        session.loading.set_ui(gui(&root), root.clone());
        session.loading.settings.controller.enabled = true;
        session.loading.settings.controller.glyphs = GlyphStyle::Nintendo;
        session.loading.settings.controller.nintendo_letters = true;
        session.loading.menu.title.as_mut().unwrap().mode =
            Mode::Multiplayer(MultiplayerScreen::Direct {
                address: "localhost".into(),
            });
        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));

        // With Nintendo labels, the physical bottom B button normalizes to East.
        // Pressing B and Start together dispatches only one Escape: Direct
        // Connection backs to the multiplayer list but remains in Multiplayer.
        let mut back_and_start = PadState::default();
        back_and_start.set(Button::South, true);
        back_and_start.set(Button::Start, true);
        session.dispatch_pad_state(&control, back_and_start);
        assert!(matches!(
            session.loading.menu.title.as_ref().unwrap().mode,
            Mode::Multiplayer(MultiplayerScreen::List { .. })
        ));
        session.dispatch_pad_state(&control, back_and_start);
        assert!(matches!(
            session.loading.menu.title.as_ref().unwrap().mode,
            Mode::Multiplayer(MultiplayerScreen::List { .. })
        ));

        // Start on its own exits the list, using the same Escape path.
        session.dispatch_pad_state(&control, PadState::default());
        let mut start = PadState::default();
        start.set(Button::Start, true);
        session.dispatch_pad_state(&control, start);
        assert!(matches!(
            session.loading.menu.title.as_ref().unwrap().mode,
            Mode::Main
        ));

        // B also cancels a pending server entry through Escape's existing path.
        session.dispatch_pad_state(&control, PadState::default());
        session.loading.menu.pending = Some(PendingWorldEntry::Server {
            address: "localhost".into(),
            name: "Player".into(),
            cookies: None,
            lan: false,
        });
        let mut b = PadState::default();
        b.set(Button::South, true);
        session.dispatch_pad_state(&control, b);
        assert!(session.loading.menu.pending.is_none());
        assert!(session.loading.menu.physical_keys.is_empty());

        drop(session);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cafe_pad_confirms_a_focused_world_row_through_the_title_click_path() {
        use horizon_client::gamepad::{Button, PadState};
        use horizon_client::title::{Mode, WorldEntry};

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join(format!(
            "horizon-cafe-pad-world-row-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        session.loading.set_ui(gui(&root), root.clone());
        let mut title = Title::new(root.join("saves"), &root);
        title.mode = Mode::Select {
            worlds: vec![WorldEntry {
                folder: "world".into(),
                name: "World".into(),
                game_mode: 0,
                hardcore: false,
                modified: std::time::SystemTime::UNIX_EPOCH,
                commands: false,
                version: "26.3".into(),
                locked: false,
                corrupted: false,
                incompatible: None,
                file_fixing: false,
                downgrade: false,
            }],
            selected: Some(0),
        };
        session.loading.menu.title = Some(title);
        draw_menu_once(&mut session.loading);
        let row = session
            .loading
            .menu_key_targets()
            .into_iter()
            .find(|target| target.kind == Kind::Row)
            .expect("the selected world is registered as a row target");
        session.loading.menu.key_focus = Some(row);
        session.loading.menu.key_screen = session.loading.menu.key_screen();

        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));
        let mut state = PadState::default();
        state.set(Button::South, true);
        session.dispatch_pad_state(&control, state);

        assert!(matches!(
            session.loading.menu.title.as_ref().unwrap().mode,
            Mode::Recover { .. }
        ));
        assert!(session.loading.menu.physical_keys.is_empty());
        drop(session);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cafe_pad_respects_disabled_setting_and_requires_release_after_reenable() {
        use horizon_client::gamepad::{Button, PadState};

        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let root = std::env::temp_dir().join("horizon-cafe-pad-disabled-test");
        session.loading.set_ui(gui(&root), root);
        draw_menu_once(&mut session.loading);
        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));

        session.loading.settings.controller.enabled = false;
        let mut held = PadState::default();
        held.set(Button::South, true);
        session.dispatch_pad_state(&control, held);
        assert!(session.loading.menu.key_focus.is_none());
        assert!(!session.loading.menu.options_open);

        session.loading.settings.controller.enabled = true;
        session.dispatch_pad_state(&control, held);
        assert!(session.loading.menu.key_focus.is_none());
        session.dispatch_pad_state(&control, PadState::default());
        session.dispatch_pad_state(&control, held);
        assert!(session.loading.menu.key_focus.is_some());
        assert!(!session.loading.menu.options_open);
        assert!(session.loading.menu.physical_keys.is_empty());
    }

    #[test]
    fn cafe_session_lends_boot_then_loaded_artwork_and_marks_only_accepted_title() {
        #[derive(Clone, Copy)]
        enum Outcome {
            Accept(bool),
        }
        struct Presenter {
            outcome: Outcome,
            calls: usize,
            prepared_revisions: Vec<u64>,
            callback_order: Vec<&'static str>,
            atlas_revision: u64,
            panorama: Option<usize>,
            title: bool,
            layout: [u32; 2],
            targets: [[u32; 4]; 2],
        }
        impl CafeLoadingPresenter for Presenter {
            fn layout_viewport(&self) -> [u32; 2] {
                if self.prepared_revisions.is_empty() {
                    [0, 0]
                } else {
                    [854, 480]
                }
            }
            fn target_viewports(&self) -> [[u32; 4]; 2] {
                if self.prepared_revisions.is_empty() {
                    [[0; 4]; 2]
                } else {
                    [[0, 0, 1920, 1080], [0, 0, 854, 480]]
                }
            }
            fn prepare_loading(&mut self, artwork: CafeLoadingArtwork<'_>) -> Result<()> {
                self.prepared_revisions.push(artwork.generation);
                self.callback_order.push("prepare");
                Ok(())
            }
            fn present_loading(
                &mut self,
                artwork: CafeLoadingArtwork<'_>,
                frame: &Frame<'_>,
                layout_viewport: [u32; 2],
                target_viewports: [[u32; 4]; 2],
                _: u64,
                _: bool,
            ) -> Result<bool> {
                assert_eq!(self.callback_order.last(), Some(&"prepare"));
                self.callback_order.push("present");
                self.calls += 1;
                self.atlas_revision = artwork.generation;
                self.panorama = artwork.panorama.map(|faces| faces as *const _ as usize);
                self.title = frame.title;
                self.layout = layout_viewport;
                self.targets = target_viewports;
                match self.outcome {
                    Outcome::Accept(accepted) => Ok(accepted),
                }
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let boot_revision = session.loading.boot.revision;
        let mut presenter = Presenter {
            outcome: Outcome::Accept(false),
            calls: 0,
            prepared_revisions: Vec::new(),
            callback_order: Vec::new(),
            atlas_revision: 0,
            panorama: None,
            title: false,
            layout: [0; 2],
            targets: [[0; 4]; 2],
        };

        assert!(!session.draw(&mut presenter).unwrap());
        assert_eq!(presenter.calls, 1);
        assert_eq!(presenter.atlas_revision, boot_revision);
        assert_eq!(presenter.prepared_revisions, [boot_revision]);
        assert_eq!(presenter.callback_order, ["prepare", "present"]);
        assert!(presenter.panorama.is_none());
        assert!(presenter.title);
        assert_eq!(presenter.layout, [854, 480]);
        assert_eq!(presenter.targets, [[0, 0, 1920, 1080], [0, 0, 854, 480]]);
        assert!(!session.loading.title_drawn);

        let root = std::env::temp_dir().join("horizon-cafe-session-artwork-test");
        let resources = gui(&root);
        let loaded_revision = resources.atlas.revision;
        let expected_panorama = Arc::as_ptr(&resources.panorama) as usize;
        session.loading.set_ui(resources, root);
        presenter.outcome = Outcome::Accept(true);
        assert!(session.draw(&mut presenter).unwrap());
        assert_eq!(presenter.calls, 2);
        assert_eq!(presenter.atlas_revision, loaded_revision);
        assert_eq!(
            presenter.prepared_revisions,
            [boot_revision, loaded_revision]
        );
        assert_eq!(
            presenter.callback_order,
            ["prepare", "present", "prepare", "present"]
        );
        assert_eq!(presenter.panorama, Some(expected_panorama));
        assert!(session.loading.title_drawn);
        assert_eq!(session.status(), CafeLoadingStatus::Loading);
        assert!(session.loading.loaded.is_none());
    }

    #[test]
    fn cafe_presenter_error_keeps_loader_failure_and_pending_entry() {
        struct Presenter;
        impl CafeLoadingPresenter for Presenter {
            fn layout_viewport(&self) -> [u32; 2] {
                [854, 480]
            }
            fn target_viewports(&self) -> [[u32; 4]; 2] {
                [[0, 0, 1920, 1080], [0, 0, 854, 480]]
            }
            fn prepare_loading(&mut self, _: CafeLoadingArtwork<'_>) -> Result<()> {
                Ok(())
            }
            fn present_loading(
                &mut self,
                _: CafeLoadingArtwork<'_>,
                _: &Frame<'_>,
                _: [u32; 2],
                _: [[u32; 4]; 2],
                _: u64,
                _: bool,
            ) -> Result<bool> {
                Err("presenter failure".into())
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        session.loading.loaded = Some(Err("loader failure".into()));
        session.loading.menu.pending = Some(PendingWorldEntry::Server {
            address: "example.invalid".into(),
            name: "test".into(),
            lan: false,
            cookies: None,
        });
        let mut presenter = Presenter;

        let error = session.draw(&mut presenter).unwrap_err();
        assert_eq!(error.to_string(), "presenter failure");
        assert_eq!(session.status(), CafeLoadingStatus::Failed);
        assert_eq!(session.failure_message().as_deref(), Some("loader failure"));
        assert!(matches!(
            session.loading.loaded.as_ref(),
            Some(Err(error)) if error == "loader failure"
        ));
        assert!(session.loading.menu.pending.is_some());
        assert!(!session.loading.title_drawn);
        assert!(!session.loading.shown);
    }

    #[test]
    fn cafe_prepare_failure_skips_layout_presentation_and_shown_state() {
        struct Presenter {
            prepared_revision: Option<u64>,
        }
        impl CafeLoadingPresenter for Presenter {
            fn layout_viewport(&self) -> [u32; 2] {
                [854, 480]
            }
            fn target_viewports(&self) -> [[u32; 4]; 2] {
                [[0, 0, 1920, 1080], [0, 0, 854, 480]]
            }
            fn prepare_loading(&mut self, artwork: CafeLoadingArtwork<'_>) -> Result<()> {
                self.prepared_revision = Some(artwork.generation);
                Err("resource preparation failed".into())
            }
            fn present_loading(
                &mut self,
                _: CafeLoadingArtwork<'_>,
                _: &Frame<'_>,
                _: [u32; 2],
                _: [[u32; 4]; 2],
                _: u64,
                _: bool,
            ) -> Result<bool> {
                panic!("presentation must not run after preparation fails")
            }
        }

        let _gui_scale = GuiScaleRestore::new();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut session = CafeLoadingSession {
            loading: loading(receiver),
        };
        let initial_viewport = session.loading.viewport;
        session.loading.loaded = Some(Err("retained loader failure".into()));
        session.loading.menu.pending = Some(PendingWorldEntry::Server {
            address: "example.invalid".into(),
            name: "test".into(),
            lan: false,
            cookies: None,
        });
        let boot_revision = session.loading.boot.revision;
        let mut presenter = Presenter {
            prepared_revision: None,
        };

        let error = session.draw(&mut presenter).unwrap_err();
        assert_eq!(error.to_string(), "resource preparation failed");
        assert_eq!(presenter.prepared_revision, Some(boot_revision));
        assert_eq!(session.loading.viewport, initial_viewport);
        assert_eq!(session.status(), CafeLoadingStatus::Failed);
        assert_eq!(
            session.failure_message().as_deref(),
            Some("retained loader failure")
        );
        assert!(session.loading.menu.pending.is_some());
        assert!(!session.loading.title_drawn);
        assert!(!session.loading.shown);
    }

    #[test]
    fn loader_handoff_waits_for_the_title_frame() {
        let (_sender, receiver) = std::sync::mpsc::channel();
        let mut loading = loading(receiver);
        loading.ui_at = Some(Instant::now());
        loading.loaded = Some(Err("loader failure".into()));
        assert!(loading.take_ready().is_none());
        loading.title_drawn = true;
        assert!(matches!(loading.take_ready(), Some(Err(ref e)) if e == "loader failure"));
    }

    #[test]
    fn menu_is_usable_while_the_core_worker_is_blocked() {
        struct LoopControl(std::cell::Cell<EventLoopControl>);
        impl EventLoopCtl for LoopControl {
            fn set_control_flow(&self, control: EventLoopControl) {
                self.0.set(control);
            }
        }
        let root = std::env::temp_dir().join("horizon-startup-menu-no-world-resources");
        let (sender, receiver) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (published, published_rx) = std::sync::mpsc::channel();
        let worker_root = root.clone();
        let worker = std::thread::spawn(move || {
            sender
                .send(LoaderMsg::Ui(gui(&worker_root), worker_root))
                .unwrap();
            published.send(()).unwrap();
            wait.recv().unwrap();
            sender
                .send(LoaderMsg::Done(Err("core failure".into())))
                .unwrap();
        });
        published_rx.recv().unwrap();
        let mut loading = loading(receiver);
        loading.poll_menu_resources().unwrap();
        assert!(loading.gui.is_some());
        assert!(loading.loaded.is_none());
        assert!(!worker.is_finished());
        // Navigate and enter a world name without a registry, biomes, pack
        // tables or a Game. Replacing fallback artwork preserves this state.
        let title = loading.menu.title.as_mut().unwrap();
        title.mode = horizon_client::title::Mode::Create {
            name: String::new(),
            seed: "42".into(),
            creative: true,
            focus: 0,
            options: Default::default(),
        };
        let control = LoopControl(std::cell::Cell::new(EventLoopControl::Poll));
        loading.dispatch_platform_window_event(
            &control,
            WindowEvent::KeyboardInput {
                event: horizon_client_platform::KeyboardEvent {
                    physical_key: None,
                    logical_key: horizon_client_platform::LogicalKey::Character("My world".into()),
                    location: horizon_client_platform::KeyLocation::Standard,
                    state: horizon_client_platform::KeyState::Pressed,
                    logical_text: Some("My world".into()),
                    text: Some("My world".into()),
                    unmodified_text: Some("My world".into()),
                    repeat: false,
                },
                is_synthetic: false,
            },
        );
        assert_eq!(control.0.get(), EventLoopControl::Poll);
        loading.set_ui(gui(&root), root);
        let action = loading.menu.title.as_ref().unwrap().create_action();
        loading.menu.queue_entry(action, "Player".into());
        assert!(
            matches!(&loading.menu.pending, Some(PendingWorldEntry::Local {
            name: Some(name), seed: Some(42), creative: true, create: Some(_), ..
        }) if name == "My world")
        );
        let mut batch = HudBatch::new([1280, 720]);
        loading.draw_menu(&mut batch);
        assert!(
            !batch.vertices.is_empty(),
            "queued entry draws loading progress"
        );
        release.send(()).unwrap();
        worker.join().unwrap();
        loading.poll_menu_resources().unwrap();
        assert!(matches!(loading.loaded, Some(Err(ref error)) if error == "core failure"));
    }

    #[test]
    fn queued_server_keeps_address_player_identity_and_lan_kind() {
        let mut menu = StartupMenu::default();
        menu.queue_entry(
            Action::Connect {
                address: "localhost:25565".into(),
                lan: true,
            },
            "Guest".into(),
        );
        assert!(
            matches!(menu.pending.take(), Some(PendingWorldEntry::Server { address, name, lan: true, .. })
            if address == "localhost:25565" && name == "Guest")
        );
        assert!(menu.pending.take().is_none(), "an entry is consumed once");
    }

    #[test]
    fn a_failed_generation_keeps_its_error_after_the_loader_ends() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender
                .send(LoaderMsg::Done(Err("missing facts.json".into())))
                .unwrap();
        });
        let mut loading = loading(receiver);
        loading.handle = Some(worker);
        while loading.loaded.is_none() {
            loading.poll_menu_resources().unwrap();
        }
        // App::about_to_wait takes the result to report it; the loop may poll
        // once more before it exits, with the loader gone.
        assert!(matches!(loading.loaded.take(), Some(Err(e)) if e == "missing facts.json"));
        while !loading.handle.as_ref().is_none_or(JoinHandle::is_finished) {
            std::thread::yield_now();
        }
        loading.poll_menu_resources().unwrap();
        assert!(loading.loaded.is_none());
    }

    #[test]
    fn a_panicking_loader_reports_its_panic() {
        let (sender, receiver) = std::sync::mpsc::channel::<LoaderMsg>();
        let worker = std::thread::spawn(move || {
            let _sender = sender;
            panic!("atlas has no fonts");
        });
        while !worker.is_finished() {
            std::thread::yield_now();
        }
        let mut loading = loading(receiver);
        loading.handle = Some(worker);
        assert_eq!(
            loading.poll_menu_resources().unwrap_err().to_string(),
            "resource loader stopped before finishing: atlas has no fonts"
        );
    }

    #[test]
    fn disconnected_core_worker_is_reported_instead_of_waiting_forever() {
        let (sender, receiver) = std::sync::mpsc::channel();
        drop(sender);
        let mut loading = loading(receiver);
        assert_eq!(
            loading.poll_menu_resources().unwrap_err().to_string(),
            "resource loader stopped before finishing: no loader thread was running"
        );
    }
}
