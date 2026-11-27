#!/usr/bin/env python3
"""Summarize a hosted Cafe HZP capture or export its samples as CSV."""

import argparse
import base64
from collections import Counter
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
GPU_SOLID_BOUNDARY = re.compile(
    r"Horizon GX2 GPU solid boundary hz=(\d+) seq=(\d+) "
    r"tv=([0-9a-fA-F]{16})/([0-9a-fA-F]{16})/([0-9a-fA-F]{16}) "
    r"drc=([0-9a-fA-F]{16})/([0-9a-fA-F]{16})/([0-9a-fA-F]{16})\b"
)
GPU_PASSES = re.compile(
    r"Horizon GX2 GPU passes seq=(\d+) target=([01]) hz=(\d+) "
    r"cycles=([0-9a-fA-F]{16}(?:/[0-9a-fA-F]{16}){8})(?![0-9a-fA-F/])"
)
GPU_PASS_NAMES = (
    "world_setup", "sky", "solid_terrain", "entities_before_translucency",
    "translucent_terrain", "remaining_overlays", "world_composite", "hud_and_keyboard",
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


def summarize_flip_quantization(frames, start, end, refresh_hz, terrain_sections=None):
    """Group matched dual-output gameplay by scale, dimensions and visible terrain."""
    refresh_ms = 1000 / refresh_hz
    tolerance_ms = refresh_ms * 0.05
    cohorts = {}
    previous_cohort = previous_sequence = None
    for row in frames:
        cohort = None
        if (row["phase"] == "playing" and start <= row["start_ms"] / 1000 < end
                and row["presented"] == row["tv_submissions"] == row["gamepad_submissions"] == 1
                and row["game_frames_built"] == 1
                and not row["screenshot_ms"]
                and row["world_requested_scale"] == row["world_effective_scale"] > 0
                and min(row["tv_sections_drawn"], row["drc_sections_drawn"]) > 0
                and (terrain_sections is None
                     or row["tv_sections_drawn"] == row["drc_sections_drawn"] == terrain_sections)):
            cohort = (
                row["world_effective_scale"], row["tv_world_width"], row["tv_world_height"],
                row["drc_world_width"], row["drc_world_height"],
                row["tv_sections_drawn"], row["drc_sections_drawn"],
            )
            counts, flips, waits, finishes = cohorts.setdefault(cohort, (Counter(), [], [], []))
            counts["paired_submissions"] += 1
            waits.append(row["wait_flip_ms"])
            finishes.append(row["finish_present_ms"])
            if cohort == previous_cohort and row["sequence"] == previous_sequence + 1:
                counts["consecutive_pairs"] += 1
                counts["gx2_swap_delta"] += row["gx2_swap_delta"]
                counts["gx2_flip_delta"] += row["gx2_flip_delta"]
                if row["gx2_flip_delta"] == 1 and row["gx2_flip_interval_ms"] is not None:
                    flips.append(row["gx2_flip_interval_ms"])
        previous_cohort, previous_sequence = cohort, row["sequence"]

    groups = []
    for cohort, (counts, flips, waits, finishes) in sorted(cohorts.items()):
        scale, tv_w, tv_h, pad_w, pad_h, tv_sections, pad_sections = cohort
        bins = Counter()
        for interval in flips:
            refreshes = round(interval / refresh_ms)
            if refreshes >= 1 and abs(interval - refreshes * refresh_ms) <= tolerance_ms:
                bins[refreshes] += 1
        groups.append({
            "requested_and_effective_scale": scale,
            "world_dimensions_tv_gamepad": [[tv_w, tv_h], [pad_w, pad_h]],
            "sections_tv_gamepad": [tv_sections, pad_sections],
            "paired_submissions": counts["paired_submissions"],
            "consecutive_pairs": counts["consecutive_pairs"],
            "gx2_swap_delta": counts["gx2_swap_delta"],
            "gx2_flip_delta": counts["gx2_flip_delta"],
            "combined_flip_interval_ms": stats(flips),
            "intervals_without_one_timed_flip": counts["consecutive_pairs"] - len(flips),
            "flips_by_refresh_count": dict(sorted(bins.items())),
            "missed_combined_refresh_intervals": (
                sum((refreshes - 1) * count for refreshes, count in bins.items()) if bins else None
            ),
            "off_grid_flip_intervals": len(flips) - sum(bins.values()),
            "whb_wait_flip_ms": stats(waits),
            "whb_finish_ms": stats(finishes),
        })
    return {
        "refresh_hz": refresh_hz,
        "refresh_period_ms": refresh_ms,
        "bin_tolerance_ms": tolerance_ms,
        "cohorts": groups,
    }


def summarize_stock_saved_view(frames, start, end):
    """Measure the 174-section fixed-75% reference across its full retained wall interval."""
    rows = [row for row in frames if start <= row["start_ms"] / 1000 < end]
    if len(rows) < 2 or rows[0]["start_ms"] == rows[-1]["start_ms"]:
        raise ValueError("the saved-view window needs two distinct retained times")
    wall_ms = rows[-1]["start_ms"] - rows[0]["start_ms"]
    gameplay_pairs = [
        row for row in rows
        if row["phase"] == "playing"
        and row["presented"] == row["tv_submissions"] == row["gamepad_submissions"] == 1
        and row["game_frames_built"] == 1
    ]
    reference = [
        row for row in gameplay_pairs
        if not row["screenshot_ms"]
        and row["world_requested_scale"] == row["world_effective_scale"] == 75
        and row["tv_sections_drawn"] == row["drc_sections_drawn"] == 174
        and min(row[key] for key in ("tv_width", "tv_height", "drc_width", "drc_height",
                                    "tv_world_width", "tv_world_height",
                                    "drc_world_width", "drc_world_height")) > 0
    ]
    # Each interval is billed to its ending frame. The first retained row
    # supplies the clock boundary and cannot also count as an interval.
    timed_pairs = sum(row["sequence"] != rows[0]["sequence"] for row in reference)
    intervals = [
        current["start_ms"] - prior["start_ms"]
        for prior, current in zip(reference, reference[1:])
    ]
    partial = sum(row["partial"] for row in reference)
    dimensions = sorted({
        (row["tv_width"], row["tv_height"], row["drc_width"], row["drc_height"],
         row["tv_world_width"], row["tv_world_height"],
         row["drc_world_width"], row["drc_world_height"])
        for row in reference
    })
    return {
        "first_sequence": rows[0]["sequence"],
        "last_sequence": rows[-1]["sequence"],
        "wall_ms": round(wall_ms, 3),
        "at_least_60_seconds_retained": wall_ms >= 60_000,
        "host_loops": len(rows),
        "non_playing_loops": sum(row["phase"] != "playing" for row in rows),
        "gameplay_paired_submissions": len(gameplay_pairs),
        "paired_submissions_outside_reference": len(gameplay_pairs) - len(reference),
        "reference_paired_submissions": len(reference),
        "reference_complete_submissions": len(reference) - partial,
        "reference_partial_submissions": partial,
        "reference_pairs_per_second": round(1000 * timed_pairs / wall_ms, 3),
        "reference_pair_intervals_ms": stats(intervals),
        "reference_dimensions": [
            {"outputs_tv_gamepad": [list(size[0:2]), list(size[2:4])],
             "world_tv_gamepad": [list(size[4:6]), list(size[6:8])]} for size in dimensions
        ],
        "reference_section_draws": stats(row["section_draws"] for row in reference),
        "reference_indices": stats(row["indices"] for row in reference),
        "rust_live_bytes": stats(row["rust_live_bytes"] for row in rows),
        "rust_peak_bytes": max(row["rust_peak_bytes"] for row in rows),
    }


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
        if (span or GPU_ENDPOINTS.search(line) or GPU_SOLID_BOUNDARY.search(line) or GPU_PASSES.search(line)
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
    spans, endpoints, solid_boundaries, pass_boundaries = {}, {}, {}, {}
    for line in serial_lines:
        for match in GPU_SPAN.finditer(line):
            hz, sequence, tv, gamepad = map(int, match.groups())
            _remember_gpu_record(spans, sequence, (hz, tv, gamepad))
        for match in GPU_ENDPOINTS.finditer(line):
            sequence, *ticks = match.groups()
            _remember_gpu_record(endpoints, int(sequence), tuple(int(tick, 16) for tick in ticks))
        for match in GPU_SOLID_BOUNDARY.finditer(line):
            hz, sequence, *ticks = match.groups()
            _remember_gpu_record(solid_boundaries, int(sequence), (int(hz), *(int(tick, 16) for tick in ticks)))
        for match in GPU_PASSES.finditer(line):
            sequence, target, hz, ticks = match.groups()
            views = pass_boundaries.setdefault(int(sequence), {})
            record = (int(hz), tuple(int(tick, 16) for tick in ticks.split("/")))
            if views.setdefault(int(target), record) != record:
                raise ValueError(f"conflicting GPU pass records at HZP sequence {sequence}, target {target}")
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
    report = {
        "source": "gx2-top-bottom", "qualified_for_drs": False,
        "matched_samples": len(rows), "excluded_samples": excluded,
        "by_effective_scale": groups, "samples": rows,
    }
    if solid_boundaries:
        report["solid_boundary"] = correlate_solid_boundaries(solid_boundaries, rows, by_sequence)
    if pass_boundaries:
        report["pass_boundaries"] = correlate_pass_boundaries(pass_boundaries, rows, by_sequence)
    return report


def correlate_solid_boundaries(boundaries, rows, frames):
    """Keep optional solid completion markers only when the original pair also matches."""
    by_sequence = {row["sequence"]: row for row in rows}
    samples = []
    invalid = unmatched = 0
    for sequence, (hz, tv_top, tv_solid, tv_bottom, pad_top, pad_solid, pad_bottom) in sorted(boundaries.items()):
        row = by_sequence.get(sequence)
        if row is None:
            unmatched += 1
            continue
        original = [int(value, 16) for pair in row["endpoints_tv_gamepad"] for value in pair]
        if (original != [tv_top, tv_bottom, pad_top, pad_bottom] or hz != row["frequency_hz"]
                or not (tv_top < tv_solid < tv_bottom and pad_top < pad_solid < pad_bottom)
                or frames[sequence]["game_frames_built"] != 1
                or min(row["sections_drawn_tv_gamepad"]) <= 0):
            invalid += 1
            continue
        tick_ms = 1000 / hz
        samples.append({
            "sequence": sequence, "scale": row["scale"],
            "tv_through_solid_ms": (tv_solid - tv_top) * tick_ms,
            "tv_after_solid_ms": (tv_bottom - tv_solid) * tick_ms,
            "gamepad_through_solid_ms": (pad_solid - pad_top) * tick_ms,
            "gamepad_after_solid_ms": (pad_bottom - pad_solid) * tick_ms,
        })
    timings = ("tv_through_solid_ms", "tv_after_solid_ms", "gamepad_through_solid_ms", "gamepad_after_solid_ms")
    groups = {}
    for scale in sorted({row["scale"] for row in samples}):
        matching = [row for row in samples if row["scale"] == scale]
        groups[str(scale)] = {key: stats(row[key] for row in matching) for key in timings}
    return {"matched_samples": len(samples), "unmatched_samples": unmatched, "invalid_samples": invalid,
            "by_effective_scale": groups, "samples": samples}


def correlate_pass_boundaries(boundaries, rows, frames):
    base = {row["sequence"]: row for row in rows}
    excluded = dict(unmatched_pair=0, missing_target=0, invalid_boundaries=0)
    samples = []
    for sequence, views in sorted(boundaries.items()):
        row = base.get(sequence)
        if row is None:
            excluded["unmatched_pair"] += 1
            continue
        if len(views) != 2:
            excluded["missing_target"] += 1
            continue
        if (frames[sequence]["game_frames_built"] != 1 or frames[sequence]["presented"] != 1
                or min(row["sections_drawn_tv_gamepad"]) <= 0):
            excluded["unmatched_pair"] += 1
            continue
        valid = all(
            hz == row["frequency_hz"] and list(ticks) == sorted(ticks)
            and [ticks[0], ticks[-1]] == [int(value, 16) for value in row["endpoints_tv_gamepad"][view]]
            for view, (hz, ticks) in views.items()
        )
        if not valid:
            excluded["invalid_boundaries"] += 1
            continue
        tick_ms = 1000 / row["frequency_hz"]
        spans = []
        for view in range(2):
            _, ticks = views[view]
            elapsed = ((end - begin) * tick_ms for begin, end in zip(ticks, ticks[1:]))
            spans.append(dict(zip(GPU_PASS_NAMES, elapsed)))
        samples.append({
            "sequence": sequence, "scale": row["scale"],
            "sections_drawn_tv_gamepad": row["sections_drawn_tv_gamepad"],
            "partial": bool(frames[sequence]["partial"]),
            "spans_ms_tv_gamepad": spans,
        })
    groups = {}
    for scale in sorted({row["scale"] for row in samples}):
        matching = [row for row in samples if row["scale"] == scale]
        groups[str(scale)] = [
            {name: stats(row["spans_ms_tv_gamepad"][view][name] for row in matching)
             for name in GPU_PASS_NAMES}
            for view in range(2)
        ]
    return {"matched_samples": len(samples), "excluded_samples": excluded,
            "by_effective_scale_tv_gamepad": groups, "samples": samples}


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
    parser.add_argument("--stock-saved-view", action="store_true",
                        help="measure the fixed 75%% / 174-section TV and GamePad reference")
    parser.add_argument("--flip-refresh-hz", type=float, help="bin combined driver flips against this measured refresh rate")
    parser.add_argument("--flip-terrain-sections", type=int, help="require this visible section count on both outputs")
    args = parser.parse_args()
    if not (0 <= args.start < args.end and 0 < args.stall_ms < math.inf):
        parser.error("invalid interval or stall threshold")
    if args.flip_refresh_hz is not None and not (1 <= args.flip_refresh_hz < math.inf):
        parser.error("invalid flip refresh rate")
    if args.flip_terrain_sections is not None and (
            args.flip_refresh_hz is None or args.flip_terrain_sections < 1):
        parser.error("use a positive flip terrain section count with --flip-refresh-hz")
    first, last = args.serial_first_seq, args.serial_last_seq
    if first is not None or last is not None:
        if args.gpu_serial_capture is None or first is None or last is None or not 1 <= first <= last:
            parser.error("use both serial sequence bounds with --gpu-serial-capture, in increasing order")
    try:
        metadata, frames = read(args.capture.read_bytes())
        report = {**metadata, "phases": summarize(frames, args.start, args.end, args.stall_ms)}
        if args.stock_saved_view:
            if metadata["version"] != 3:
                raise ValueError("the stock saved-view comparison requires HZP3")
            report["stock_saved_view"] = summarize_stock_saved_view(frames, args.start, args.end)
        if args.flip_refresh_hz is not None:
            if metadata["version"] != 3:
                raise ValueError("matched flip quantization requires HZP3")
            if args.flip_refresh_hz > metadata["clock_hz"]:
                raise ValueError("flip refresh rate exceeds the capture clock frequency")
            report["gx2_flip_quantization"] = summarize_flip_quantization(
                frames, args.start, args.end, args.flip_refresh_hz, args.flip_terrain_sections,
            )
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
