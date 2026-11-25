#!/usr/bin/env python3
"""Read loading milestones and matched gameplay windows from a bounded UWP serial capture."""

import argparse
import json
from pathlib import Path
import re
from statistics import median

from gx2_profile import uwp_serial_lines


GUEST_LINE = re.compile(r"^(\d+);(\d+);(\d+);(\d+): (.*)")
CAFE_WINDOW = re.compile(
    r"Horizon Cafe (cadence|pacing|streaming|CPU per frame|wall game|wall render|world): t_ms=(\d+) (.*)"
)
FIELD = re.compile(r"(\w+)=(\S+)")
STAGE_NAME = re.compile(r"[a-z][a-z0-9_.]*")
DIMENSIONS = re.compile(r"\d+x\d+")
DECIMAL = re.compile(r"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?")
STARTUP = "Horizon Wii U startup: "
TRACE = "Horizon startup "
IO = "Horizon prepared I/O: stage="
SPRITES = "Horizon sprite cache: "
NATIVE = "Horizon GX2 profile: "
MEMORY = "Horizon memory hosted.after_game_drop "
ENTRY = STARTUP + "hosted game entry"
NIGHT = "Horizon pregame: frame.home.play"
LOADING = "Horizon pregame: frame.loading"
READY = re.compile(r"Horizon Cafe first original Game frame returned elapsed_ms=(\d+) world_ready=true\b")
GENERATOR = re.compile(r"Horizon: worldgen data loaded in (\d+(?:\.\d+)?)(s|ms|µs|ns)$")
PRESENTATION_MARKS = (
    "loading first draw returned", "Cafe Game handoff begin", "original Game initialized; pending entry starts",
    "first original Game frame returned", "Game world-entry pending state cleared",
    "hosted save barrier completed", "hosted game and loading workers released",
)
WALL_FIELDS = {
    "wall game": ("game_ms", "idle_ms", "artwork_ms", "other_ms"),
    "wall render": ("redraw_ms", "submit_ms", "gx2_wait_ms", "outside_game_ms"),
}


def fields(text):
    """Retain only the numeric and enumerated fields used by Cafe telemetry."""
    parsed = {}
    for key, value in FIELD.findall(text):
        if value.startswith("Some(") and value.endswith(")"):
            value = value[5:-1]
        if re.fullmatch(r"0x[0-9a-fA-F]+", value):
            parsed[key] = int(value, 16)
        elif re.fullmatch(r"-?\d+", value):
            parsed[key] = int(value)
        elif DECIMAL.fullmatch(value):
            parsed[key] = float(value)
        elif value in ("None", "true", "false"):
            parsed[key] = {"None": None, "true": True, "false": False}[value]
        elif (key in ("tv", "gamepad") and DIMENSIONS.fullmatch(value)
              or key == "phase" and value in ("loading", "title", "gameplay", "menus", "menu")):
            parsed[key] = value
    return parsed


def remember(records, key, value):
    if records.setdefault(key, value) != value:
        raise ValueError(f"conflicting Cafe telemetry for {key}")


