#!/usr/bin/env python3
"""Summarize a hosted Cafe HZP capture or export its samples as CSV."""

import argparse
import csv
import json
import math
from pathlib import Path
import struct
import sys


PHASES = ("unknown", "night", "loading", "title", "world_entry", "playing", "menu", "closing")
STAGES = (
    "input", "idle", "world_update", "streaming", "tick", "entities", "particles", "hud",
    "stage_artwork", "terrain_upload", "texture_upload", "submit_tv", "submit_drc",
    "wait_flip", "finish_present", "terrain_wait", "screenshot", "save_poll", "software_sleep",
)
COUNTERS = (
    "presented", "partial", "section_draws", "indices", "terrain_upload_bytes", "hud_vertices",
    "rust_live_bytes", "rust_peak_bytes", "rust_live_blocks",
)
WORKERS = ("streaming_worker", "mesh_worker", "integrated_tick", "save_work")
V2_STAGES = (
    "clear_tv", "clear_drc", "scan_copy_tv", "scan_copy_drc", "solid_terrain_tv",
    "solid_terrain_drc", "hud_submit_tv", "hud_submit_drc",
)
V2_COUNTERS = (
    "game_frames_built", "tv_submissions", "gamepad_submissions", "gx2_swap_delta",
    "gx2_flip_delta", "gx2_flip_interval_ticks", "tv_sections_tested", "drc_sections_tested",
    "tv_sections_drawn", "drc_sections_drawn", "tv_width", "tv_height", "drc_width", "drc_height",
)
HEADER = struct.Struct(">7I")
ROW = struct.Struct(f">{6 + len(STAGES) + len(COUNTERS) + 2 * len(WORKERS)}I")
V2_ROW = struct.Struct(f">{ROW.size // 4 + len(V2_STAGES) + len(V2_COUNTERS)}I")
FORMATS = {(0x485A5031, 1): ROW, (0x485A5032, 2): V2_ROW}


