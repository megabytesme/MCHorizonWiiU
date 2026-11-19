//! Non-secret launch configuration passed to the session factory by value.

use crate::accounts::Profile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    pub swap_buttons: bool,
    pub touch_navigation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            swap_buttons: false,
            touch_navigation: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldOrServer<'a> {
    Demo,
    LocalWorld { id: &'a str },
    Server { host: &'a str, port: u16 },
}

/// The host resolves these paths and checks resource compatibility before
/// allowing the real engine to use them. This descriptor carries no secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedResources<'a> {
    pub directory: &'a str,
    pub game_version: &'a str,
    pub manifest_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resources<'a> {
    BuiltInDemo,
    Prepared(PreparedResources<'a>),
}

/// A preselected instance. Future profile/world selectors replace this value
/// at the launcher boundary with a resolved instance, never during a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Instance<'a> {
    pub game_directory: &'a str,
    pub target: WorldOrServer<'a>,
    pub resources: Resources<'a>,
}

impl Instance<'static> {
    pub const DEMO: Self = Self {
        game_directory: "",
        target: WorldOrServer::Demo,
        resources: Resources::BuiltInDemo,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchConfig<'a> {
    /// Microsoft profiles carry an account ID. A provider supplies the game
    /// identity directly to the engine in memory, independent of this value.
    pub profile: Profile<'a>,
    pub instance: Instance<'a>,
    pub settings: Settings,
}
