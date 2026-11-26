#!/usr/bin/env python3
"""Run bounded saved-world rounds on a fixture prepared by gx2_cemu.py."""

import argparse
from ctypes import wintypes
from datetime import datetime, timezone
import gzip
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

import gx2_cemu


BAD_MARKERS = ("Crashlog for Cemu", "panicked at", "frame.startup_error",
               "rust allocation failed", "alloc_failed", "upload failed",
               "AX shutdown skipped", "save compression failed")
POSITION_TAG = b"\x09\x00\x03Pos\x06\x00\x00\x00\x03"
GAME_MODE_TAG = b"\x03\x00\x0eplayerGameType"
DEATH_TAG = b"\x0a\x00\x11LastDeathLocation"
HEALTH_TAG = b"\x05\x00\x06Health"
GROUND_TAG = b"\x01\x00\x08OnGround"
MOVEMENT_KEYS = {"forward": 87, "backward": 83, "left": 65, "right": 68}
CAFE_MENU_PHASE = r'Horizon Cafe cadence: .*\bphase=menu\b'
MENU_PHASE = r'"phase":"menu"|' + CAFE_MENU_PHASE
GAMEPLAY_PHASE = r'"phase":"gameplay"|Horizon Cafe cadence: .*\bphase=gameplay\b'
TERRAIN_SAMPLE = r"Horizon Cafe streaming: .*\btv_sections=(\d+) gamepad_sections=(\d+)\b"


def timestamp():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def png_dimensions(data):
    if len(data) < 24 or data[:16] != b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR":
        raise ValueError("Cemu capture lacks a valid PNG header")
    return struct.unpack_from(">II", data, 16)


class Lab:
    def __init__(self, root, process, require_distinct_captures=False):
        self.root = root
        self.process = process
        self.require_distinct_captures = require_distinct_captures
        self.log = gx2_cemu.paths(root)[0] / "log.txt"
        self.phase = "night"

    def record(self, event, **fields):
        record = {"at": timestamp(), "phase": self.phase, "event": event, **fields}
        line = json.dumps(record, ensure_ascii=False)
        with (self.root / "acceptance.jsonl").open("a", encoding="utf-8") as trace:
            trace.write(line + "\n")
        print(line, flush=True)

    def mark(self):
        return self.log.stat().st_size if self.log.is_file() else 0

    def wait(self, marker, timeout, offset=0, qualifies=None):
        deadline = time.monotonic() + timeout
        pattern = re.compile(marker)
        pending = b""
        while time.monotonic() < deadline:
            if self.log.is_file():
                with self.log.open("rb") as handle:
                    handle.seek(offset)
                    data = handle.read(256 * 1024)
                    offset = handle.tell()
                pending += data
                lines = pending.split(b"\n")
                pending = lines.pop()[-16384:]
                for raw in lines:
                    line = raw.decode("utf-8", "replace")
                    if any(bad in line for bad in BAD_MARKERS):
                        raise RuntimeError(f"Cafe or Cemu reported a fault in {self.phase}")
                    match = pattern.search(line)
                    if match and (qualifies is None or qualifies(match)):
                        return match
            if self.process.poll() is not None:
                raise RuntimeError(f"Cemu exited with code {self.process.returncode}")
            time.sleep(0.25)
        raise TimeoutError(f"no {marker!r} after {timeout:g} seconds")

    def send(self, action, **kwargs):
        defaults = {"hold": 0.25, "timeout": 12}
        defaults.update(kwargs)
        gx2_cemu.run(argparse.Namespace(lab=self.root, action=action, **defaults))

    def show(self, name):
        self.send("shot", name=name)
        tv = f"{name}-tv.png"
        gamepad = f"{name}-gamepad.png"
        tv_png = (self.root / tv).read_bytes()
        gamepad_png = (self.root / gamepad).read_bytes()
        tv_dimensions = png_dimensions(tv_png)
        gamepad_dimensions = png_dimensions(gamepad_png)
        identical = tv_png == gamepad_png
        self.record("displays", tv=tv, gamepad=gamepad, tv_dimensions=tv_dimensions,
                    gamepad_dimensions=gamepad_dimensions, identical_captures=identical)
        if identical and self.require_distinct_captures:
            raise ValueError("Cemu produced identical TV and GamePad captures")

    def select_world(self):
        start = self.mark()
        self.send("tap", x=150, y=263)
        self.wait(r"frame.local.library", 8, start)
        start = self.mark()
        self.send("tap", x=150, y=175)
        self.wait(r"frame.local.worlds", 8, start)
        self.show("world-list")
        start = self.mark()
        self.send("tap", x=150, y=175)
        self.wait(r"frame.home.library", 8, start)

    def close(self):
        if self.process.poll() is not None:
            return
        try:
            user, windows = gx2_cemu.input_window(self.process.pid, "Cemu ")
            user.PostMessageW.argtypes = (
                wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM
            )
            user.PostMessageW(windows[0], 0x0010, 0, 0)
            self.process.wait(5)
        except (OSError, ValueError, subprocess.TimeoutExpired):
            self.process.terminate()
            self.process.wait(10)


