"""Add Wii U dispositions to every generated official tool definition."""

import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs/mcp"
APP_READ = set("""
versions settings instance_list worlds_list multiplayer_list account_list notes links logs
data_status data_show java_list screenshots_list packs_list worlds_datapack_list instance_versions
instance_others modpack_list mods_list mods_search mods_check servers_list server_log rust_status
storage native_lock native_cache
""".split())
APP_NETWORK = set("""
versions install prepare update share_log unshare_log account_sign_in account_refresh account_skin
java_install rust_install instance_versions instance_java instance_loader mods_search mods_check
mods_install mods_update modpack_port play bug_report worlds_datapack_add packs_add data_allow
data_send data_bug data_benchmark servers_create servers_set_up servers_run server
""".split())
APP_DESTRUCTIVE = set("""
remove account_sign_out account_forget_offline data_delete instance_delete instance_launch
worlds_delete worlds_datapack_remove packs_remove screenshots_delete multiplayer_remove
modpack_remove mods_remove rust_remove servers_forget unshare_log storage
""".split())
GAME_READ = set("""
hello status state screen inventory looking_at entities block light blocks find_block ui messages
narration wait_ready wait_for events input_log logs memory worlds options_get effects stats
record_status record_list servers profile screenshot
""".split())
BUILDER = set("""
install remove prepare native_cache native_lock update java_list java_install rust_status rust_install
rust_remove modpack_port instance_versions instance_java instance_loader instance_version
mods_install mods_update mods_check
""".split())
DESKTOP = set("""
desktop_entry instance_shortcut instance_others instance_copy_from server server_command
server_log server_stop servers_list servers_create servers_set_up servers_run servers_forget
""".split())
ADAPTERS = {
    "versions": "Installed Cafe build and preparation status. Desktop release queries are excluded.",
    "settings": "Read effective Night settings. Desktop settings writes are excluded.",
    "instance_list": "Existing Night installations and their preparation status.",
    "instance_select": "Persistently select a compatible installation.",
    "worlds_list": "Validated saves for the selected instance with availability, name and ID.",
    "multiplayer_list": "Night's shared saved-server list.",
    "multiplayer_add": "Add to Night's shared saved-server list. No connection is made.",
    "play": "Queue a normal hosted launch of the selected installation or an existing local world.",
    "game_wait_ready": "Poll original game status for up to 10 seconds per request.",
    "game_screenshot": "Use the original Cafe TV encoder and return PNG content, bounded to 7 MiB of base64.",
    "game_quit": "Request the original hosted save-and-return path.",
    "game_save": "Request the original game save.",
}
SESSION_OUTPUT = {
    "play": "Text documenting the started version and how to wait for the world.",
    "account_list": "Text with account JSON, current account and pending sign-in status.",
    "account_sign_in": "Text with the Microsoft authorization URI and user code.",
    "server": "Text with version, server directory and recent log lines.",
    "server_command": "Text with matching console output.",
    "server_log": "Text with recent console output.",
    "server_stop": "Text confirming termination or explaining failure.",
}
SCOPE_CONSTANTS = {
    "READ": "launcher.read", "MANAGE": "launcher.manage",
    "OBSERVE": "game.observe", "CONTROL": "game.control",
}


def implemented(source_path=ROOT / "wiiu/game-bridge/night_mcp.rs"):
    source = source_path.read_text(encoding="utf-8")
    launch_policy = re.search(r"^fn requires_launch_policy\([^)]*\)[^{]*\{(.*?)^\}", source, re.M | re.S)
    launch_names = re.findall(r'"([a-z_]+)"', launch_policy[1]) if launch_policy else ["play"]
    result = {name: ["launcher.manage", "game.control"] for name in launch_names}
    for const, scope in SCOPE_CONSTANTS.items():
        declaration = re.search(rf"const {const}: &\[&str\] = &\[(.*?)\];", source, re.S)[1]
        for name in re.findall(r'"([a-z_]+)"', declaration):
            if name in result:
                raise ValueError(f"Duplicate Wii U tool {name}")
            result[name] = [scope]
    return result


