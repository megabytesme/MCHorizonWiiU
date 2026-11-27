"""Compare fixed-scale saved-view windows without qualifying a lower-scale result as stock."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
from statistics import median

from gx2_saved_view_compare import retained_window


def compare(windows):
    if not windows:
        raise ValueError("provide the fixed-75 reference and at least one other fixed scale")
    by_scale = defaultdict(list)
    seen = defaultdict(list)
    scanouts = windows[0][1]["reference_dimensions"][0]["outputs_tv_gamepad"]
    for scale, window in windows:
        shape = window["reference_dimensions"][0]
        if shape["outputs_tv_gamepad"] != scanouts:
            raise ValueError("display dimensions changed between fixed-scale windows")
        indices = seen[window["capture_sha256"]]
        first, last = window["first_sequence"], window["last_sequence"]
        if any(first <= prior_last and last >= prior_first for prior_first, prior_last in indices):
            raise ValueError("fixed-scale windows overlap within the same capture")
        indices.append((first, last))
        by_scale[scale].append(window)
    if 75 not in by_scale or len(by_scale) < 2:
        raise ValueError("provide the fixed-75 reference and at least one other fixed scale")

    scales = {}
    for scale, group in sorted(by_scale.items(), reverse=True):
        targets = group[0]["reference_dimensions"][0]["world_tv_gamepad"]
        if any(row["reference_dimensions"][0]["world_tv_gamepad"] != targets for row in group):
            raise ValueError(f"world targets changed between windows at fixed scale {scale}")
        rates = [row["reference_pairs_per_second"] for row in group]
        complete_rates = [row["reference_complete_pairs_per_second"] for row in group]
        scales[scale] = {
            "windows": len(group), "world_dimensions_tv_gamepad": targets,
            "world_pixels_per_pair": sum(width * height for width, height in targets),
            "median_pairs_per_second": median(rates),
            "minimum_pairs_per_second": min(rates), "maximum_pairs_per_second": max(rates),
            "median_complete_pairs_per_second": median(complete_rates),
            "median_pair_interval_ms": median(row["reference_pair_intervals_ms"]["median"] for row in group),
            "worst_window_p99_pair_interval_ms": max(row["reference_pair_intervals_ms"]["p99"] for row in group),
            "reported_median_indices_each_window": [row["reference_indices"]["median"] for row in group],
            "reported_median_section_draws_each_window": [row["reference_section_draws"]["median"] for row in group],
        }
    stock = scales[75]
    for scale, summary in scales.items():
        summary["world_pixel_ratio_to_fixed75"] = round(summary["world_pixels_per_pair"] / stock["world_pixels_per_pair"], 6)
        summary["paired_rate_change_from_fixed75"] = round(summary["median_pairs_per_second"] - stock["median_pairs_per_second"], 3)
        summary["eligible_scale_for_primary_stock_measurement"] = scale == 75
    return {
        "outputs_tv_gamepad": scanouts,
        "scales_in_capture_order": [scale for scale, _ in windows],
        "scale_summaries": scales,
        "runs": [{"scale": scale, **window} for scale, window in windows],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--window", action="append", required=True, nargs=4,
                        metavar=("SCALE", "HZP3", "FIRST_SEQ", "LAST_SEQ"),
                        help="one non-overlapping retained window, in capture order")
    parser.add_argument("--flip-refresh-hz", type=float, help="measured physical combined flip refresh rate")
    args = parser.parse_args()
    try:
        windows = []
        for scale, path, first, last in args.window:
            scale = int(scale)
            if not 1 <= scale <= 100:
                raise ValueError("expected a resolution percentage from 1 to 100")
            window = retained_window(Path(path), int(first), int(last), args.flip_refresh_hz,
                                     scale=scale, fixed_scale=True)
            if window["reference_pair_intervals_ms"] is None:
                raise ValueError(f"{path} needs at least two qualifying paired submissions")
            windows.append((scale, window))
        result = compare(windows)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
