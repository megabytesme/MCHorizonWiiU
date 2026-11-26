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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="a saved Village or stationary benchmark JSON")
    parser.add_argument("--against", type=Path, metavar="TRIAL",
                        help="compare a control stationary report with a separate trial")
    args = parser.parse_args()
    try:
        report = load_report(args.report)
        if args.against:
            result = compare_stationary(report, load_report(args.against))
        else:
            result = summarize(report)
        print(json.dumps(result, indent=2, allow_nan=False))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    main()
