#!/usr/bin/env python3
"""Summarize recorded terrain coverage in hosted GX2 benchmark reports."""

import argparse
from itertools import groupby
import json
from pathlib import Path
import re
from sys import float_info

import gx2_profile


SCENARIOS = {
    "village": ("village", 12),
    "cafe-diagnostic-village": ("village", 12),
    "cafe-stationary": ("cafe-stationary", 30),
}
COVERAGE = (
    "host_frames",
    "paired_frames",
    "terrain_paired_frames",
    "eligible_intervals",
    "paired_terrain_intervals",
    "longest_ineligible_run_frames",
    "elapsed_ms",
    "eligible_interval_ms",
)
TARGETS = (
    "configured_scale",
    "effective_scale",
    "tv_width",
    "tv_height",
    "gamepad_width",
    "gamepad_height",
    "tv_world_width",
    "tv_world_height",
    "gamepad_world_width",
    "gamepad_world_height",
    "world_surface_bytes",
)
VILLAGE_TRACE = re.compile(
    r"Cafe Village streaming seq=(\d+) elapsed_ms=(\d+) reason=(progress|lost|recovered)"
    r" camera=(-?\d+\.\d+),(-?\d+\.\d+) sections=(\d+)/(\d+)"
    r" columns=(\d+) client_pending=(\d+) uploads=(\d+) remote_pending=(\d+)"
    r" forward_resident=([0-9a-f]) forward_lit=([0-9a-f])\b"
)


def number(value, *, signed=False):
    minimum = -float_info.max if signed else 0
    if type(value) in (int, float) and minimum <= value <= float_info.max:
        return value
    expected = "a finite number" if signed else "a finite nonnegative number"
    raise ValueError(f"expected {expected}")


def metric(metrics, key, *, signed=False):
    if key not in metrics:
        return None
    summary = metrics[key]
    if not isinstance(summary, dict) or summary.get("n") != 1:
        raise ValueError(f"{key} needs one recorded value")
    try:
        return number(summary.get("mean"), signed=signed)
    except ValueError as error:
        raise ValueError(f"invalid metric {key}") from error


def terrain_coverage(metrics, phase, frame_count):
    values = {key: metric(metrics, f"cafe.{phase}.{key}") for key in COVERAGE}
    missing = [key for key, value in values.items() if value is None]
    if missing:
        return {"evidence": "missing", "missing": missing, **values}

    host, paired, terrain, eligible, retained, gap, elapsed, eligible_ms = values.values()
    if (
        any(int(count) != count for count in (host, paired, terrain, eligible, retained, gap))
        or not 0 < retained == frame_count <= eligible <= max(0, terrain - 1)
        or not terrain <= paired <= host
        or not gap <= host - terrain
        or not 0 < eligible_ms <= elapsed
    ):
        raise ValueError("inconsistent frame and terrain coverage values")
    fraction = eligible_ms / elapsed
    return {
        "evidence": "uninterrupted" if eligible == host - 1 else "gaps",
        "eligible_wall_fraction": fraction,
        **values,
    }


