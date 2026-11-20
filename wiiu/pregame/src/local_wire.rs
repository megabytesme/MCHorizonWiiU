//! Fixed-size exchange between Night's renderer and its hosted local services.

pub const ROWS: usize = 6;
pub const ACTIVE: u8 = 0;
pub const SELECTED: u8 = 1;
pub const DISABLED: u8 = 2;
pub const EDITOR: u8 = 1;
pub const PRIVATE_INPUT: u8 = 2;
pub const NO_UPDATE: u8 = 4;
pub const SWAP_AB: u8 = 1;
pub const SWAP_XY: u8 = 2;
pub const TOUCH: u8 = 4;

pub const OPEN_PROFILES: u8 = 1;
pub const OPEN_LIBRARY: u8 = 2;
pub const OPEN_VERSIONS: u8 = 3;
pub const OPEN_SETTINGS: u8 = 4;
pub const BACK: u8 = 5;
pub const LEFT: u8 = 6;
pub const RIGHT: u8 = 7;
pub const ACTIVATE: u8 = 8;
pub const TEXT_ACCEPTED: u8 = 9;
pub const TEXT_CANCELLED: u8 = 10;
pub const TEXT_UNAVAILABLE: u8 = 11;
pub const TICK: u8 = 12;
pub const SHUTDOWN: u8 = 13;
pub const OPEN_AGENT: u8 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Text<const N: usize>([u8; N]);

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> Text<N> {
    pub fn new(value: &str) -> Self {
        let mut bytes = [0; N];
        let mut length = value.len().min(N);
        while !value.is_char_boundary(length) {
            length -= 1;
        }
        bytes[..length].copy_from_slice(&value.as_bytes()[..length]);
        Self(bytes)
    }

    pub fn as_str(&self) -> &str {
        let length = self.0.iter().position(|&byte| byte == 0).unwrap_or(N);
        // These bytes originate in Text::new or the zeroed default, including
        // the values built by the Rust side of the host ABI. Both the capacity
        // cut and a NUL cut fall on UTF-8 boundaries.
        unsafe { core::str::from_utf8_unchecked(self.0.get(..length).unwrap_or(&[])) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct LocalRow {
    pub label: Text<46>,
    pub detail: Text<64>,
    pub state: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct LocalMenu {
    pub title: Text<30>,
    pub description: Text<100>,
    pub profile: Text<28>,
    pub play_detail: Text<52>,
    pub rows: [LocalRow; ROWS],
    pub count: u8,
    pub mode: u8,
    pub ready: u8,
    pub settings: u8,
    pub input: Text<253>,
    pub input_limit: u16,
}

impl core::fmt::Debug for LocalMenu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LocalMenu")
            .field("rows", &self.count)
            .field("mode", &self.mode)
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}
