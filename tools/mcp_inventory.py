"""Extract the actual Horizon tool registrations without building GPUI or the game."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PIN = "69fab597a6f0e8a6ce524d38d1b73e599e343034"
REFERENCE = "e568ef530e10a524fa5dac1e8faaaa2a3c59762e"
GAME = "crates/horizon-client/src/bin/horizon-ctl/"


def git(revision, path):
    return subprocess.check_output(
        [os.environ.get("HORIZON_MCP_GIT", "git"), "-C", os.environ.get("HORIZON_MCP_GIT_ROOT", str(ROOT)), "show", f"{revision}:{path}"], encoding="utf-8"
    )


def block(source, start, end="\n}"):
    begin = source.index(start)
    return source[begin:source.index(end, begin) + len(end)]


def extract(revision):
    cli = git(revision, "launcher/src/cli.rs")
    definitions = cli[cli.index("#[derive(Parser)]"):cli.index("const SETTING_KEYS")]
    definitions = re.sub(r"^impl [^{]+\{.*?^\}\n", "", definitions, flags=re.M | re.S)
    definitions += block(cli, "const SETTING_KEYS", "\n];")
    definitions = definitions.replace("version = backend::VERSION", 'version = "0.0.0"')
    if "developer_builds::DeveloperInstall" in definitions:
        developer = git(revision, "launcher/src/cli/developer_builds.rs")
        definitions += (
            "\nfn tr(value: &str) -> &str { value }\n"
            "mod developer_builds { use super::{PathBuf, tr};\n#[derive(clap::Args)]\n"
            + block(developer, "pub(super) struct DeveloperInstall") + "\n}\n"
        )
    if "native_mods::Profile" in definitions:
        native = git(revision, "launcher/src/native_mods.rs")
        definitions += (
            "\nmod native_mods {\n#[derive(Clone, Copy, Default, clap::ValueEnum)]\n"
            + block(native, "pub enum Profile") + "\n}\n"
        )
    if "parse_storage_row" in definitions:
        # clap exposes no enumerated values for the custom parser. Keep the same shape.
        definitions += '''
#[derive(Clone, Debug)]
struct ClearStorage;
fn parse_storage_row(_: &str) -> Result<ClearStorage, String> { Ok(ClearStorage) }
'''
    launcher = git(revision, "launcher/src/mcp.rs")
    game = git(revision, GAME + "mcp.rs")
    prefix = git(revision, "launcher/src/game_tools.rs")
    lock = git(revision, "launcher/Cargo.lock")
    clap = re.search(r'\nname = "clap"\nversion = "([^"]+)"', lock)[1]
    main = f'''
#![allow(dead_code)]
mod cli;
mod cli_tools;
mod commands;
mod launcher {{
    use crate::cli_tools;
    use serde_json::{{json, Value}};
    {block(launcher, "fn session_tools(")}
    pub {block(launcher, "fn tools(")}
    pub fn sessions() -> Value {{ session_tools() }}
}}
mod game {{
    use crate::commands;
    use serde_json::{{json, Value}};
    pub {block(game, "fn tools(")}
    {block(game, "fn capital(")}
    {line(prefix, "pub const PREFIX")}
    {line(prefix, "const HIDDEN")}
    pub {block(prefix, "fn prefixed(")}
}}
fn main() {{
    let launcher_tools = launcher::tools();
    let sessions = launcher::sessions();
    let raw_game = game::tools();
    let game_tools = game::prefixed(&serde_json::json!(raw_game));
    let mut tools = Vec::new();
    for tool in launcher_tools {{
        let name = tool["name"].as_str().unwrap();
        let source = cli_tools::all().into_iter().find(|t| t.name == name);
        let implementation = match &source {{
            Some(t) => format!("horizon-launcher {{}}", t.path.join(" ")),
            None => format!("launcher/src/mcp.rs ({{name}})"),
        }};
        let side_effects = source.as_ref().map(|tool| {{
            let words = tool.path.join(" ");
            serde_json::json!({{
                "destructive": cli_tools::DESTRUCTIVE.contains(&words.as_str()),
                "network": cli_tools::OPEN_WORLD.contains(&words.as_str())
            }})
        }});
        let kind = if sessions.as_array().unwrap().iter().any(|t| t["name"] == name) {{
            "launcher_session"
        }} else {{
            "launcher"
        }};
        tools.push(serde_json::json!({{
            "tool": tool, "source": implementation, "kind": kind, "officialSideEffects": side_effects
        }}));
    }}
    for tool in raw_game {{
        let name = tool["name"].as_str().unwrap();
        let command = commands::COMMANDS.iter().find(|c| commands::tool_name(c) == name).unwrap();
        let mapped = game_tools.iter().find(|t| t["name"] == format!("game_{{name}}"));
        tools.push(serde_json::json!({{
            "tool": mapped.unwrap_or(&tool), "source": format!("horizon-ctl {{}}", command.name),
            "kind": if mapped.is_some() {{ "gameplay" }} else {{ "gameplay_standalone" }},
            "group": command.group.title(), "officialSideEffects": {{
                "destructive": commands::DESTRUCTIVE.contains(&command.name),
                "network": commands::OPEN_WORLD.contains(&command.name)
            }}
        }}));
    }}
    println!("{{}}", serde_json::to_string(&tools).unwrap());
}}
'''
    cli_tools = git(revision, "launcher/src/cli_tools.rs")
    cli_tools = re.sub(r"^    /// Runs the command.*?^    \}\n", "", cli_tools, flags=re.M | re.S)

    with tempfile.TemporaryDirectory(prefix="horizon-tool-inventory-") as scratch:
        crate = Path(scratch)
        Cargo = f'''[package]
name = "horizon-tool-inventory"
version = "0.0.0"
edition = "2024"
[dependencies]
clap = {{ version = "={clap}", features = ["derive"] }}
clap_complete = "4.6"
serde_json = "1"
'''
        (crate / "Cargo.toml").write_text(Cargo, encoding="utf-8")
        (crate / "src").mkdir()
        files = {
            "cli.rs": "use clap::{Parser, Subcommand};\nuse std::path::PathBuf;\n" + definitions,
            "cli_tools.rs": cli_tools,
            "commands.rs": git(revision, GAME + "commands.rs"),
            "main.rs": main,
        }
        for filename, content in files.items():
            (crate / "src" / filename).write_text(content, encoding="utf-8")
        data = subprocess.check_output(
            ["cargo", "run", "--quiet", "--manifest-path", str(crate / "Cargo.toml")],
            cwd=crate,
            encoding="utf-8",
        )
        return json.loads(data)


def line(source, start):
    return next(line for line in source.splitlines() if line.startswith(start))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default=PIN)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    rows = extract(args.revision)
    result = json.dumps({"revision": args.revision, "tools": rows}, indent=2, ensure_ascii=False) + "\n"
    if args.check:
        expected = args.output or ROOT / "docs/mcp/pinned-tools.json"
        if args.revision == PIN:
            builder = json.loads((ROOT / "catalog/compatibility-v1.json").read_text())
            if builder["upstream"]["commit"] != PIN:
                raise SystemExit("The builder pin changed. Re-extract and review the tool inventory.")
        if json.loads(expected.read_text(encoding="utf-8")) != json.loads(result):
            raise SystemExit(f"MCP tool inventory drift detected in {expected}")
    elif args.output:
        args.output.write_text(result, encoding="utf-8", newline="\n")
    else:
        print(result, end="")


if __name__ == "__main__":
    main()