def summarize(report):
    if (
        not isinstance(report, dict)
        or report.get("format") != "horizon-benchmark-report"
        or report.get("version") != 1
        or report.get("kind") != "game"
    ):
        raise ValueError("expected a version 1 Horizon game benchmark report")
    conditions = report.get("conditions")
    metrics = report.get("metrics")
    if not isinstance(conditions, dict) or not isinstance(metrics, dict):
        raise ValueError("expected benchmark conditions and metrics")
    if conditions.get("backend") != "gx2":
        raise ValueError("expected a hosted GX2 benchmark report")
    scenario = conditions.get("scenario")
    if not isinstance(scenario, str) or scenario not in SCENARIOS:
        raise ValueError(f"unsupported benchmark scenario {scenario!r}")
    phase, duration = SCENARIOS[scenario]
    frames = report.get("frames")
    if not isinstance(frames, list) or len(frames) != 1:
        raise ValueError("expected one measured frame series")
    frame = frames[0]
    if not isinstance(frame, dict) or frame.get("phase") != phase or frame.get("repeat") != 0:
        raise ValueError(f"expected the measured {phase} phase")
    frame_count = frame.get("frames")
    if type(frame_count) is not int or frame_count <= 0:
        raise ValueError("expected a nonempty measured frame series")

    coverage = terrain_coverage(metrics, phase, frame_count)
    if scenario == "cafe-stationary" and coverage["evidence"] != "missing":
        coverage["meets_stationary_coverage"] = (
            coverage["eligible_intervals"] >= 10
            and coverage["eligible_wall_fraction"] >= 0.8
            and coverage["elapsed_ms"] >= duration * 1000
        )
    result = {
        "scenario": scenario,
        "phase": phase,
        "requested_duration_s": duration,
        "conditions": conditions,
        "targets": {key: metric(metrics, f"cafe.{key}") for key in TARGETS},
        "coverage": coverage,
        "frame_series": {
            key: number(frame[key]) for key in (
                "frames", "seconds", "p50_ms", "p95_ms", "p99_ms", "cpu_p50_ms", "gpu_p50_ms",
            ) if frame.get(key) is not None
        },
    }
    if scenario == "cafe-stationary":
        camera = {
            key: metric(metrics, f"cafe.stationary.camera.{key}", signed=True)
            for key in ("x", "y", "z", "yaw", "pitch")
        }
        if any(value is not None for value in camera.values()):
            if any(value is None for value in camera.values()):
                raise ValueError("incomplete stationary camera evidence")
            result["camera"] = camera
        sections = {
            f"{output}_{boundary}": metric(metrics, f"cafe.stationary.{output}_sections_{boundary}")
            for output in ("tv", "gamepad")
            for boundary in ("before", "after")
        }
        if any(value is not None for value in sections.values()):
            result["terrain_sections"] = sections
    return result


def stationary_pacing(summary):
    coverage = summary["coverage"]
    elapsed = coverage["elapsed_ms"]
    paired_interval = elapsed / coverage["paired_frames"]
    frame_series = summary["frame_series"]
    return {
        "wall_ms": elapsed,
        "paired_submissions": coverage["paired_frames"],
        "paired_submissions_per_s": 1000 / paired_interval,
        "elapsed_per_pair_ms": paired_interval,
        "eligible_wall_percent": 100 * coverage["eligible_wall_fraction"],
        "eligible_intervals": coverage["eligible_intervals"],
        "eligible_intervals_per_s": (
            1000 * coverage["eligible_intervals"] / coverage["eligible_interval_ms"]
        ),
        "p50_ms": frame_series["p50_ms"],
        "p95_ms": frame_series["p95_ms"],
        "p99_ms": frame_series["p99_ms"],
        "gap_to_30_hz_ms": paired_interval - 1000 / 30,
    }


def settled_sections(summary):
    sections = summary.get("terrain_sections", {})
    return (
        len(sections) == 4
        and all(value is not None and value > 0 and int(value) == value for value in sections.values())
        and all(sections[f"{output}_before"] == sections[f"{output}_after"] for output in ("tv", "gamepad"))
    )


def compare_stationary(control, trial):
    """Compare two stationaries with equal settings and settled workload."""
    first, second = map(summarize, (control, trial))
    if first["scenario"] != "cafe-stationary" or second["scenario"] != first["scenario"]:
        raise ValueError("comparison requires two stationary GX2 reports")
    if control == trial:
        raise ValueError("comparison requires two distinct captures")
    for name, summary in (("control", first), ("trial", second)):
        if not summary["coverage"].get("meets_stationary_coverage"):
            raise ValueError(f"{name} did not meet the stationary coverage gate")
        targets = summary["targets"]
        if (
            any(value is None or int(value) != value for value in targets.values())
            or any(not value for key, value in targets.items() if key != "world_surface_bytes")
            or targets["configured_scale"] != targets["effective_scale"]
        ):
            raise ValueError(f"{name} requires complete fixed-scale target evidence")
        if not settled_sections(summary):
            raise ValueError(f"{name} needs stable nonzero section counts on both displays")
        if "camera" not in summary:
            raise ValueError(f"{name} requires recorded camera evidence")
        if not all(key in summary["frame_series"] for key in ("p50_ms", "p95_ms", "p99_ms")):
            raise ValueError(f"{name} requires the frame pacing percentiles")
    for key in ("app", "hardware", "conditions"):
        if not isinstance(control.get(key), dict) or not control[key] or control[key] != trial.get(key):
            raise ValueError(f"mismatched or missing {key}")
    for key in ("targets", "camera", "terrain_sections"):
        if first[key] != second[key]:
            raise ValueError(f"mismatched {key}")
    for key in ("cafe.entity_distance_percent", "cafe.omitted_pass_mask"):
        a, b = (metric(report["metrics"], key) for report in (control, trial))
        if a is None or a != b:
            raise ValueError(f"mismatched or missing {key}")

    control_pacing, trial_pacing = map(stationary_pacing, (first, second))
    return {
        "conditions": first["conditions"],
        "targets": first["targets"],
        "camera": first["camera"],
        "terrain_sections": first["terrain_sections"],
        "control": {name: round(value, 3) for name, value in control_pacing.items()},
        "trial": {name: round(value, 3) for name, value in trial_pacing.items()},
        "trial_minus_control": {
            name: round(trial_pacing[name] - value, 3)
            for name, value in control_pacing.items()
        },
    }


