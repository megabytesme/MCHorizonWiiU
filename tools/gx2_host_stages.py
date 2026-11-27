#!/usr/bin/env python3
"""Summarize CPU-side timing and synchronization API wall time from HZP3 gameplay."""

import argparse
from collections import Counter
from hashlib import sha256
import json
from pathlib import Path
from statistics import fmean

from gx2_profile import WORKERS, read, stats


def distribution(values):
    values = list(values)
    result = stats(values)
    return None if result is None else {**result, "mean": round(fmean(values), 3)}


def explicit_sync(row):
    return row["wait_flip_ms"] + row["finish_present_ms"] + row["terrain_wait_ms"]


def submit(row):
    return row["submit_tv_ms"] + row["submit_drc_ms"]


def hud_submit(row):
    return row["hud_submit_tv_ms"] + row["hud_submit_drc_ms"]


def work(row):
    if row["presented"] == row["tv_submissions"] == row["gamepad_submissions"] == 1:
        if row["tv_sections_drawn"] > 0 and row["drc_sections_drawn"] > 0:
            return "paired_with_terrain"
        return "paired_missing_terrain"
    return "without_one_pair"


def summarize_cohort(rows, observed_client_spans, observed_workers):
    sizes = Counter(
        (row["world_requested_scale"], row["world_effective_scale"],
         row["tv_world_width"], row["tv_world_height"], row["drc_world_width"],
         row["drc_world_height"])
        for row in rows
    )
    nearby = (
        current["start_ms"] - previous["start_ms"]
        for previous, current in zip(rows, rows[1:])
        if current["sequence"] == previous["sequence"] + 1
    )
    metrics = {
        "host_loop": lambda row: row["loop_ms"],
        "explicit_sync_api_wall": explicit_sync,
        "previous_flip_wait": lambda row: row["wait_flip_ms"],
        "presentation_finish": lambda row: row["finish_present_ms"],
        "terrain_wait": lambda row: row["terrain_wait_ms"],
        "graphics_submit_including_hud": submit,
        "hud_submit_including_internal_sync": hud_submit,
        "graphics_submit_outside_hud": lambda row: max(0.0, submit(row) - hud_submit(row)),
        "mesh_upload_including_allocator": lambda row: row["terrain_upload_ms"],
        "texture_upload": lambda row: row["texture_upload_ms"],
        "artwork": lambda row: row["stage_artwork_ms"],
    }
    timings = {key: distribution(getter(row) for row in rows) for key, getter in metrics.items()}
    for stage in ("world_update", "streaming", "tick"):
        timings[stage] = (
            distribution(row[f"{stage}_ms"] for row in rows) if stage in observed_client_spans else None
        )
    uploading = [row for row in rows if row["terrain_upload_bytes"] or row["terrain_upload_ms"]]
    largest = max(rows, key=lambda row: row["loop_ms"])
    return {
        "loops": len(rows),
        "partial_content_loops": sum(row["partial"] != 0 for row in rows),
        "loops_over_60hz_budget": sum(row["loop_ms"] > 1000 / 60 for row in rows),
        "adjacent_frame_start_interval_ms": distribution(nearby),
        "spans_ms": timings,
        "largest_host_loop": {
            "sequence": largest["sequence"],
            "ms": round(largest["loop_ms"], 3),
            "next_cohort_frame_is_adjacent": any(
                row["sequence"] == largest["sequence"] + 1 for row in rows
            ),
        },
        "mesh_upload": {
            "active_loops": len(uploading),
            "vertex_bytes": sum(row["terrain_upload_bytes"] for row in uploading),
            "active_loop_ms": distribution(row["terrain_upload_ms"] for row in uploading),
        },
        "parallel_worker_completions": {
            worker: {
                "available_intervals": sum(row[f"{worker}_ms"] is not None for row in rows),
                "jobs": sum(row[f"{worker}_count"] or 0 for row in rows),
                "completed_wall_ms": round(sum(row[f"{worker}_ms"] or 0 for row in rows), 3),
            } if worker in observed_workers else None
            for worker in WORKERS
        },
        "resolution_cohorts": [
            {"requested_percent": requested, "effective_percent": effective,
             "tv_world": [tv_w, tv_h], "gamepad_world": [pad_w, pad_h], "loops": count}
            for (requested, effective, tv_w, tv_h, pad_w, pad_h), count in sorted(sizes.items())
        ],
    }


def analyze(frames, first, last):
    if not frames or not frames[0]["sequence"] <= first <= last <= frames[-1]["sequence"]:
        raise ValueError("sequence interval must lie entirely inside the retained capture")
    selected = [row for row in frames if first <= row["sequence"] <= last]
    usable = [row for row in selected if row["phase"] == "playing" and not row["screenshot_ms"]]
    if not usable:
        raise ValueError("sequence interval contains no screenshot-free gameplay rows")
    for row in usable:
        for output in ("tv", "drc"):
            if row[f"hud_submit_{output}_ms"] > row[f"submit_{output}_ms"] + 1e-6:
                raise ValueError(f"HUD span exceeds its enclosing {output} submit at {row['sequence']}")
    all_gameplay = [row for row in frames if row["phase"] == "playing"]
    client_spans = [
        stage for stage in ("world_update", "streaming", "tick")
        if any(row[f"{stage}_ms"] > 0 for row in all_gameplay)
    ]
    observed_workers = [
        worker for worker in WORKERS
        if any(row[f"{worker}_count"] or row[f"{worker}_ms"] for row in all_gameplay)
    ]
    groups = {"all_gameplay": usable}
    for row in usable:
        groups.setdefault(work(row), []).append(row)
    return {
        "sequence_first": first,
        "sequence_last": last,
        "selected_loops": len(selected),
        "excluded_other_phase": sum(row["phase"] != "playing" for row in selected),
        "excluded_screenshot": sum(row["phase"] == "playing" and bool(row["screenshot_ms"])
                                   for row in selected),
        "client_update_spans_observed": client_spans,
        "worker_spans_observed": observed_workers,
        "cohorts": {
            name: summarize_cohort(rows, client_spans, observed_workers) for name, rows in groups.items()
        },
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--first-sequence", type=int, help="inclusive retained HZP3 sequence")
    parser.add_argument("--last-sequence", type=int, help="inclusive retained HZP3 sequence")
    args = parser.parse_args(argv)
    try:
        data = args.capture.read_bytes()
        metadata, frames = read(data)
        if metadata["version"] != 3 or not frames:
            raise ValueError("analysis requires a nonempty HZP3 capture")
        first = frames[0]["sequence"] if args.first_sequence is None else args.first_sequence
        last = frames[-1]["sequence"] if args.last_sequence is None else args.last_sequence
        report = analyze(frames, first, last)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps({**metadata, "capture_sha256": sha256(data).hexdigest(), **report,
                      "accounting": [
                          "Spans measure host wall time and overlap. Stream service is nested in world update.",
                          "HUD submit is nested in submit. Its internal synchronization has no separate HZP3 span.",
                          "Explicit waits may overlap submission or world update. Presentation finish also issues graphics calls.",
                          "Allocator time is included in mesh upload, which also includes conversion and GPU transfer setup.",
                          "Worker spans overlap host time and one another, and are attributed to the completion interval.",
                          "Unobserved client and worker spans are reported as null rather than measured zero.",
                          "A loop duration belongs to the loop start. Its full duration may cross the sequence boundary.",
                      ]}, indent=2))


if __name__ == "__main__":
    main()
