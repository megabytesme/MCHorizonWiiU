#!/usr/bin/env python3
"""Join one frame-lap serial log to its HZP3 capture."""
import argparse
from collections import defaultdict
from hashlib import sha256
import json
from pathlib import Path
import re
import struct

import gx2_profile as analyzer

NAMES = [
    ("prepare", "renderer", "pr_cleanup", "pr_setup", "pr_camera", "pr_listener"),
    ("pr_occlusion", "pr_environment", "pr_debug", "pr_native_visuals", "pr_outline", "pr_projection"),
    ("pr_hud", "pr_entity_setup", "pr_entities", "pr_dragon_bolts", "pr_celestial", "pr_hand"),
    ("pr_screen_end", "pr_weather_border", "pr_lights", "pr_shader_inputs", "pr_frame", "pr_loading"),
]
RECORD = re.compile(
    r"Horizon Cafe frame wall us seq=(\d{1,10}) start_ticks=(\d{1,20}) "
    r"group=([0-3]) samples=\[([\d, ]+)\]"
)
MISSING = (1 << 32) - 1
FRAME_PREFIX = struct.Struct(">4I")
HZP_WALL_MS = (
    "loop", "world_update", "streaming", "tick", "stage_artwork", "terrain_upload",
    "texture_upload", "terrain_wait", "wait_flip", "finish_present", "submit_tv", "submit_drc",
)
HZP_WORKERS_MS = ("streaming_worker", "mesh_worker", "integrated_tick")


def join(data, serial):
    header, frames = analyzer.read(data)
    if header["version"] != 3:
        raise ValueError("frame-lap join requires HZP3")
    # The canonical reader validated sizes, ranges and phases. Read only the
    # raw HZP3 prefix here so the join compares integer ticks without a float
    # round trip through start_ms.
    by_sequence = {}
    for i, frame in enumerate(frames):
        offset = analyzer.HEADER.size + i * analyzer.V3_ROW.size
        sequence, _, hi, lo = FRAME_PREFIX.unpack_from(data, offset)
        by_sequence[sequence] = ((hi << 32) | lo, frame)

    samples = defaultdict(dict)
    mismatched = set()
    for line in serial:
        for match in RECORD.finditer(line):
            sequence, start_ticks, group = map(int, match.groups()[:3])
            if sequence not in by_sequence:
                continue
            if start_ticks != by_sequence[sequence][0]:
                mismatched.add(sequence)
                continue
            if sequence % 120 != 0:
                raise ValueError(f"non-sampled frame-lap sequence {sequence}")
            values = tuple(int(value) for value in match[4].split(","))
            if len(values) != len(NAMES[group]) or any(value > MISSING for value in values):
                raise ValueError(f"invalid frame-lap values at sequence {sequence} group {group}")
            previous = samples[sequence].setdefault(group, values)
            if values != previous:
                raise ValueError(f"conflicting frame-lap row at sequence {sequence} group {group}")

    rows = []
    incomplete, excluded = [], []
    for sequence, groups in sorted(samples.items()):
        if len(groups) != len(NAMES):
            incomplete.append(sequence)
            continue
        start_ticks, frame = by_sequence[sequence]
        if (
            frame["phase"] != "playing"
            or frame["game_frames_built"] != 1
            or frame["presented"] != 1
            or frame["tv_submissions"] != 1
            or frame["gamepad_submissions"] != 1
            or frame["screenshot_ms"]
            or min(frame["tv_sections_drawn"], frame["drc_sections_drawn"]) == 0
        ):
            excluded.append(sequence)
            continue
        row = {
            "sequence": sequence,
            "start_ticks": start_ticks,
            "start_ms": frame["start_ms"],
            "partial": bool(frame["partial"]),
            "tv_sections": frame["tv_sections_drawn"],
            "gamepad_sections": frame["drc_sections_drawn"],
            "world_scale": frame["world_effective_scale"],
            "wall_us": {
                name: value if value != MISSING else None
                for group, names in enumerate(NAMES)
                for name, value in zip(names, groups[group])
            },
            "hzp_wall_ms": {name: frame[f"{name}_ms"] for name in HZP_WALL_MS},
            "hzp_worker_delta_ms": {name: frame[f"{name}_ms"] for name in HZP_WORKERS_MS},
        }
        rows.append(row)
    return {
        "paired_samples": rows,
        "incomplete": incomplete,
        "excluded": excluded,
        "tick_mismatch": sorted(mismatched),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("serial", type=Path)
    parser.add_argument("--serial-capture", action="store_true", help="read UWP capture events")
    parser.add_argument("--serial-first-seq", type=int)
    parser.add_argument("--serial-last-seq", type=int)
    args = parser.parse_args()
    first, last = args.serial_first_seq, args.serial_last_seq
    if first is not None or last is not None:
        if not args.serial_capture or first is None or last is None or not 0 <= first <= last:
            parser.error("use both increasing event bounds with --serial-capture")
    try:
        capture = args.capture.read_bytes()
        if args.serial_capture:
            report = join(capture, analyzer.uwp_serial_lines(args.serial, first, last))
        else:
            with args.serial.open(encoding="utf-8-sig", errors="replace") as serial:
                report = join(capture, analyzer.plain_serial_lines(serial))
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps({"capture_sha256": sha256(capture).hexdigest(), **report}, indent=2))


if __name__ == "__main__":
    main()
