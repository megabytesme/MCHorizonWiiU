#!/usr/bin/env python3
"""Summarize a hosted Cafe HZP capture or export its samples as CSV."""

import argparse
import base64
import csv
import json
import math
from pathlib import Path
import re
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
V3_STAGES = (
    "world_setup_tv", "world_setup_drc", "translucent_terrain_tv", "translucent_terrain_drc",
    "outline_tv", "outline_drc", "crumbling_tv", "crumbling_drc", "first_person_tv",
    "first_person_drc", "world_composite_tv", "world_composite_drc", "world_sky_tv",
    "world_sky_drc", "world_entities_tv", "world_entities_drc",
)
V3_COUNTERS = (
    "world_requested_scale", "world_effective_scale", "tv_world_width", "tv_world_height",
    "drc_world_width", "drc_world_height", "world_surface_bytes", "translucent_index_upload_bytes",
)
HEADER = struct.Struct(">7I")
ROW = struct.Struct(f">{6 + len(STAGES) + len(COUNTERS) + 2 * len(WORKERS)}I")
V2_ROW = struct.Struct(f">{ROW.size // 4 + len(V2_STAGES) + len(V2_COUNTERS)}I")
V3_ROW = struct.Struct(f">{V2_ROW.size // 4 + len(V3_STAGES) + len(V3_COUNTERS)}I")
FORMATS = {(0x485A5031, 1): ROW, (0x485A5032, 2): V2_ROW, (0x485A5033, 3): V3_ROW}
GPU_SPAN = re.compile(
    r"Horizon GX2 GPU span source=gx2-top-bottom hz=(\d+) seq=(\d+) tv_ticks=(\d+) drc_ticks=(\d+)\b"
)
GPU_ENDPOINTS = re.compile(
    r"Horizon GX2 GPU endpoints seq=(\d+) tv=([0-9a-fA-F]{16})/([0-9a-fA-F]{16})"
    r" drc=([0-9a-fA-F]{16})/([0-9a-fA-F]{16})\b"
)
SERIAL_DELIMITER = re.compile(rb"\r\n|\r|\n")
SERIAL_ANSI = re.compile(rb"\x1b\[[0-9;?]*[ -/]*[@-~]")


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
        if version >= 2:
            end_extra_stages = ROW.size // 4 + len(V2_STAGES)
            frame.update((f"{key}_ms", cost * scale) for key, cost in zip(
                V2_STAGES, row[ROW.size // 4:end_extra_stages],
            ))
            frame.update(zip(V2_COUNTERS, row[end_extra_stages:V2_ROW.size // 4]))
            interval = frame.pop("gx2_flip_interval_ticks")
            frame["gx2_flip_interval_ms"] = interval * scale if interval else None
        if version >= 3:
            end_world_stages = V2_ROW.size // 4 + len(V3_STAGES)
            frame.update((f"{key}_ms", cost * scale) for key, cost in zip(
                V3_STAGES, row[V2_ROW.size // 4:end_world_stages],
            ))
            frame.update(zip(V3_COUNTERS, row[end_world_stages:]))
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
        if "world_effective_scale" in selected[0]:
            summary["stages_ms_on_presented_loops"].update({
                name: stats(row[f"{name}_ms"] for row in presented) for name in V3_STAGES
            })
            world_frames = [row for row in presented if row["world_effective_scale"]]
            summary["world_targets"] = {
                key: stats(row[key] for row in world_frames) for key in V3_COUNTERS[:-1]
            }
            summary["workload"]["translucent_index_upload_bytes"] = stats(
                row["translucent_index_upload_bytes"] for row in world_frames
            )
    return summaries


def _remember_gpu_record(records, sequence, record):
    if records.setdefault(sequence, record) != record:
        raise ValueError(f"conflicting GPU serial records at HZP sequence {sequence}")


def _is_uwp_event(event):
    return (isinstance(event, dict) and type(event.get("seq")) is int
            and isinstance(event.get("kind"), str) and "time" in event and "port" in event)


def plain_serial_lines(source):
    for line in source:
        candidate = line.lstrip("\ufeff \t")
        if candidate.startswith("{"):
            try:
                event = json.loads(candidate)
            except json.JSONDecodeError:
                pass
            else:
                if _is_uwp_event(event):
                    raise ValueError("UWP capture JSONL requires --gpu-serial-capture")
        yield line


def _uwp_rx_chunks(path, last):
    segments = sorted(path.glob("serial-*.jsonl")) if path.is_dir() else [path]
    if not segments:
        raise ValueError("UWP capture directory contains no serial segments")
    previous = None
    for segment in segments:
        with segment.open(encoding="utf-8-sig") as source:
            for number, line in enumerate(source, start=1):
                if not line.strip():
                    continue
                try:
                    event = json.loads(line)
                    if not _is_uwp_event(event):
                        raise ValueError("expected a UWP serial event")
                    sequence = event["seq"]
                    if previous is not None and sequence <= previous:
                        raise ValueError("serial event sequences must increase across segments")
                    if last is not None and sequence > last:
                        return
                    previous = sequence
                    if event["kind"] == "rx":
                        yield sequence, base64.b64decode(event["bytes"], validate=True)
                    if sequence == last:
                        return
                except (ValueError, KeyError, TypeError) as error:
                    raise ValueError(f"invalid UWP serial event in {segment.name}:{number}: {error}") from error


def uwp_serial_lines(path, first=None, last=None):
    pending = b""
    drop_prefix = False
    emitted = False
    for sequence, payload in _uwp_rx_chunks(path, last):
        if first is not None and sequence < first:
            if payload:
                drop_prefix = not payload.endswith((b"\r", b"\n"))
            continue
        complete = SERIAL_DELIMITER.split(pending + payload)
        pending = complete.pop()
        if complete and drop_prefix:
            # The first line started in an RX event before the chosen boundary.
            complete = complete[1:]
            drop_prefix = False
        for raw in complete:
            line = SERIAL_ANSI.sub(b"", raw).decode("utf-8", errors="replace")
            if line:
                emitted = True
                yield line
    if not emitted:
        raise ValueError("no complete UWP serial lines in the selected event range")


def one_uwp_game_session(lines):
    seen_session = False
    active_session = False
    last_span = None
    error = ("UWP serial range spans more than one game session. Select one session using "
             "--serial-first-seq and --serial-last-seq")
    for line in lines:
        if ("Horizon GX2 capture enabled," in line or "Horizon pregame: frame.loading" in line
                or "hosted save barrier completed" in line):
            active_session = False
        span = GPU_SPAN.search(line)
        if (span or GPU_ENDPOINTS.search(line)
                or "Horizon Wii U startup: Cafe Game handoff begin" in line
                or "Horizon Cafe first original Game frame begin" in line):
            if not active_session:
                if seen_session:
                    raise ValueError(error)
                seen_session = active_session = True
        if span:
            sequence = int(span[2])
            if last_span is not None and sequence < last_span:
                raise ValueError(error)
            last_span = sequence
        yield line


def correlate_gpu_spans(frames, serial_lines, start, end):
    """Join a same-run serial log to its HZP3 frame sequence."""
    spans, endpoints = {}, {}
    for line in serial_lines:
        for match in GPU_SPAN.finditer(line):
            hz, sequence, tv, gamepad = map(int, match.groups())
            _remember_gpu_record(spans, sequence, (hz, tv, gamepad))
        for match in GPU_ENDPOINTS.finditer(line):
            sequence, *ticks = match.groups()
            _remember_gpu_record(endpoints, int(sequence), tuple(int(tick, 16) for tick in ticks))
    by_sequence = {frame["sequence"]: frame for frame in frames}
    excluded = dict(missing_serial_line=0, not_retained=0, outside_interval=0,
                    ineligible_frame=0, invalid_endpoints=0)
    rows = []
    for sequence in sorted(spans.keys() | endpoints.keys()):
        if sequence not in spans or sequence not in endpoints:
            excluded["missing_serial_line"] += 1
            continue
        frame = by_sequence.get(sequence)
        if frame is None:
            excluded["not_retained"] += 1
            continue
        if not start <= frame["start_ms"] / 1000 < end:
            excluded["outside_interval"] += 1
            continue
        if (frame["phase"] != "playing" or frame["tv_submissions"] != 1
                or frame["gamepad_submissions"] != 1 or frame["screenshot_ms"]
                or not frame["world_effective_scale"]):
            excluded["ineligible_frame"] += 1
            continue
        hz, tv_ticks, gamepad_ticks = spans[sequence]
        tv_top, tv_bottom, pad_top, pad_bottom = endpoints[sequence]
        if not (hz and 0 < tv_top < tv_bottom < 2**64 - 1
                and 0 < pad_top < pad_bottom < 2**64 - 1
                and tv_ticks == tv_bottom - tv_top and gamepad_ticks == pad_bottom - pad_top):
            excluded["invalid_endpoints"] += 1
            continue
        gap = max(tv_top, pad_top) - min(tv_bottom, pad_bottom)
        if tv_bottom <= pad_top:
            order = "tv_then_gamepad"
        elif pad_bottom <= tv_top:
            order = "gamepad_then_tv"
        elif tv_top == pad_top and tv_bottom == pad_bottom:
            order = "same_interval"
        elif tv_top <= pad_top and pad_bottom <= tv_bottom:
            order = "tv_contains_gamepad"
        elif pad_top <= tv_top and tv_bottom <= pad_bottom:
            order = "gamepad_contains_tv"
        else:
            order = "overlap"
        tick_ms = 1000 / hz
        rows.append({
            "sequence": sequence, "start_ms": frame["start_ms"], "loop_ms": frame["loop_ms"],
            "scale": frame["world_effective_scale"],
            "world_dimensions_tv_gamepad": [
                [frame["tv_world_width"], frame["tv_world_height"]],
                [frame["drc_world_width"], frame["drc_world_height"]],
            ],
            "sections_drawn_tv_gamepad": [frame["tv_sections_drawn"], frame["drc_sections_drawn"]],
            "cpu_submit_ms_tv_gamepad": [frame["submit_tv_ms"], frame["submit_drc_ms"]],
            "cpu_terrain_wait_ms": frame["terrain_wait_ms"],
            "frequency_hz": hz,
            "endpoints_tv_gamepad": [
                [f"{tv_top:016x}", f"{tv_bottom:016x}"],
                [f"{pad_top:016x}", f"{pad_bottom:016x}"],
            ],
            "tv_span_ms": tv_ticks * tick_ms, "gamepad_span_ms": gamepad_ticks * tick_ms,
            "endpoint_order": order, "endpoint_overlap_ms": max(0, -gap) * tick_ms,
            "endpoint_gap_ms": max(0, gap) * tick_ms,
            "paired_bracket_ms": (max(tv_bottom, pad_bottom) - min(tv_top, pad_top)) * tick_ms,
        })
    groups = {}
    for scale in sorted({row["scale"] for row in rows}):
        matching = [row for row in rows if row["scale"] == scale]
        groups[str(scale)] = {
            key: stats(row[key] for row in matching)
            for key in ("tv_span_ms", "gamepad_span_ms", "endpoint_overlap_ms", "endpoint_gap_ms",
                        "paired_bracket_ms")
        }
    return {
        "source": "gx2-top-bottom", "qualified_for_drs": False,
        "matched_samples": len(rows), "excluded_samples": excluded,
        "by_effective_scale": groups, "samples": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--csv", type=Path, help="write all retained rows")
    serial = parser.add_mutually_exclusive_group()
    serial.add_argument("--gpu-serial", type=Path, help="join plain serial text from the same HZP3 run")
    serial.add_argument("--gpu-serial-capture", type=Path, help="join a UWP COM Monitor segment or capture directory")
    parser.add_argument("--serial-first-seq", type=int, help="first UWP serial event in the selected run, inclusive")
    parser.add_argument("--serial-last-seq", type=int, help="last UWP serial event in the selected run, inclusive")
    parser.add_argument("--start", type=float, default=0, help="start in seconds after capture activation")
    parser.add_argument("--end", type=float, default=math.inf, help="end in seconds after capture activation")
    parser.add_argument("--stall-ms", type=float, default=50, help="pacing stall threshold")
    args = parser.parse_args()
    if not (0 <= args.start < args.end and 0 < args.stall_ms < math.inf):
        parser.error("invalid interval or stall threshold")
    first, last = args.serial_first_seq, args.serial_last_seq
    if first is not None or last is not None:
        if args.gpu_serial_capture is None or first is None or last is None or not 1 <= first <= last:
            parser.error("use both serial sequence bounds with --gpu-serial-capture, in increasing order")
    try:
        metadata, frames = read(args.capture.read_bytes())
        report = {**metadata, "phases": summarize(frames, args.start, args.end, args.stall_ms)}
        if (args.gpu_serial is not None or args.gpu_serial_capture is not None) and metadata["version"] != 3:
            raise ValueError("GPU serial correlation requires HZP3")
        if args.gpu_serial is not None:
            with args.gpu_serial.open(encoding="utf-8", errors="replace") as serial:
                report["gx2_gpu_correlation"] = correlate_gpu_spans(
                    frames, plain_serial_lines(serial), args.start, args.end,
                )
        elif args.gpu_serial_capture is not None:
            lines = one_uwp_game_session(uwp_serial_lines(args.gpu_serial_capture, first, last))
            report["gx2_gpu_correlation"] = correlate_gpu_spans(frames, lines, args.start, args.end)
        if args.csv is not None and frames:
            with args.csv.open("w", encoding="utf-8", newline="") as target:
                writer = csv.DictWriter(target, fieldnames=frames[0].keys())
                writer.writeheader()
                writer.writerows(frames)
        print(json.dumps(report, indent=2))
    except (OSError, ValueError) as err:
        print(f"Capture could not be read. {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