def read_session(lines):
    entry = night_before = loading_before = ready_counter = generator_ms = None
    markers, spans, windows, read_totals, sprite_cache, native, memory = {}, {}, {}, {}, {}, [], {}
    for line in lines:
        # OSReport also emits unclocked copies. Timing the guest-prefixed record avoids counting both.
        match = GUEST_LINE.match(line)
        if not match:
            continue
        hours, minutes, seconds, millis = map(int, match.groups()[:4])
        guest_ms = ((hours * 60 + minutes) * 60 + seconds) * 1000 + millis
        message = match[5]
        if message == ENTRY:
            if entry is not None and guest_ms != entry:
                raise ValueError("select one hosted entry with the UWP sequence bounds")
            entry = guest_ms
        if entry is None:
            if message == NIGHT:
                night_before = guest_ms
            elif message == LOADING:
                loading_before = guest_ms
            continue
        elapsed = guest_ms - entry
        if message.startswith(STARTUP):
            name = message[len(STARTUP):]
            if name in PRESENTATION_MARKS:
                markers.setdefault(name, elapsed)
        elif message == NIGHT:
            markers.setdefault("Night returned", elapsed)
        elif message.startswith(TRACE):
            event = json.loads("{" + message[len(TRACE):] + "}")
            name, start, duration = event.get("name"), event.get("ts"), event.get("dur")
            if (event.get("ph") == "X" and isinstance(name, str) and STAGE_NAME.fullmatch(name)
                    and type(start) is int and type(duration) is int and min(start, duration) >= 0):
                # A named span can legitimately run more than once. Exact copies carry the same start and duration.
                span = {"trace_start_ms": start / 1000, "duration_ms": duration / 1000}
                named = spans.setdefault(name, [])
                if span not in named:
                    named.append(span)
        elif message.startswith(IO):
            stage, _, values = message[len(IO):].partition(" ")
            if STAGE_NAME.fullmatch(stage):
                remember(read_totals, stage, fields(values))
        elif message.startswith(SPRITES):
            sprite_cache = fields(message[len(SPRITES):])
        elif message.startswith(NATIVE):
            native.append({"guest_elapsed_ms": elapsed, **fields(message[len(NATIVE):])})
        elif message.startswith(MEMORY):
            memory.update(fields(message[len(MEMORY):]))
        elif match := READY.match(message):
            if ready_counter is None:
                ready_counter = int(match[1])
        elif match := GENERATOR.fullmatch(message):
            generator_ms = round(float(match[1]) * {"s": 1000, "ms": 1, "µs": 0.001, "ns": 0.000001}[match[2]], 3)
        elif match := CAFE_WINDOW.match(message):
            kind, stamp, values = match.groups()
            window = windows.setdefault(int(stamp), {})
            remember(window, kind, {"guest_elapsed_ms": elapsed, **fields(values)})
    if entry is None:
        raise ValueError("the selected UWP range contains no guest-clocked hosted entry")
    for name, stamp in (("Night last observed", night_before), ("Night loading frame", loading_before)):
        if stamp is not None:
            markers[name] = stamp - entry
    terrain = [
        {"session_ms": stamp, **window["streaming"]}
        for stamp, window in sorted(windows.items())
        if window.get("cadence", {}).get("paired", 0) > 0 and "streaming" in window
    ]
    for name, threshold in (("first paired nonzero sampled terrain", 1), ("first sampled 75 sections per display", 75)):
        for sample in terrain:
            if min(sample["tv_sections"], sample["gamepad_sections"]) >= threshold:
                markers[name] = sample["guest_elapsed_ms"]
                break
    return {
        "guest_elapsed_milestones_ms": markers,
        "world_ready_host_counter_ms": ready_counter,
        "generator_data_construction_ms": generator_ms,
        "startup_trace_spans": spans,
        "cumulative_prepared_io": read_totals,
        "sprite_cache": sprite_cache,
        "memory_after_game_drop": memory,
        "paired_terrain_samples": terrain,
        "native_profile_windows": native,
        "cafe_windows": [{"session_ms": stamp, **window} for stamp, window in sorted(windows.items())],
    }