def load_report(path):
    with path.open(encoding="utf-8-sig") as file:
        return json.load(file)


def village_window(frames, coverage, first_sequence=None):
    count = int(coverage["host_frames"])
    if count <= 0:
        raise ValueError("Village profile correlation requires measured host frames")
    matched = None
    for start in range(len(frames) - count + 1):
        if first_sequence is not None and frames[start]["sequence"] != first_sequence:
            continue
        rows = frames[start:start + count]
        if any(row["phase"] != "playing" for row in rows):
            continue
        paired = [row["tv_submissions"] == row["gamepad_submissions"] == 1 for row in rows]
        if sum(paired) != coverage["paired_frames"]:
            continue
        eligible = [pair and min(row["tv_sections_drawn"], row["drc_sections_drawn"]) > 0
                    for pair, row in zip(paired, rows)]
        if sum(eligible) != coverage["terrain_paired_frames"]:
            continue
        intervals = []
        gaps = []
        start_slack = end_slack = 0.0
        for terrain, group in groupby(zip(eligible, rows), key=lambda pair: pair[0]):
            run = [row for _, row in group]
            if not terrain:
                gaps.append(run)
            elif len(run) > 1:
                intervals.extend(b["start_ms"] - a["start_ms"] for a, b in zip(run, run[1:]))
                # The Game starts inside each host frame. Over a continuous
                # run the offset cancels except at the first and last frame.
                start_slack += run[0]["loop_ms"]
                end_slack += run[-1]["loop_ms"]
        elapsed = sum(intervals)
        reported_ms = coverage["eligible_interval_ms"]
        rounding_ms = 0.001
        if (len(intervals) == coverage["eligible_intervals"]
                and max(map(len, gaps), default=0) == coverage["longest_ineligible_run_frames"]
                and elapsed - start_slack - rounding_ms <= reported_ms
                <= elapsed + end_slack + rounding_ms):
            if matched is not None:
                raise ValueError("multiple Village windows match. Use --profile-first-frame")
            matched = rows, intervals, gaps
    if matched is None:
        raise ValueError("no retained gameplay window matches the Village frame coverage")
    return matched


def village_streaming_samples(rows, lines):
    frames = {row["sequence"]: row for row in rows}
    samples = {}
    earliest_flight_start = -float_info.max
    latest_flight_start = float_info.max
    for line in lines:
        match = VILLAGE_TRACE.search(line)
        if not match:
            continue
        seq, elapsed, reason, x, z, tv, drc, columns, client, uploads, remote, resident, lit = match.groups()
        sequence = int(seq)
        row = frames.get(sequence)
        if row is None:
            continue
        sections = [int(tv), int(drc)]
        if sections != [row["tv_sections_drawn"], row["drc_sections_drawn"]]:
            raise ValueError(f"Village serial and HZP3 sections disagree at frame {sequence}")
        eligible = row["tv_submissions"] == row["gamepad_submissions"] == 1 and min(sections) > 0
        if reason != "progress" and eligible != (reason == "recovered"):
            raise ValueError(f"Village trace reason disagrees with HZP3 coverage at frame {sequence}")
        elapsed_ms = int(elapsed)
        # The trace observation is inside its host row, with elapsed rounded down to ms.
        # All matching observations must admit the same flight start on the HZP clock.
        earliest_flight_start = max(earliest_flight_start, row["start_ms"] - elapsed_ms - 1)
        latest_flight_start = min(latest_flight_start, row["start_ms"] + row["loop_ms"] - elapsed_ms)
        sample = {
            "sequence": sequence,
            "elapsed_ms": elapsed_ms,
            "reason": reason,
            "camera_xz": [float(x), float(z)],
            "drawn_sections": sections,
            "columns": int(columns),
            "client_pending": int(client),
            "uploads": int(uploads),
            "remote_pending": int(remote),
            "forward_resident_mask": int(resident, 16),
            "forward_lit_mask": int(lit, 16),
        }
        if samples.setdefault(sequence, sample) != sample:
            raise ValueError(f"conflicting Village serial reports for frame {sequence}")
    if earliest_flight_start > latest_flight_start + 0.001:
        raise ValueError("Village serial timing does not match the HZP3 frame clock")
    return [samples[seq] for seq in sorted(samples)]


