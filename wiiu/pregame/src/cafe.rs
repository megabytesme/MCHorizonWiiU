use core::ffi::{c_char, c_void, CStr};
use core::ptr::NonNull;

use crate::canvas::{Canvas, Display, Point, Vertex, CAPACITY};
#[cfg(not(horizon_wiiu_hosted))]
use crate::config::Instance;
use crate::config::Settings;
#[cfg(horizon_wiiu_hosted)]
use crate::hosted::CafeFactory;
use crate::launcher::{Action, Launcher, Page, Touch, View};
#[cfg(horizon_wiiu_hosted)]
use crate::local_wire;
#[cfg(not(horizon_wiiu_hosted))]
use crate::mock::MockFactory;
use crate::session::SessionStatus;

#[path = "../themes/night/theme.rs"]
mod theme;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Sample {
    buttons: u32,
    touch: u32,
    x: u32,
    y: u32,
}

extern "C" {
    fn shell_process() -> i32;
    fn shell_release();
    #[cfg(horizon_wiiu_hosted)]
    fn shell_destroy_view();
    #[cfg(horizon_wiiu_hosted)]
    fn horizon_swkbd_request(initial: *const u16, max: i32) -> i32;
    #[cfg(horizon_wiiu_hosted)]
    fn horizon_swkbd_take_result(text: *mut u8, capacity: usize, length: *mut usize) -> i32;
    #[cfg(horizon_wiiu_hosted)]
    fn horizon_swkbd_close();
    fn shell_poll_input(samples: *mut Sample) -> i32;
    fn shell_create_view(atlas: *const u8, width: u32, height: u32, capacity: u32) -> bool;
    fn shell_present(
        tv: *const Vertex,
        tv_count: u32,
        tv_color: u32,
        drc: *const Vertex,
        drc_count: u32,
        drc_color: u32,
    ) -> bool;
    fn shell_trace(message: *const c_char);
    fn shell_measure(tv_vertices: u32, drc_vertices: u32, atlas_bytes: u32);
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

pub(crate) fn trace(message: &'static CStr) {
    unsafe { shell_trace(message.as_ptr()) }
}

struct FrameStorage(NonNull<Vertex>);

impl Drop for FrameStorage {
    fn drop(&mut self) {
        unsafe { free(self.0.as_ptr().cast()) }
    }
}

fn marker(view: View) -> &'static CStr {
    match view.page {
        Page::Home => match view.focused {
            Some(Action::Profiles) => c"frame.home.profiles",
            Some(Action::Library) => c"frame.home.library",
            Some(Action::Versions) => c"frame.home.versions",
            Some(Action::Settings) => c"frame.home.settings",
            Some(Action::AgentControl) => c"frame.home.agent",
            Some(Action::Quit) => c"frame.home.close",
            _ => c"frame.home.play",
        },
        Page::Profiles if matches!(view.profile, crate::launcher::OfflineProfile::Guest) => {
            c"frame.profiles.guest_selected"
        }
        Page::Profiles => c"frame.profiles",
        Page::Versions => c"frame.versions",
        Page::Settings if view.settings.swap_buttons => c"frame.settings.swapped",
        Page::Settings => c"frame.settings",
        Page::AgentControl => c"frame.agent",
        Page::Local => match view.title() {
            "Offline profiles" => c"frame.local.profiles",
            "Offline profile" => c"frame.local.profile",
            "Settings" if view.settings.swap_buttons => c"frame.local.settings.swapped",
            "Settings" => c"frame.local.settings",
            "Worlds & servers" => c"frame.local.library",
            "Local worlds" => c"frame.local.worlds",
            "Saved servers" => c"frame.local.servers",
            "Version & resources" => c"frame.local.versions",
            "Installed resources" => c"frame.local.instances",
            "About Horizon Wii U" => c"frame.local.about",
            "Diagnostic reports" => c"frame.local.reports",
            "Benchmark report" => c"frame.local.report",
            _ => c"frame.local.form",
        },
        Page::Loading if matches!(view.session, Some(SessionStatus::Stopping(_))) => {
            c"frame.loading.stopping"
        }
        Page::Loading => c"frame.loading",
        Page::Session => match view.session {
            Some(SessionStatus::Saving) => c"frame.session.saving",
            Some(SessionStatus::SaveFailed(_)) => c"frame.session.save_failed",
            Some(SessionStatus::Stopping(_)) => c"frame.session.stopping",
            _ => c"frame.session",
        },
        Page::Error => c"frame.startup_error",
        Page::Exiting => c"frame.exiting",
    }
}

