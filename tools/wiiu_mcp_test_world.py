#!/usr/bin/env python3
"""Qualify disposable Night worlds with one existing, exclusively used Codex session."""

import argparse
from dataclasses import asdict, dataclass
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import stat
import time

from mcp_codex_cemu_test import paired_session
from wiiu_mcp_lifecycle_test import launch_id, response_text


FOLDER = "Disposable Test World"
WORLD_ID = re.compile(r"[0-9a-f]{32}")
TERMINAL_BENCHMARK = {"idle", "completed", "cancelled", "aborted", "failed"}


@dataclass(frozen=True)
class Limits:
    ready: float = 1800
    terrain: float = 180
    save: float = 180
    reopen: float = 1200
    sections: int = 1


def object_reply(reply):
    value = reply.get("structuredContent")
    if not isinstance(value, dict):
        value = json.loads(response_text(reply))
    if not isinstance(value, dict):
        raise ValueError("Expected an object in the MCP reply")
    return value


def plain(path, directory=False):
    metadata = path.lstat()
    wanted = stat.S_ISDIR if directory else stat.S_ISREG
    reparse = getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
    if not wanted(metadata.st_mode) or reparse:
        raise ValueError(f"An expected save path is not a plain {'directory' if directory else 'file'}")
    return metadata


def digest(path):
    before = plain(path)
    with path.open("rb") as file:
        value = hashlib.file_digest(file, "sha256").hexdigest()
    after = plain(path)
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise ValueError("A save file changed while it was being hashed")
    return value


def ordinary_saves(root):
    def entries(directory):
        plain(directory, directory=True)
        for path in sorted(directory.iterdir()):
            name = path.relative_to(root).as_posix()
            if stat.S_ISDIR(path.lstat().st_mode):
                yield [name, "directory"]
                yield from entries(path)
            else:
                yield [name, "file", digest(path)]

    rows = list(entries(root))
    # Require a known ordinary world so an empty root cannot qualify preservation.
    if not any(len(row) == 3 and row[0].count("/") == 1
               and row[0].endswith("/level.dat") for row in rows):
        raise ValueError("The installation mirror needs an ordinary save with level.dat")
    return {
        "files": sum(len(row) == 3 for row in rows),
        "sha256": hashlib.sha256(json.dumps(rows, ensure_ascii=True).encode()).hexdigest(),
    }


def saved_world(installation, instance, world):
    sandbox = installation / ".night-test-worlds"
    workspace = sandbox / world
    save = workspace / "saves" / FOLDER
    for directory in (sandbox, workspace, save.parent, save):
        plain(directory, directory=True)
    marker = workspace / "horizon-night-test-world.json"
    plain(marker)
    owner = json.loads(marker.read_text(encoding="utf-8"))
    if (owner.get("schema"), owner.get("id"), owner.get("instance")) != (1, world, instance):
        raise ValueError("The saved workspace marker belongs to another world or installation")

    level = save / "level.dat"
    level_hash = digest(level)
    with gzip.open(level, "rb") as nbt:
        if nbt.read(1) != b"\x0a":
            raise ValueError("The disposable level.dat has no NBT compound")
        while nbt.read(65536):
            pass  # Read through the gzip checksum.

    player_data = save / "players" / "data"
    plain(player_data.parent, directory=True)
    plain(player_data, directory=True)
    players = {path.name for path in player_data.glob("*.dat") if plain(path).st_size}
    overworld = save / "dimensions" / "minecraft" / "overworld"
    regions = overworld / "region"
    for directory in (overworld.parent.parent, overworld.parent, overworld, regions):
        plain(directory, directory=True)
    written = 0
    for path in regions.glob("r.*.*.mca"):
        if plain(path).st_size > 8192:
            with path.open("rb") as file:
                written += bool(any(file.read(4096)))
    if not players or not written:
        raise ValueError("The save has no persistent player data or indexed Overworld terrain")
    marker_hash = digest(marker)
    return (marker_hash, players), {
        "marker_sha256": marker_hash, "level_sha256": level_hash,
        "player_files": len(players), "regions_with_chunks": written,
    }