def village_profile(summary, frames, first_sequence=None, serial=()):
    if summary["scenario"] != "village" or summary["coverage"]["evidence"] == "missing":
        raise ValueError("profile correlation requires a stock Village coverage report")
    rows, intervals, gaps = village_window(frames, summary["coverage"], first_sequence)
    start_ms = rows[0]["start_ms"]
    tested = ("tv_sections_tested", "drc_sections_tested")
    return {
        "first_sequence": rows[0]["sequence"],
        "last_sequence": rows[-1]["sequence"],
        "first_to_last_start_ms": round(rows[-1]["start_ms"] - start_ms, 3),
        "eligible_ms_from_profile_starts": round(sum(intervals), 3),
        "ineligible_runs": [{
            "first_sequence": run[0]["sequence"],
            "last_sequence": run[-1]["sequence"],
            "start_offset_ms": round(run[0]["start_ms"] - start_ms, 3),
            "frames": len(run),
            "sections_tested_at_start": [run[0][field] for field in tested],
            "sections_tested_at_end": [run[-1][field] for field in tested],
            "sections_tested_tv_and_gamepad": [
                gx2_profile.stats([row[field] for row in run]) for field in tested
            ],
            "terrain_upload_vertex_bytes": sum(row["terrain_upload_bytes"] for row in run),
        } for run in gaps],
        "forward_streaming_samples": village_streaming_samples(rows, serial),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="a saved Village or stationary benchmark JSON")
    parser.add_argument("--against", type=Path, metavar="TRIAL",
                        help="compare a control stationary report with a separate trial")
    parser.add_argument("--profile", type=Path, metavar="HZP3",
                        help="correlate a stock Village report with its native capture")
    parser.add_argument("--profile-first-frame", type=int,
                        help="identify the first Village frame when coverage alone is ambiguous")
    parser.add_argument("--serial-capture", type=Path, help="join Village streaming samples from UWP serial")
    parser.add_argument("--serial-first-seq", type=int)
    parser.add_argument("--serial-last-seq", type=int)
    args = parser.parse_args()
    first, last = args.serial_first_seq, args.serial_last_seq
    if args.profile is not None and args.against is not None:
        parser.error("--profile and --against are mutually exclusive")
    if args.profile is None and (args.profile_first_frame is not None or args.serial_capture is not None):
        parser.error("--profile is required for frame selection or serial correlation")
    if args.serial_capture is not None or first is not None or last is not None:
        if args.serial_capture is None or first is None or last is None or not 1 <= first <= last:
            parser.error("specify --serial-capture with both serial bounds in increasing order")
    try:
        report = load_report(args.report)
        if args.against:
            result = compare_stationary(report, load_report(args.against))
        else:
            result = summarize(report)
            if args.profile is not None:
                metadata, frames = gx2_profile.read(args.profile.read_bytes())
                if metadata["version"] != 3:
                    raise ValueError("Village profile correlation requires HZP3")
                lines = () if args.serial_capture is None else gx2_profile.uwp_serial_lines(
                    args.serial_capture, first, last,
                )
                result["village_profile"] = village_profile(result, frames, args.profile_first_frame, lines)
        print(json.dumps(result, indent=2, allow_nan=False))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    main()