def read(data):
    if len(data) < HEADER.size:
        raise ValueError("truncated capture header")
    magic, version, hz, size, capacity, count, total = HEADER.unpack_from(data)
    layout = FORMATS.get((magic, version))
    if layout is None or size != layout.size or not hz:
        raise ValueError("unsupported HZP capture")
    if count > min(capacity, total) or len(data) != HEADER.size + count * size:
        raise ValueError("capture length or frame count is invalid")

    scale = 1000 / hz
    frames = []
    previous = None
    for expected, row in enumerate(layout.iter_unpack(data[HEADER.size:]), total - count):
        seq, phase, hi, lo, loop, pacing = row[:6]
        start = (hi << 32) | lo
        if seq != expected or not 0 < phase < len(PHASES):
            raise ValueError("capture frame indices or phase are invalid")
        if frames and start * scale < frames[-1]["start_ms"]:
            raise ValueError("capture clock moved backwards")
        frame = {
            "sequence": seq, "phase": PHASES[phase], "start_ms": start * scale,
            "loop_ms": loop * scale, "pacing_ms": pacing * scale if pacing else None,
        }
        end_stages = 6 + len(STAGES)
        end_counters = end_stages + len(COUNTERS)
        frame.update((f"{key}_ms", cost * scale) for key, cost in zip(STAGES, row[6:end_stages]))
        frame.update(zip(COUNTERS, row[end_stages:end_counters]))
        workers = row[end_counters:ROW.size // 4]
        for index, name in enumerate(WORKERS):
            i = 2 * index
            frame[f"{name}_count"] = None if previous is None else (workers[i] - previous[i]) % 2**32
            frame[f"{name}_ms"] = None if previous is None else (
                (workers[i + 1] - previous[i + 1]) % 2**32
            ) * scale
        if version == 2:
            end_extra_stages = ROW.size // 4 + len(V2_STAGES)
            frame.update((f"{key}_ms", cost * scale) for key, cost in zip(
                V2_STAGES, row[ROW.size // 4:end_extra_stages],
            ))
            frame.update(zip(V2_COUNTERS, row[end_extra_stages:]))
            interval = frame.pop("gx2_flip_interval_ticks")
            frame["gx2_flip_interval_ms"] = interval * scale if interval else None
        frames.append(frame)
        previous = workers
    return {"version": version, "clock_hz": hz, "ring_capacity": capacity,
            "total": total, "retained": count}, frames


def stats(values):
    ordered = sorted(values)
    if not ordered:
        return None
    result = {
        name: round(ordered[math.ceil(len(ordered) * quantile) - 1], 3)
        for name, quantile in (("median", 0.5), ("p95", 0.95), ("p99", 0.99), ("max", 1))
    }
    return {"samples": len(ordered), **result}


def summarize(frames, start, end, stall_ms=50):
    summaries = {}
    for phase in PHASES[1:]:
        selected = [
            row for row in frames
            if row["phase"] == phase and start <= row["start_ms"] / 1000 < end
        ]
        if not selected:
            continue
        presented = [row for row in selected if row["presented"] and not row["screenshot_ms"]]
        pacing = [
            row["pacing_ms"] for previous, row in zip(presented, presented[1:])
            if previous["sequence"] + 1 == row["sequence"] and row["pacing_ms"] is not None
        ]
        partial = sum(row["partial"] for row in presented)
        summary = summaries[phase] = {
            "host_loops": len(selected),
            "presented_loops": len(presented),
            "partial_presentations": partial,
            "complete_presentations": len(presented) - partial,
            "host_loop_ms": stats(row["loop_ms"] for row in selected),
            "frame_pacing_ms": stats(pacing),
            "pacing_stalls": sum(value >= stall_ms for value in pacing),
            "stall_threshold_ms": stall_ms,
            "stages_ms_on_presented_loops": {
                name: stats(row[f"{name}_ms"] for row in presented) for name in STAGES
            },
            "workload": {
                key: stats(row[key] for row in presented) for key in COUNTERS[2:]
            },
            "parallel_work": {
                key: {
                    "completed": sum(row[f"{key}_count"] or 0 for row in selected),
                    "completed_wall_ms": round(sum(row[f"{key}_ms"] or 0 for row in selected), 3),
                } for key in WORKERS
            },
        }
        if "gx2_flip_interval_ms" in selected[0]:
            summary["submission_totals"] = {
                key: sum(row[key] for row in selected) for key in V2_COUNTERS[:5]
            }
            summary["submission_totals"]["paired_submissions"] = sum(row["presented"] for row in selected)
            summary["paired_flip_interval_ms"] = stats(
                row["gx2_flip_interval_ms"] for row in selected if row["gx2_flip_interval_ms"] is not None
            )
            summary["stages_ms_on_presented_loops"].update({
                name: stats(row[f"{name}_ms"] for row in presented) for name in V2_STAGES
            })
            summary["workload"].update({key: stats(row[key] for row in presented) for key in V2_COUNTERS[6:]})
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--csv", type=Path, help="write all retained rows")
    parser.add_argument("--start", type=float, default=0, help="start in seconds after capture activation")
    parser.add_argument("--end", type=float, default=math.inf, help="end in seconds after capture activation")
    parser.add_argument("--stall-ms", type=float, default=50, help="pacing stall threshold")
    args = parser.parse_args()
    if not (0 <= args.start < args.end and 0 < args.stall_ms < math.inf):
        parser.error("invalid interval or stall threshold")
    try:
        metadata, frames = read(args.capture.read_bytes())
        if args.csv is not None and frames:
            with args.csv.open("w", encoding="utf-8", newline="") as target:
                writer = csv.DictWriter(target, fieldnames=frames[0].keys())
                writer.writeheader()
                writer.writerows(frames)
        print(json.dumps({**metadata, "phases": summarize(frames, args.start, args.end, args.stall_ms)}, indent=2))
    except (OSError, ValueError) as err:
        print(f"Capture could not be read. {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
