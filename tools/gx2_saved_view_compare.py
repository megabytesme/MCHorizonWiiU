#!/usr/bin/env python3
"""Compare explicit, retained 60-second Wii U saved-view capture windows."""

import argparse
from hashlib import sha256
import json
import math
from pathlib import Path
from statistics import median

from gx2_profile import read, summarize_flip_quantization, summarize_stock_saved_view


def retained_window(path, first, last, refresh_hz=None):
    data = path.read_bytes()
    metadata, frames = read(data)
    if metadata["version"] != 3 or not frames:
        raise ValueError(f"{path} must contain an HZP3 capture")
    if not frames[0]["sequence"] <= first < last <= frames[-1]["sequence"]:
        raise ValueError(f"{path} does not retain both requested sequence bounds")
    origin = frames[0]["sequence"]
    rows = frames[first - origin:last - origin + 1]
    report = summarize_stock_saved_view(rows, -math.inf, math.inf)
    if not report["at_least_60_seconds_retained"]:
        raise ValueError(f"{path} must retain at least 60 seconds between the requested bounds")
    if len(report["reference_dimensions"]) != 1:
        raise ValueError(f"{path} needs qualifying paired geometry at one set of target dimensions")
    if refresh_hz is not None:
        if not 1 <= refresh_hz <= metadata["clock_hz"]:
            raise ValueError(f"{path} needs a refresh rate within the capture clock frequency")
        flips = summarize_flip_quantization(rows, -math.inf, math.inf, refresh_hz, terrain_sections=174)
        dimensions = report["reference_dimensions"][0]["world_tv_gamepad"]
        flips["cohorts"] = [
            cohort for cohort in flips["cohorts"]
            if cohort["requested_and_effective_scale"] == 75
            and cohort["world_dimensions_tv_gamepad"] == dimensions
        ]
        report["reference_combined_flip_quantization"] = flips
    return {"capture": str(path), "capture_sha256": sha256(data).hexdigest(), **report}


def compare(control, trial):
    windows = [*control, *trial]
    dimensions = windows[0]["reference_dimensions"][0]
    seen = {}
    for index, window in enumerate(windows):
        if window["reference_dimensions"][0] != dimensions:
            raise ValueError("the selected runs have different saved-view target dimensions")
        intervals = seen.setdefault(window["capture_sha256"], [])
        first, last = window["first_sequence"], window["last_sequence"]
        if any(first <= prior_last and last >= prior_first for prior_first, prior_last in intervals):
            raise ValueError("the selected runs overlap within the same capture")
        intervals.append((first, last))

    aggregates = []
    for arm in (control, trial):
        rates = [run["reference_pairs_per_second"] for run in arm]
        complete = [run["reference_complete_pairs_per_second"] for run in arm]
        aggregates.append({
            "windows": len(arm),
            "median_pairs_per_second": median(rates),
            "median_complete_pairs_per_second": median(complete),
            "minimum_complete_pairs_per_second": min(complete),
            "longest_complete_pair_gap_ms": max(
                run["reference_max_gap_between_complete_pairs_or_window_bounds_ms"] for run in arm
            ),
        })
    baseline, experiment = aggregates
    return {
        "dimensions": dimensions,
        "control": {"runs": control, **baseline},
        "trial": {"runs": trial, **experiment},
        "median_pairs_per_second_difference": round(
            experiment["median_pairs_per_second"] - baseline["median_pairs_per_second"], 3,
        ),
        "median_complete_pairs_per_second_difference": round(
            experiment["median_complete_pairs_per_second"] - baseline["median_complete_pairs_per_second"], 3,
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arm in ("control", "trial"):
        parser.add_argument(f"--{arm}", action="append", required=True, nargs=3,
                            metavar=("HZP3", "FIRST_SEQ", "LAST_SEQ"),
                            help="one inclusive window, repeated for additional runs")
    parser.add_argument("--flip-refresh-hz", type=float,
                        help="include combined driver flips at the measured refresh rate")
    args = parser.parse_args()
    try:
        control, trial = ([retained_window(Path(path), int(first), int(last), args.flip_refresh_hz)
                           for path, first, last in arm]
                          for arm in (args.control, args.trial))
        report = compare(control, trial)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