def classify(row, runtime, pinned):
    tool = row["tool"]
    name = tool["name"]
    kind = row["kind"]
    game = kind.startswith("gameplay")
    raw_name = name.removeprefix("game_") if game else name
    official = row["officialSideEffects"] or {}
    destructive = official.get("destructive") or name in APP_DESTRUCTIVE or name == "game_world_rename"
    network = official.get("network") or name in APP_NETWORK
    read = raw_name in (GAME_READ if game else APP_READ)
    effects = ["read-only" if read else "mutating"]
    if name in {"settings", "storage"}:
        effects.append("mutating")
    if destructive:
        effects.append("destructive")
    if network:
        effects.append("network")
    if kind == "gameplay_standalone":
        category, detail = 6, "The official launcher filters launch out. Use play."
    elif name in runtime:
        category = 2 if game else 1 if name == "settings" else 3
        detail = ADAPTERS.get(name, "Direct dispatch through the original gameplay control handler.")
        if not pinned:
            detail += " Wii U advertises the pinned schema."
    elif name in BUILDER:
        category, detail = 4, "Uses the pinned desktop builder, preparation, or installation tooling."
    elif name in DESKTOP:
        category, detail = 6, "Desktop integration or dedicated-server hosting is unavailable on CafeOS."
    elif name.startswith("account_"):
        category, detail = 5, "The separate accounts integration owns Microsoft and account tools."
    elif game:
        category, detail = 5, "No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities."
    else:
        category, detail = 5, "Night has no reviewed MCP dispatch for this command."
    scope = runtime.get(name) or [
        ("game.observe" if read else "game.control") if game else
        ("launcher.read" if read else "launcher.manage")
    ]
    if "outputSchema" in tool:
        contract = "Structured MCP output and text JSON. The structured value follows outputSchema."
    elif name == "game_screenshot":
        contract = "An MCP image/png block and a text block with capture metadata."
    elif name == "game_logs":
        contract = "A text block with the log path and recent lines."
    elif kind == "launcher_session":
        contract = SESSION_OUTPUT[name]
    elif game:
        contract = "Text containing the game's JSON control reply. A failed command sets isError."
    else:
        contract = "Text containing --json output or CLI stdout/stderr. A failed command sets isError."
    confirmation = []
    if official.get("destructive"):
        confirmation.append("Official tool requires confirm=true after player agreement.")
    if destructive:
        confirmation.append("Wii U requires console-local confirmation for irreversible operations.")
    if official.get("network"):
        confirmation.append("The agent acts only when the player asks.")
    if name == "play":
        confirmation.append("The player must locally allow gameplay control and agent launches.")
    return {
        **row,
        "officialSupported": True,
        "implementedOnWiiU": category in (1, 2, 3),
        "availability": "gameplay" if game else "launcher-only" if name == "play" else "app-wide",
        "scopes": scope,
        "sideEffects": effects,
        "confirmation": confirmation,
        "category": category,
        "platformDisposition": detail,
        "outputContract": contract,
    }


def write(path, contents, check):
    if check:
        if path.read_text(encoding="utf-8") != contents:
            raise SystemExit(f"Regenerate {path.name}")
    else:
        path.write_text(contents, encoding="utf-8", newline="\n")