def game_ready(state):
    player = state.get("player")
    position = player.get("position") if isinstance(player, dict) else None
    server = state.get("server")
    return (
        state.get("screen") == "playing"
        and type(state.get("pending_chunks")) is int and state["pending_chunks"] == 0
        and (not isinstance(server, dict) or server.get("loaded") is not False)
        and isinstance(position, list) and len(position) == 3
        and all(type(value) in (int, float) and math.isfinite(value) for value in position)
    )


def terrain_sample(state, previous, sections):
    sample = state.get("cafe_performance") or {}
    at = sample.get("sample_at_session_ms")
    counts = sample.get("last_terrain_sections")
    pairs = sample.get("paired_submissions")
    if (
        state.get("screen") == "playing" and sample.get("phase") == "gameplay"
        and type(at) in (int, float) and math.isfinite(at) and at > previous
        and type(pairs) is int and pairs > 0
        and isinstance(counts, list) and len(counts) == 2
        and all(type(count) is int and count >= sections for count in counts)
    ):
        return {"sample_at_session_ms": at, "paired_submissions": pairs, "sections_tv_gamepad": counts}
    return None


def qualify(call, installation, instance, seed, limits, evidence=None, *,
            clock=time.monotonic, sleep=time.sleep):
    """Run serialized calls on the supplied session. Evidence records completed checkpoints."""
    evidence = {} if evidence is None else evidence

    def tool(name, args=None):
        return object_reply(call(name, args))

    def until(label, started, timeout, probe):
        deadline = started + timeout
        while clock() < deadline:
            value = probe()
            if value is not None and clock() <= deadline:
                return value
            remaining = deadline - clock()
            if remaining > 0:
                sleep(min(0.75, remaining))
        raise TimeoutError(f"{label} exceeded {timeout:g} seconds")

    def home():
        status = tool("wiiu_benchmark_status")
        return status["phase"] in TERMINAL_BENCHMARK and status["night_home_ready"] is True

    def worlds():
        listing = tool("wiiu_test_worlds_list")
        if listing["instance"] != instance:
            raise ValueError("Night selected a different installation than the supplied mirror")
        return {entry["id"]: entry["available"] for entry in listing["worlds"]}

    def ready(ticket):
        status = tool("wiiu_loading_status", {"launch_id": ticket})
        if status["launch_id"] != ticket or status["phase"] not in {
            "queued", "loading", "handoff_started",
        }:
            raise RuntimeError(f"Unexpected disposable launch state {status['phase']}")
        if status["phase"] != "handoff_started":
            return None
        try:
            status = tool("game_wait_ready", {"seconds": 0})
        except RuntimeError as error:
            failure = str(error)
            if failure.endswith("Auth=notLoggedIn"):
                raise
            if failure.startswith((
                "Codex did not list game_wait_ready from ",
                "Tool call game_wait_ready failed. The world is still loading.",
                "Tool call game_wait_ready failed. No Horizon game is running under Agent Control.",
                "Tool call game_wait_ready failed. The game did not accept the request in time.",
            )):
                return None
            raise
        return status if game_ready(status) else None

    def paired(previous):
        try:
            status = tool("game_status")
        except RuntimeError as error:
            if str(error) == "Tool call game_status failed. The game did not accept the request in time.":
                return None
            raise
        return terrain_sample(status, previous, limits.sections)

    evidence.update(stage="preflight", instance=instance, seed=seed, limits=asdict(limits))
    if not home():
        raise RuntimeError("Night must be idle at Home before a disposable world qualification")
    existing = worlds()
    ordinary_root = installation / "saves"
    original = ordinary_saves(ordinary_root)
    evidence["ordinary_saves"] = original
    world = first_ticket = saved_identity = None

    for operation, ready_limit in (("create", limits.ready), ("reopen", limits.reopen)):
        evidence["stage"] = f"{operation}_launch"
        started = clock()
        reply = call("wiiu_test_world_create", {"seed": seed}) if operation == "create" else (
            call("wiiu_test_world_open", {"id": world})
        )
        ticket = launch_id(reply)
        fields = object_reply(reply)
        identifier = fields.get("world_id")
        if fields.get("instance") != instance or not isinstance(identifier, str) or not WORLD_ID.fullmatch(identifier):
            raise ValueError("Night returned no valid disposable world ID for this installation")
        if operation == "create":
            if identifier in existing:
                raise ValueError("Night reused an existing disposable world ID")
            world, first_ticket = identifier, ticket
            evidence["world_id"] = world
        elif identifier != world or ticket == first_ticket:
            raise ValueError("Night did not reopen the same disposable world in a new launch")
        step = evidence[operation] = {"launch_id": ticket}

        evidence["stage"] = f"{operation}_game_ready"
        status = until("Game readiness", started, ready_limit, lambda: ready(ticket))
        step["ready_seconds"] = clock() - started
        earlier = (status.get("cafe_performance") or {}).get("sample_at_session_ms") or 0
        evidence["stage"] = f"{operation}_paired_terrain"
        started = clock()
        step["paired_terrain"] = until("Paired terrain", started, limits.terrain,
                                       lambda: paired(earlier))
        step["terrain_seconds"] = clock() - started

        evidence["stage"] = f"{operation}_durable_save"
        started = clock()
        # Quit starts Cafe's durable save barrier. Its reply only acknowledges
        # the request. Standalone save has a shorter integrated-server RPC deadline.
        if tool("game_quit").get("ok") is not True:
            raise RuntimeError("Game did not acknowledge the normal Quit")
        step["quit_ack_seconds"] = clock() - started
        until("Save and return to Night Home", started, limits.save, lambda: True if home() else None)
        if worlds().get(world) is not True:
            raise ValueError("Night does not report the saved disposable world as available")
        identity, files = saved_world(installation, instance, world)
        if saved_identity is not None and (identity[0] != saved_identity[0]
                                          or not saved_identity[1].issubset(identity[1])):
            raise ValueError("Reopening did not preserve the world marker and existing player data")
        saved_identity = identity
        step["saved_files"] = files
        step["save_and_return_seconds"] = clock() - started
        if step["save_and_return_seconds"] > limits.save:
            raise TimeoutError(f"Durable save verification exceeded {limits.save:g} seconds")

        evidence["stage"] = f"{operation}_ordinary_save_identity"
        if ordinary_saves(ordinary_root) != original:
            raise ValueError("The ordinary saves tree changed during qualification")
        step["ordinary_saves_unchanged"] = True

    evidence["stage"] = "complete"
    return evidence