fn open_view() -> Option<FrameStorage> {
    let atlas = theme::ATLAS;
    if atlas.pixels.len()
        != (atlas.width as usize)
            .saturating_mul(atlas.height as usize)
            .saturating_mul(4)
        || !unsafe {
            shell_create_view(
                atlas.pixels.as_ptr(),
                atlas.width,
                atlas.height,
                CAPACITY as u32,
            )
        }
    {
        trace(c"view.init_failed");
        return None;
    }
    let Some(data) =
        NonNull::new(unsafe { calloc(CAPACITY * 2, core::mem::size_of::<Vertex>()) }.cast())
    else {
        trace(c"view.allocation_failed");
        return None;
    };
    Some(FrameStorage(data))
}

#[no_mangle]
extern "C" fn horizon_pregame_run() -> i32 {
    let atlas = theme::ATLAS;
    let Some(storage) = open_view() else {
        return 1;
    };
    #[cfg(horizon_wiiu_hosted)]
    let mut storage = Some(storage);
    #[cfg(not(horizon_wiiu_hosted))]
    let mut launcher = {
        let fixture = cfg!(horizon_wiiu_pregame_fixture).then_some(Instance::DEMO);
        Launcher::new(MockFactory, fixture, Settings::default())
    };
    #[cfg(horizon_wiiu_hosted)]
    let mut launcher = Launcher::new(
        CafeFactory,
        Some(crate::hosted::hosted_instance()),
        Settings::default(),
    );
    #[cfg(horizon_wiiu_hosted)]
    launcher.install_local_menu(crate::hosted::local_menu(0, 0, ""));
    #[cfg(horizon_wiiu_hosted)]
    let mut keyboard = false;
    let mut displayed = None;
    let mut frame = 0_u32;
    #[cfg(horizon_wiiu_hosted)]
    let mut exit_code = 0;
    #[cfg(not(horizon_wiiu_hosted))]
    let exit_code = 0;
    loop {
        match unsafe { shell_process() } {
            1 => launcher.focus(true),
            2 => break,
            3 => {
                #[cfg(horizon_wiiu_hosted)]
                if core::mem::take(&mut keyboard) {
                    unsafe { horizon_swkbd_close() };
                }
                launcher.focus(false);
                displayed = None;
                unsafe { shell_release() };
                continue;
            }
            _ => {
                launcher.focus(false);
                displayed = None;
                continue;
            }
        }

        #[cfg(horizon_wiiu_hosted)]
        if let Some(driver) = launcher.live_driver() {
            unsafe { shell_destroy_view() };
            drop(storage.take());
            trace(c"game.borrowed_foreground");
            if driver.run() {
                exit_code = 3;
            }
            launcher.advance();
            launcher.install_local_menu(crate::hosted::local_menu(0, 0, ""));
            launcher.input_lost();
            displayed = None;
            continue;
        }
        #[cfg(horizon_wiiu_hosted)]
        let Some(storage) = storage.as_ref() else {
            let Some(restored) = open_view() else {
                return finish(&mut launcher, 2);
            };
            storage = Some(restored);
            trace(c"game.returned_foreground");
            continue;
        };
        let mut samples = [Sample::default(); 16];
        let count = unsafe { shell_poll_input(samples.as_mut_ptr()) };
        if count < 0 {
            launcher.input_lost();
        } else {
            for sample in samples.into_iter().take(count as usize) {
                #[cfg(horizon_wiiu_hosted)]
                if keyboard {
                    break;
                }
                let touch = match sample.touch {
                    0 => Touch::Released,
                    1 => Touch::Pressed(theme::hit_test(
                        launcher.view(),
                        Point {
                            x: sample.x as f32,
                            y: sample.y as f32,
                        },
                    )),
                    _ => Touch::Invalid,
                };
                launcher.sample(sample.buttons, touch);
                #[cfg(horizon_wiiu_hosted)]
                if let Some((command, index)) = launcher.take_local_intent() {
                    let menu = crate::hosted::local_menu(command, index, "");
                    let input_limit = menu.input_limit;
                    launcher.install_local_menu(menu);
                    if input_limit != 0 {
                        let mut initial = [0_u16; 254];
                        for (slot, unit) in
                            initial.iter_mut().zip(menu.input.as_str().encode_utf16())
                        {
                            *slot = unit;
                        }
                        keyboard = unsafe {
                            horizon_swkbd_request(initial.as_ptr(), i32::from(input_limit))
                        } == 0;
                        launcher.input_lost();
                        if !keyboard {
                            unsafe { horizon_swkbd_close() };
                            launcher.install_local_menu(crate::hosted::local_menu(
                                local_wire::TEXT_UNAVAILABLE,
                                0,
                                "",
                            ));
                        }
                    }
                }
            }
        }

        #[cfg(horizon_wiiu_hosted)]
        if keyboard {
            let mut text = [0_u8; 1024];
            let mut len = 0;
            let result =
                unsafe { horizon_swkbd_take_result(text.as_mut_ptr(), text.len(), &mut len) };
            if result != 0 {
                keyboard = false;
                unsafe { horizon_swkbd_close() };
                launcher.input_lost();
                let input = text
                    .get(..len)
                    .and_then(|bytes| core::str::from_utf8(bytes).ok());
                let action = match (result, input) {
                    (1, Some(_)) => local_wire::TEXT_ACCEPTED,
                    (2, _) => local_wire::TEXT_CANCELLED,
                    _ => local_wire::TEXT_UNAVAILABLE,
                };
                launcher.install_local_menu(crate::hosted::local_menu(
                    action,
                    0,
                    input.unwrap_or(""),
                ));
            }
        }

        let view = launcher.view();
        if view.page == Page::Exiting {
            break;
        }
        let pixels = storage.0.as_ptr();
        let mut tv = Canvas::new(
            unsafe { core::slice::from_raw_parts_mut(pixels, CAPACITY) },
            &atlas,
        );
        let mut drc = Canvas::new(
            unsafe { core::slice::from_raw_parts_mut(pixels.add(CAPACITY), CAPACITY) },
            &atlas,
        );
        // The fixture uses the launcher renderer for its live page. A real
        // host instead borrows live_driver() and gives it this foreground frame.
        theme::paint(&mut tv, view, Display::Tv, frame);
        theme::paint(&mut drc, view, Display::GamePad, frame);

        let (Some((tv_ptr, tv_count, tv_color)), Some((drc_ptr, drc_count, drc_color))) =
            (tv.output(), drc.output())
        else {
            trace(c"view.batch_overflow");
            return finish(&mut launcher, 2);
        };
        if !unsafe { shell_present(tv_ptr, tv_count, tv_color, drc_ptr, drc_count, drc_color) } {
            trace(c"view.draw_failed");
            return finish(&mut launcher, 2);
        }
        let current = marker(view);
        if displayed != Some(current) {
            trace(current);
            unsafe { shell_measure(tv_count, drc_count, atlas.pixels.len() as u32) };
            displayed = Some(current);
        }
        frame = frame.wrapping_add(1);
        launcher.advance();
    }
    finish(&mut launcher, exit_code)
}

fn finish(launcher: &mut Launcher<'_, impl crate::session::SessionFactory>, code: i32) -> i32 {
    #[cfg(horizon_wiiu_hosted)]
    unsafe {
        horizon_swkbd_close()
    };
    let result = launcher.close();
    if result.is_err() {
        trace(c"session.exit_failed");
    }
    trace(c"launcher.closed");
    if result.is_err() {
        3
    } else {
        code
    }
}
