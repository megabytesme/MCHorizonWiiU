use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv6Addr};
use std::path::{Path, PathBuf};

const SCHEMA: u32 = 2;
const LIMIT: u64 = 64 * 1024;
pub const ENTRIES: usize = 48;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub identity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub name: String,
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub swap_ab: bool,
    pub swap_xy: bool,
    pub touch: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub profiles: Vec<Profile>,
    pub profile: usize,
    pub settings: Settings,
    pub instance: String,
    pub servers: Vec<Server>,
    pub server: Option<usize>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            profiles: vec![Profile {
                name: "Player".into(),
                identity: "Player".into(),
            }],
            profile: 0,
            settings: Settings {
                swap_ab: false,
                swap_xy: false,
                touch: true,
            },
            instance: "default".into(),
            servers: Vec::new(),
            server: None,
        }
    }
}

pub fn label(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value == value.trim()
        && value.chars().all(|c| !c.is_control())
}

pub fn identity(value: &str) -> bool {
    label(value, 16)
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

pub fn identifier(value: &str) -> bool {
    label(value, 96)
        && !value.starts_with('.')
        && !value.ends_with('.')
        && !value
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'))
}

pub fn endpoint(raw: &str) -> Option<String> {
    if !label(raw, 253) || !raw.is_ascii() {
        return None;
    }
    let valid_port = |text: &str| {
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(())?;
        text.parse::<u16>().ok().filter(|port| *port != 0)
    };
    if let Some(rest) = raw.strip_prefix('[') {
        let (address, tail) = rest.split_once(']')?;
        let ip = address.parse::<Ipv6Addr>().ok()?;
        let port = if tail.is_empty() {
            25565
        } else {
            valid_port(tail.strip_prefix(':')?)?
        };
        return Some(format!("[{ip}]:{port}"));
    }
    if let Ok(ip) = raw.parse::<Ipv6Addr>() {
        return Some(format!("[{ip}]:25565"));
    }
    let (host, port) = if let Some((host, port)) = raw.split_once(':') {
        (host, Some(valid_port(port)?))
    } else {
        (raw, None)
    };
    let dns = host.strip_suffix('.').unwrap_or(host);
    let valid_dns = !dns.is_empty()
        && dns.split('.').all(|part| {
            (1..=63).contains(&part.len())
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        });
    let resembles_ip = dns.contains('.') && dns.bytes().all(|c| c.is_ascii_digit() || c == b'.');
    if host.parse::<IpAddr>().is_err() && (!valid_dns || resembles_ip) {
        return None;
    }
    // Keep a host supplied without a port in the original game's usual form.
    let host = host.to_ascii_lowercase();
    Some(match port {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

impl Preferences {
    pub fn validate(&self) -> bool {
        self.profiles.len() <= ENTRIES
            && self.profile < self.profiles.len()
            && self.profiles.iter().enumerate().all(|(i, profile)| {
                identity(&profile.identity)
                    && label(&profile.name, 24)
                    && !self.profiles[..i]
                        .iter()
                        .any(|other| other.identity.eq_ignore_ascii_case(&profile.identity))
            })
            && identifier(&self.instance)
            && self.servers.len() <= ENTRIES
            && self.server.is_none_or(|i| i < self.servers.len())
            && self.servers.iter().all(|server| {
                label(&server.name, 24)
                    && endpoint(&server.address).as_deref() == Some(server.address.as_str())
            })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u32,
    generation: u64,
    preferences: Preferences,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    schema: u32,
    player: String,
    #[serde(default)]
    swap_buttons: bool,
    #[serde(default = "enabled")]
    touch_navigation: bool,
}

fn enabled() -> bool {
    true
}

pub fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too large"));
    }
    Ok(data)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct Store {
    directory: PathBuf,
    active: Option<usize>,
    generation: u64,
    pub notice: &'static str,
    read_only: bool,
}

impl Store {
    pub fn open(directory: PathBuf) -> (Self, Preferences) {
        let path = |slot| directory.join(format!("settings.{slot}.json"));
        let mut candidates = Vec::new();
        let mut corrupt = false;
        let mut read_only = false;
        for slot in 0..2 {
            match read_limited(&path(slot), LIMIT) {
                Ok(data) => {
                    let header = serde_json::from_slice::<serde_json::Value>(&data).ok();
                    if header
                        .as_ref()
                        .and_then(|v| v["schema"].as_u64())
                        .is_some_and(|v| v > SCHEMA as u64)
                    {
                        read_only = true;
                    }
                    match serde_json::from_slice::<Record>(&data) {
                        Ok(record)
                            if record.schema == SCHEMA
                                && record.generation > 0
                                && record.preferences.validate() =>
                        {
                            candidates.push((slot, record))
                        }
                        _ => corrupt = true,
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == io::ErrorKind::InvalidData => corrupt = true,
                Err(_) => read_only = true,
            }
        }
        let recovered = candidates
            .into_iter()
            .max_by_key(|(_, record)| record.generation);
        let (active, generation, preferences) = match recovered {
            Some((slot, record)) => (Some(slot), record.generation, record.preferences),
            None => {
                let mut preferences = Preferences::default();
                match read_limited(&directory.join("settings.json"), LIMIT) {
                    Ok(data) => match serde_json::from_slice::<Legacy>(&data) {
                        Ok(legacy) if legacy.schema == 1 && identity(&legacy.player) => {
                            preferences.profiles[0] = Profile {
                                name: legacy.player.clone(),
                                identity: legacy.player,
                            };
                            preferences.settings.swap_ab = legacy.swap_buttons;
                            preferences.settings.touch = legacy.touch_navigation;
                        }
                        _ => {
                            if serde_json::from_slice::<serde_json::Value>(&data)
                                .ok()
                                .and_then(|v| v["schema"].as_u64())
                                .is_some_and(|v| v > SCHEMA as u64)
                            {
                                read_only = true;
                            }
                            corrupt = true;
                        }
                    },
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) if error.kind() == io::ErrorKind::InvalidData => corrupt = true,
                    Err(_) => read_only = true,
                }
                (None, 0, preferences)
            }
        };
        let notice = if read_only {
            "Settings storage is unavailable or newer. Changes cannot be saved."
        } else if corrupt && active.is_some() {
            "Recovered the last valid launcher settings."
        } else if corrupt {
            "Launcher settings were unreadable. Using safe defaults."
        } else {
            ""
        };
        (
            Self {
                directory,
                active,
                generation,
                notice,
                read_only,
            },
            preferences,
        )
    }

    pub fn commit(&mut self, preferences: &Preferences) -> io::Result<()> {
        if self.read_only || !preferences.validate() {
            return Err(invalid("settings cannot be saved"));
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("settings generation exhausted"))?;
        let record = Record {
            schema: SCHEMA,
            generation,
            preferences: preferences.clone(),
        };
        let data = serde_json::to_vec(&record).map_err(io::Error::other)?;
        fs::create_dir_all(&self.directory)?;
        let inactive = 1 - self.active.unwrap_or(1);
        let staged = self.directory.join(format!("settings.{inactive}.pending"));
        let slot = self.directory.join(format!("settings.{inactive}.json"));
        let mut output = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&staged)?;
        output.write_all(&data)?;
        output.sync_all()?;
        drop(output);
        match fs::remove_file(&slot) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::rename(staged, slot)?;
        self.active = Some(inactive);
        self.generation = generation;
        self.notice = "";
        Ok(())
    }
}