def matched_windows(windows, first, last, phase="gameplay"):
    """Summarize explicit stationary windows and refuse mixed scales or interrupted windows."""
    selected = [window for window in windows if first <= window["session_ms"] <= last]
    required = ("cadence", "pacing", "streaming", "wall game", "wall render", "world")
    if not selected or (selected[0]["session_ms"], selected[-1]["session_ms"]) != (first, last):
        raise ValueError("both selected Cafe window endpoints must be present")
    if any(not all(key in window for key in required) or window["cadence"]["phase"] != phase
           for window in selected):
        raise ValueError(f"selected windows must have all telemetry for phase {phase}")
    identity_keys = ("requested", "effective", "tv", "gamepad", "surface_bytes", "omitted_passes")
    world = {key: selected[0]["world"][key] for key in identity_keys}
    for window in selected:
        cadence, streaming = window["cadence"], window["streaming"]
        if (cadence["paired"] == 0 or min(streaming["tv_sections"], streaming["gamepad_sections"]) <= 0
                or any(window["world"][key] != value for key, value in world.items())):
            raise ValueError("selected windows must contain paired terrain at one scale and pass coverage")
    for before, after in zip(selected, selected[1:]):
        gap = after["session_ms"] - before["session_ms"] - after["cadence"]["span_ms"]
        if abs(gap) > 2:
            raise ValueError("selected Cafe windows are not contiguous")

    duration = sum(window["cadence"]["span_ms"] for window in selected)
    pairs = sum(window["cadence"]["paired"] for window in selected)
    partial = sum(window["cadence"]["partial"] for window in selected)
    loops = sum(window["cadence"]["loops"] for window in selected)
    invalid_steps = sum(window["wall game"]["invalid_steps"] for window in selected)
    wall = {}
    for group, keys in WALL_FIELDS.items():
        for key in keys:
            values = [window[group].get(key) for window in selected]
            wall[key] = (round(sum(value * window["cadence"]["loops"]
                                   for value, window in zip(values, selected)) / loops, 3)
                         if not invalid_steps and None not in values else None)
    ticks = [window["streaming"]["ticks"] for window in selected]
    section_ranges = {}
    for key in ("tv_sections", "gamepad_sections"):
        sections = [window["streaming"][key] for window in selected]
        section_ranges[key] = [min(sections), max(sections)]
    result = {
        "phase": phase, "window_count": len(selected), "first_window_end_ms": first, "last_window_end_ms": last,
        "duration_ms": round(duration, 3), "paired_submissions": pairs, "partial_submissions": partial,
        "complete_submissions": pairs - partial, "paired_per_second": round(pairs * 1000 / duration, 3),
        "elapsed_per_pair_ms": round(duration / pairs, 3),
        "invalid_game_steps": invalid_steps, "wall_ms_per_iteration": wall,
        "median_of_window_pacing_ms": {key: median(window["pacing"][key] for window in selected)
                                       for key in ("median_ms", "p95_ms", "p99_ms")},
        "maximum_window_gap_ms": max(window["pacing"]["max_ms"] for window in selected),
        "section_ranges": section_ranges,
        "streaming_totals": {key: sum(window["streaming"][key] for window in selected)
                             for key in ("meshed", "uploaded", "upload_bytes")},
        "integrated_server_tps": round(sum(ticks) * 1000 / duration, 3) if None not in ticks else None,
        "resolution_and_coverage": world,
    }
    hud_waits = [window.get("CPU per frame", {}).get("hud_wait_ms") for window in selected]
    built_frames = [window["cadence"].get("built", 0) for window in selected]
    if None not in hud_waits and all(built_frames):
        result["direct_hud_wait_ms_per_built_frame"] = round(
            sum(wait * frames for wait, frames in zip(hud_waits, built_frames)) / sum(built_frames), 3,
        )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="archived UWP segment or capture directory")
    parser.add_argument("--serial-first-seq", type=int, required=True)
    parser.add_argument("--serial-last-seq", type=int, required=True)
    window = parser.add_mutually_exclusive_group()
    window.add_argument("--gameplay-window", type=int, nargs=2, metavar=("FIRST_MS", "LAST_MS"),
                        help="select stationary gameplay window ends by session-relative t_ms")
    window.add_argument("--menu-window", type=int, nargs=2, metavar=("FIRST_MS", "LAST_MS"),
                        help="select verified stationary menu window ends by session-relative t_ms")
    args = parser.parse_args()
    if not 1 <= args.serial_first_seq <= args.serial_last_seq:
        parser.error("serial sequence bounds must be positive and increasing")
    try:
        report = read_session(uwp_serial_lines(args.capture, args.serial_first_seq, args.serial_last_seq))
        report["serial_event_bounds"] = [args.serial_first_seq, args.serial_last_seq]
        if args.gameplay_window is not None:
            report["matched_gameplay"] = matched_windows(report["cafe_windows"], *args.gameplay_window)
        elif args.menu_window is not None:
            report["matched_menu"] = matched_windows(report["cafe_windows"], *args.menu_window, phase="menu")
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