def main():
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog=("Keep this grant exclusive to one Codex process throughout the run. "
                "Use the actual scratch SD path and keep the Windows mount path short on RC45 and RC46."),
    )
    parser.add_argument("--codex-home", type=Path, required=True, help="the single existing paired profile")
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--server", required=True)
    parser.add_argument("--install-root", type=Path, required=True, help="PC-visible root of the selected scratch installation")
    parser.add_argument("--instance", default="default")
    parser.add_argument("--seed", type=int, default=1234)
    parser.add_argument("--ready-timeout", type=float, default=Limits.ready)
    parser.add_argument("--terrain-timeout", type=float, default=Limits.terrain)
    parser.add_argument("--save-timeout", type=float, default=Limits.save)
    parser.add_argument("--reopen-timeout", type=float, default=Limits.reopen)
    parser.add_argument("--min-sections-per-display", type=int, default=Limits.sections)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    limits = Limits(args.ready_timeout, args.terrain_timeout, args.save_timeout,
                    args.reopen_timeout, args.min_sections_per_display)
    times = (limits.ready, limits.terrain, limits.save, limits.reopen)
    if limits.sections < 1 or not all(math.isfinite(value) and value > 0 for value in times):
        parser.error("Provide positive finite qualification limits")
    if not -(1 << 63) <= args.seed < (1 << 63):
        parser.error("The seed must fit a signed 64-bit integer")
    evidence = {}
    with args.output.open("x", encoding="utf-8") as receipt:
        try:
            with paired_session(args.codex_home, args.ca, args.server) as session:
                qualify(session.call, args.install_root.resolve(), args.instance, args.seed, limits, evidence)
        except (RuntimeError, OSError, ValueError, KeyError, EOFError) as error:
            json.dump({"status": "failed", "reason": str(error), **evidence}, receipt, indent=2)
            receipt.write("\n")
            raise SystemExit(f"Disposable world qualification failed. {error}") from error
        json.dump({"status": "passed", **evidence}, receipt, indent=2)
        receipt.write("\n")


if __name__ == "__main__":
    main()