def nightly_audit(assembled, check):
    shell = Path("wiiu/loading_app/src")
    registrations = shell / "night_mcp.rs"
    inventory = shell / "pinned_tools.json"
    git = ["git", "-C", str(assembled)]
    files = [registrations.as_posix(), inventory.as_posix()]
    subprocess.run([*git, "diff", "--quiet", "--", *files], check=True)
    tree = subprocess.check_output([*git, "write-tree"], text=True).strip()

    upstream = json.loads((DOCS / "nightly-tools.json").read_bytes())
    recipe = json.loads((ROOT / "catalog/hosted-nightly-2026-11-20.json").read_bytes())
    if upstream["revision"] != recipe["upstream"]["commit"]:
        raise ValueError("Nightly tool inventory differs from the upstream recipe pin")
    if tree not in {patch["output_tree"] for patch in recipe["series"]}:
        raise ValueError("Assembled MCP source tree is outside the Nightly recipe")

    registration_file = assembled / registrations
    runtime = implemented(registration_file)
    registration_source = registration_file.read_text(encoding="utf-8")
    port = json.loads((assembled / inventory).read_bytes())
    embedded = {row["tool"]["name"]: row["tool"] for row in port["tools"]}
    official = {row["tool"]["name"]: row for row in upstream["tools"]}
    catalog = re.search(r"^fn catalog\([^)]*\)[^{]*\{(.*?)^\}", registration_source, re.M | re.S)
    extensions = set(re.findall(r'"name":\s*"([a-z_]+)"', catalog[1])) if catalog else set()
    if extra := runtime.keys() - embedded.keys() - extensions:
        raise ValueError(f"Nightly registers tools with no catalog definition: {extra}")

    tools = {}
    for name, row in sorted(official.items()):
        result = classify(row, runtime, pinned=True)
        entry = {
            "availability": result["availability"],
            "scopes": runtime.get(name),
            "upstreamOutputContract": result["outputContract"],
            "disposition": result["platformDisposition"],
        }
        if name in runtime and name in embedded:
            entry["embeddedDefinitionDifferences"] = [
                key for key in ("description", "inputSchema", "outputSchema")
                if row["tool"].get(key) != embedded[name].get(key)
            ]
        tools[name] = entry

    result = {
        "upstream": upstream["revision"],
        "assembledTree": tree,
        "embeddedRevision": port["revision"],
        "tools": tools,
        "wiiuOnlyTools": {name: runtime[name] for name in sorted(runtime.keys() - official.keys())},
    }
    write(DOCS / "nightly-parity.json", json.dumps(result, indent=2, ensure_ascii=False) + "\n", check)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--nightly-source", type=Path, help="audit a staged Nightly source tree")
    args = parser.parse_args()
    if args.nightly_source:
        nightly_audit(args.nightly_source, args.check)
        return
    runtime = implemented()
    snapshots = []
    report = ["""# MCP tool parity

The pinned schema comes from the actual tool registrations. The second
snapshot audits upstream main as of 20 November 2026. The machine-readable
inventory includes the exact descriptions and schemas, result contracts,
scopes, confirmations and original side-effect annotations. [Inventory](parity.json)

| Category | Disposition |
| --- | --- |
| 1 | Implement directly |
| 2 | Reuse the game implementation |
| 3 | Adapt to CafeOS |
| 4 | Builder-side operation |
| 5 | Deferred until a reviewed backend exists |
| 6 | Not applicable to Wii U |
"""]
    for variant in ("pinned", "reference"):
        source = json.loads((DOCS / f"{variant}-tools.json").read_text(encoding="utf-8"))
        names = {row["tool"]["name"] for row in source["tools"]}
        if len(names) != len(source["tools"]):
            raise ValueError("Duplicate official tool")
        if variant == "pinned" and (extra := runtime.keys() - names):
            raise ValueError(f"Port tool missing from pinned source {extra}")
        rows = [classify(row, runtime, variant == "pinned") for row in source["tools"]]
        snapshots.append({"revision": source["revision"], "tools": rows})
        count = sum(row["implementedOnWiiU"] for row in rows)
        report.extend([
            f"\n## {variant.capitalize()} inventory\n\n{source['revision']}. {len(rows)} entries, {count} mapped to Wii U source.\n",
            "\n| Tool | Official source | Availability | Scopes | Side effects | Wii U disposition |\n",
            "| --- | --- | --- | --- | --- | --- |\n",
        ])
        for row in rows:
            scope = ", ".join(row["scopes"])
            effects = ", ".join(row["sideEffects"])
            report.append(f"| {row['tool']['name']} | {row['source']} | {row['availability']} | {scope} | {effects} | {row['category']}. {row['platformDisposition']} |\n")
    write(DOCS / "parity.json", json.dumps({"snapshots": snapshots}, indent=2, ensure_ascii=False) + "\n", args.check)
    write(DOCS / "parity.md", "".join(report), args.check)


if __name__ == "__main__":
    main()
