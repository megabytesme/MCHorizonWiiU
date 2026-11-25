#!/usr/bin/env python3
"""Summarize recorded terrain coverage in hosted GX2 benchmark reports."""

import argparse
import json
from pathlib import Path
from sys import float_info


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


def number(value):
    if type(value) in (int, float) and 0 <= value <= float_info.max:
        return value
    raise ValueError("expected a finite nonnegative number")


def metric(metrics, key):
    if key not in metrics:
        return None
    summary = metrics[key]
    if not isinstance(summary, dict) or summary.get("n") != 1:
        raise ValueError(f"{key} needs one recorded value")
    try:
        return number(summary.get("mean"))
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
    return {
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="a saved Village or stationary benchmark JSON")
    args = parser.parse_args()
    try:
        with args.report.open(encoding="utf-8-sig") as file:
            print(json.dumps(summarize(json.load(file)), indent=2, allow_nan=False))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    main()