def player_record(world):
    player_files = list((world / "players/data").glob("*.dat"))
    if len(player_files) != 1:
        raise ValueError("the offline fixture must contain exactly one player record")
    record = player_files[0]
    limit = 1024 * 1024
    with gzip.open(record, "rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError("the offline fixture player record exceeds 1 MiB")
    return record, data


def tag_offset(data, tag):
    if data.count(tag) != 1:
        raise ValueError(f"fixture player record has no unique {tag!r} tag")
    return data.index(tag) + len(tag)


def position(world):
    _, data = player_record(world)
    return struct.unpack_from(">ddd", data, tag_offset(data, POSITION_TAG))


def prepare_fall(world):
    record, original = player_record(world)
    if DEATH_TAG in original or struct.unpack_from(">i", original, tag_offset(original, GAME_MODE_TAG))[0] != 0:
        raise ValueError("the fall fixture must be an unused offline Survival player")
    if struct.unpack_from(">f", original, tag_offset(original, HEALTH_TAG))[0] <= 0:
        raise ValueError("the fall fixture must start alive")
    data = bytearray(original)
    y_offset = tag_offset(data, POSITION_TAG) + 8
    elevated_y = struct.unpack_from(">d", data, y_offset)[0] + 96
    if not -64 < elevated_y < 319:
        raise ValueError("the elevated fall fixture is outside the build height")
    struct.pack_into(">d", data, y_offset, elevated_y)
    if GROUND_TAG in data:
        data[tag_offset(data, GROUND_TAG)] = 0
    updated = record.with_suffix(".acceptance-new")
    with updated.open("xb") as out:
        out.write(gzip.compress(data, mtime=0))
    updated.replace(record)
    return elevated_y


def configure_fixture_controls(root, respawn=False):
    """Map keys to the virtual GamePad only in the disposable controller profile."""
    path = root / "portable/controllerProfiles/controller0.xml"
    tree = ET.parse(path)
    root_element = tree.getroot()
    if root_element.findtext("type") != "Wii U GamePad":
        raise ValueError("the fixture controller must be a Wii U GamePad")
    mappings = root_element.find("controller/mappings")
    if mappings is None or root_element.findtext("controller/api") != "Keyboard":
        raise ValueError("the fixture must have a keyboard-backed GamePad")
    remap = {"1": 90, "9": 80, "17": 87}  # Z is A, P is Plus and W is left stick up.
    if respawn:
        remap.update({"11": 81, "4": 73})  # Q is D-pad Up and I is Y.
    keys = {str(key) for key in remap.values()}
    for entry in list(mappings):
        if entry.findtext("mapping") in remap or entry.findtext("button") in keys:
            mappings.remove(entry)
    for button, virtual_key in remap.items():
        entry = ET.SubElement(mappings, "entry")
        ET.SubElement(entry, "mapping").text = button
        ET.SubElement(entry, "button").text = str(virtual_key)
    tree.write(path, encoding="utf-8", xml_declaration=True)


def respawn_fixture(lab, start, hold):
    lab.phase = "wait-for-death"
    lab.wait(MENU_PHASE, 240, start)
    lab.show("after-fall")
    time.sleep(2)  # The death screen intentionally ignores immediate input.
    lab.phase = "respawn"
    offset = lab.mark()
    lab.send("key", key=81, hold=hold)
    lab.show("death-focused")
    lab.send("key", key=90, hold=hold)
    try:
        lab.wait(GAMEPLAY_PHASE, 85, offset)
    except TimeoutError:
        lab.show("confirm-retry-state")
        lab.send("key", key=90, hold=hold)
        lab.wait(GAMEPLAY_PHASE, 85, offset)
    lab.show("respawned")

    lab.phase = "inventory"
    offset = lab.mark()
    lab.send("key", key=73, hold=hold)
    try:
        lab.wait(MENU_PHASE, 45, offset)
    except TimeoutError:
        lab.record("inventory-missing")
        return False
    lab.show("inventory")
    offset = lab.mark()
    lab.send("key", key=80, hold=hold)
    lab.wait(GAMEPLAY_PHASE, 45, offset)
    lab.record("inventory-closed")
    return True


def tap_save(lab, x, y, hold, offset, deadline):
    lab.send("tap", x=x, y=y, hold=hold)
    try:
        lab.wait(r"Horizon Cafe touch: dispatch complete kind=release\b|hosted save barrier",
                 min(12, deadline - time.monotonic()), offset)
    except TimeoutError:
        # The interpreter can miss a touch immediately after a capture. Retry
        # once if the paused game is still submitting frames after that touch.
        try:
            lab.wait(CAFE_MENU_PHASE, min(5, deadline - time.monotonic()), lab.mark())
        except TimeoutError:
            return
        lab.record("save-touch-retry")
        lab.send("tap", x=x, y=y, hold=min(hold, 3))


def exercise(args):
    if args.minimum_terrain_sections < 0:
        raise ValueError("minimum terrain sections must be nonnegative")
    if not 1 <= args.sessions <= 5:
        raise ValueError("choose one to five same-process sessions")
    if not all(math.isfinite(value) and value > 0 for value in
               (args.startup_timeout, args.save_timeout, args.audio_timeout)):
        raise ValueError("phase timeouts must be positive and finite")
    if not 0 <= args.gameplay_dwell <= 300:
        raise ValueError("gameplay dwell must be between zero and 300 seconds")
    if args.spawn_xz is not None and (not args.respawn_fixture
                                     or not all(map(math.isfinite, args.spawn_xz))):
        raise ValueError("set a finite expected respawn location only for the fall fixture")
    root = args.lab.resolve()
    portable, app = gx2_cemu.paths(root)
    if not (root / "input-hashes.json").is_file() or (root / "process.txt").exists():
        raise ValueError("prepare a fresh lab before changing the disposable fixture")
    world = app / "saves" / args.world
    if (world.parent != app / "saves" or world.resolve() != world
            or not (world / "level.dat").is_file()):
        raise ValueError("prepare a default-instance lab with the disposable world first")
    before = position(world)
    elevated_y = prepare_fall(world) if args.respawn_fixture else None
    configure_fixture_controls(root, args.respawn_fixture)
    player_path, _ = player_record(world)
    with (root / "acceptance-inputs.json").open("x", encoding="utf-8") as receipt:
        json.dump({"controller_sha256": gx2_cemu.sha256(portable / "controllerProfiles/controller0.xml"),
                   "fixture_level_sha256": gx2_cemu.sha256(world / "level.dat"),
                   "fixture_player_sha256": gx2_cemu.sha256(player_path), "fall_y": elevated_y,
                   "spawn_xz": args.spawn_xz,
                   "sessions": args.sessions, "save_at": args.save_at,
                   "gameplay_dwell": args.gameplay_dwell,
                   "movement_direction": args.movement_direction,
                   "minimum_terrain_sections": args.minimum_terrain_sections}, receipt, indent=2)
    lab = Lab(root, gx2_cemu.start(root, args.cpu_mode, args.title_id),
              args.require_distinct_captures)
    audio_failures = []
    inventory_ok = None
    movement_ok = None
    try:
        lab.wait(r"frame.home.play", 60)
        trace = lab.log.read_text(encoding="utf-8", errors="replace")
        mode = gx2_cemu.observed_mode(trace)
        lab.record("night-ready", cpu_mode=mode)
        if mode != args.cpu_mode:
            raise ValueError(f"requested CPU mode {args.cpu_mode}, observed {mode}")
        # The interpreter can take several seconds to consume a VPAD sample.
        game_hold = 6 if mode == "interpreter" else 0.5
        lab.show("night")
        lab.select_world()
        for index in range(1, args.sessions + 1):
            lab.phase = f"session-{index}-startup"
            start = lab.mark()
            started = time.monotonic()
            lab.send("tap", x=150, y=168)
            lab.wait(r"frame.loading", 25, start)
            game = lab.wait(r"first original Game frame returned elapsed_ms=(\d+) world_ready=true",
                            args.startup_timeout, start)
            lab.record("gameplay-ready", elapsed_ms=int(game[1]))
            lab.wait(r"Horizon prepared I/O: stage=block_models_sprites", 5, start)
            if args.minimum_terrain_sections:
                remaining = args.startup_timeout - (time.monotonic() - started)
                terrain = lab.wait(
                    TERRAIN_SAMPLE, remaining, lab.mark(),
                    qualifies=lambda sample: min(map(int, sample.groups())) >= args.minimum_terrain_sections,
                )
                lab.record("terrain-ready", tv_sections=int(terrain[1]),
                           gamepad_sections=int(terrain[2]),
                           elapsed_ms=round((time.monotonic() - started) * 1000))
            lab.show(f"session-{index}-gameplay")
            if index == 1 and elevated_y is not None:
                inventory_ok = respawn_fixture(lab, start, game_hold)
            if index == 1 and args.gameplay_dwell:
                lab.phase = "stationary-gameplay"
                lab.record("dwell-start", duration_seconds=args.gameplay_dwell)
                try:
                    lab.process.wait(args.gameplay_dwell)
                except subprocess.TimeoutExpired:
                    pass
                else:
                    raise RuntimeError("Cemu exited during the stationary scene")
                lab.show("stationary-gameplay")

            if index == 1:
                key = MOVEMENT_KEYS[args.movement_direction]
                lab.send("key", key=key, hold=max(2, game_hold))
                time.sleep(16 if elevated_y is not None else 4)
            lab.phase = f"session-{index}-audio"
            try:
                lab.wait(r"Horizon AX first nonzero PCM delivered", 5, start)
            except TimeoutError:
                # The disposable Night profile uses the default Nintendo face buttons.
                lab.send("key", key=90, hold=game_hold)
                try:
                    lab.wait(r"Horizon AX first nonzero PCM delivered", args.audio_timeout, start)
                except TimeoutError:
                    lab.record("ax-pcm-missing")
                time.sleep(8)
            lab.phase = f"session-{index}-save"
            pause = lab.mark()
            lab.send("key", key=80, hold=game_hold)
            lab.wait(MENU_PHASE, 25, pause)
            lab.show(f"session-{index}-pause")
            save = lab.mark()
            deadline = time.monotonic() + args.save_timeout
            tap_save(lab, *args.save_at, game_hold, save, deadline)
            for marker in (r"hosted save barrier completed", r"hosted game and loading workers released",
                           r"frame.home.play"):
                lab.wait(marker, deadline - time.monotonic(), save)
            lab.wait(r"Cafe sound decoders joined: 2", 5, save)
            ax = lab.wait(r"AX session closed callback=(\d+) nonzero=(\d+) queue_write=(\d+) queue_read=(\d+)",
                          5, save)
            if ax[1] != "1" or ax[2] != "1" or int(ax[4]) <= 0:
                audio_failures.append(index)
            memory = lab.wait(r"hosted.after_game_drop rust_live=(\d+).*rust_peak=(\d+)",
                              5, save)
            bulk = lab.wait(r"hosted.after_game_drop bulk_free=(\d+).*bulk_largest=(\d+)",
                            5, save)
            persisted = position(world)
            if index == 1:
                if elevated_y is None:
                    movement_ok = persisted[::2] != before[::2]
                if elevated_y is not None:
                    _, data = player_record(world)
                    health = struct.unpack_from(">f", data, tag_offset(data, HEALTH_TAG))[0]
                    if DEATH_TAG not in data or health <= 0 or persisted[1] >= elevated_y:
                        raise ValueError("durable player data did not confirm death and respawn")
                    lab.record("respawn-confirmed", activation="emulated GamePad D-pad and A",
                               health=health, position=persisted)
                    if args.spawn_xz is not None:
                        movement_ok = persisted[::2] != tuple(args.spawn_xz)
            if index > 1 and persisted != before:
                raise ValueError("the untouched reopened session rewrote the durable position")
            if int(bulk[1]) != 234880920 or int(bulk[2]) != 234880896:
                raise ValueError("the bulk heap did not recover its baseline free space")
            lab.record("save-completed", rust_live=int(memory[1]), rust_peak=int(memory[2]),
                       bulk_free=int(bulk[1]), bulk_largest=int(bulk[2]),
                       ax_callback=ax[1] == "1", ax_nonzero=ax[2] == "1",
                       ax_queue_write=int(ax[3]), ax_queue_read=int(ax[4]),
                       saved_position=list(persisted))
            before = persisted
            lab.show(f"session-{index}-night")
        lab.phase = "close"
        closing = lab.mark()
        lab.send("tap", x=150, y=400)
        lab.wait(r"coreinit.exit\(0\)", 25, closing)
        lab.record("local-rounds-completed", sessions=args.sessions,
                   audio_failures=audio_failures, mcp="requires a pre-authorized fixture",
                   death="verified" if elevated_y is not None else "separate scenario",
                   inventory=inventory_ok, movement_verified=movement_ok)
        if audio_failures:
            raise RuntimeError(f"AX PCM did not pass in sessions {audio_failures}")
        if inventory_ok is False:
            raise RuntimeError("the virtual GamePad did not open the inventory")
        if movement_ok is False:
            raise RuntimeError("movement left no horizontal change in the durable player record")
    except Exception as error:
        lab.record("failure", detail=str(error))
        if lab.process.poll() is None:
            try:
                lab.show("failure")
            except (OSError, ValueError, subprocess.SubprocessError) as capture_error:
                lab.record("failure-capture-unavailable", detail=str(capture_error))
        raise
    finally:
        lab.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lab", type=Path)
    parser.add_argument("--world", default="Reopen Fixture")
    parser.add_argument("--cpu-mode", choices=gx2_cemu.CPU_MODES, required=True)
    parser.add_argument("--title-id")
    parser.add_argument("--sessions", type=int, default=3)
    parser.add_argument("--save-at", type=int, nargs=2, required=True,
                        help="verified Save & Quit button center in the 854x480 GamePad fixture")
    parser.add_argument("--startup-timeout", type=float, default=1500)
    parser.add_argument("--save-timeout", type=float, default=240)
    parser.add_argument("--audio-timeout", type=float, default=180)
    parser.add_argument("--gameplay-dwell", type=float, default=0,
                        help="seconds to keep the first gameplay view stationary")
    parser.add_argument("--movement-direction", choices=MOVEMENT_KEYS, default="forward",
                        help="direction to move from the saved fixture pose")
    parser.add_argument("--minimum-terrain-sections", type=int, default=0,
                        help="wait for at least this many visible sections per output before gameplay captures")
    parser.add_argument("--require-distinct-captures", action="store_true",
                        help="fail if the emulator returns the same PNG for both outputs")
    parser.add_argument("--respawn-fixture", action="store_true",
                        help="raise the disposable Survival player for a controller-only fall and Respawn")
    parser.add_argument("--spawn-xz", type=float, nargs=2,
                        help="known fall fixture respawn X and Z used to verify subsequent movement")
    try:
        exercise(parser.parse_args())
    except (OSError, RuntimeError, ValueError, struct.error, subprocess.SubprocessError) as error:
        print(f"Cemu acceptance stopped: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
