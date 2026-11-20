use super::{store, GameFiles, Launch, LocalService, ResourceStatus, Target};
use serde_json::{json, Value};

#[cfg(not(test))]
const CLIENT_VERSION: &str = env!("HORIZON_NIGHT_CLIENT");
#[cfg(test)]
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Remote calls use the same validation and storage transaction as Night's
// local actions. Paths are resolved from registered instance IDs.
impl<G: GameFiles> LocalService<G> {
    pub(crate) fn tool(&mut self, name: &str, args: &Value) -> Result<Value, &'static str> {
        let selected = self.preferences.instance.clone();
        let contains = |allowed: &[&str]| {
            args.as_object()
                .is_some_and(|a| a.keys().all(|k| allowed.contains(&k.as_str())))
        };
        match name {
            "versions" if contains(&["installed"]) => {
                if args.get("installed").is_some_and(|v| v != true) {
                    return Err("The installed Wii U client is the only version available here.");
                }
                Ok(json!([{"tag": CLIENT_VERSION, "installed": true,
                    "prepared": self.installed(&selected) == ResourceStatus::Ready,
                    "minecraft": self.pin.version, "selected": true}]))
            }
            "settings" if contains(&[]) => Ok(json!({
                "selected_version": CLIENT_VERSION,
                "game_architecture": "powerpc-cafe",
                "swap_ab": self.preferences.settings.swap_ab,
                "swap_xy": self.preferences.settings.swap_xy,
                "touch": self.preferences.settings.touch,
            })),
            "instance_list" if contains(&[]) => {
                let versions = self.listing(super::List::Instances);
                let rows: Vec<_> = versions
                    .into_iter()
                    .map(|item| {
                        json!({"id": item.key, "name": item.key, "selected": item.key == selected,
                        "prepared": self.installed(&item.key) == ResourceStatus::Ready})
                    })
                    .collect();
                Ok(json!(rows))
            }
            "instance_select" if contains(&["instance"]) => {
                let id = args["instance"]
                    .as_str()
                    .ok_or("An installed instance is required.")?;
                if self.installed(id) != ResourceStatus::Ready {
                    return Err("The requested installation is missing or incompatible.");
                }
                let mut next = self.preferences.clone();
                next.instance = id.to_owned();
                if !self.save(next) {
                    return Err("Could not save the selected installation.");
                }
                self.target = Target::Menu;
                Ok(json!({"selected": id}))
            }
            "worlds_list" if contains(&["instance"]) => {
                if args
                    .get("instance")
                    .is_some_and(|v| v.as_str() != Some(&selected))
                {
                    return Err("Select this installed instance before listing its worlds.");
                }
                let rows: Vec<_> = self
                    .listing(super::List::Worlds)
                    .into_iter()
                    .map(|item| {
                        json!({"id": item.key, "name": item.label, "detail": item.detail,
                        "available": item.state != super::DISABLED})
                    })
                    .collect();
                Ok(json!(rows))
            }
            "multiplayer_list" if contains(&["instance"]) => {
                if args
                    .get("instance")
                    .is_some_and(|v| v.as_str() != Some(&selected))
                {
                    return Err("Night has one shared server list. Use the selected instance.");
                }
                Ok(json!(self.preferences.servers))
            }
            "multiplayer_add" if contains(&["instance", "address", "name"]) => {
                if args
                    .get("instance")
                    .is_some_and(|v| v.as_str() != Some(&selected))
                {
                    return Err("Night has one shared server list. Use the selected instance.");
                }
                let address = args["address"]
                    .as_str()
                    .and_then(store::endpoint)
                    .ok_or("Enter a valid hostname or IP address and optional port.")?;
                let name = args
                    .get("name")
                    .map_or(Some("Minecraft Server"), Value::as_str)
                    .filter(|name| store::label(name, 48))
                    .ok_or("Enter a valid server name.")?;
                let mut next = self.preferences.clone();
                if next.servers.len() >= store::ENTRIES {
                    return Err("The server list is full.");
                }
                if !next.servers.iter().any(|server| server.address == address) {
                    next.servers.push(store::Server {
                        name: name.to_owned(),
                        address: address.clone(),
                    });
                    if !self.save(next) {
                        return Err("Could not save the server list.");
                    }
                }
                Ok(json!({"name": name, "address": address}))
            }
            _ => Err("This tool's requested options are unavailable on Wii U."),
        }
    }

    pub(crate) fn tool_launch(&self, args: &Value) -> Result<Launch, &'static str> {
        if !args.as_object().is_some_and(|a| {
            a.keys()
                .all(|key| matches!(key.as_str(), "instance" | "version" | "world"))
        }) {
            return Err(
                "Wii U play supports the selected installation and an existing local world.",
            );
        }
        if args
            .get("instance")
            .is_some_and(|v| v.as_str() != Some(&self.preferences.instance))
            || args
                .get("version")
                .is_some_and(|v| v.as_str() != Some(CLIENT_VERSION))
        {
            return Err("Select the requested installation in Night first.");
        }
        let mut launch = self
            .launch()
            .map_err(|_| "The selected installation or world is unavailable.")?;
        // A remote launch never initiates a multiplayer connection on its own.
        launch.server = None;
        if let Some(world) = args.get("world") {
            let id = world
                .as_str()
                .ok_or("An existing local world is required.")?;
            if !self
                .world(id)
                .is_some_and(|world| world.unavailable.is_none())
            {
                return Err("The existing world is missing, locked, or owned by another profile.");
            }
            launch.world = Some(id.to_owned());
        }
        Ok(launch)
    }
}
