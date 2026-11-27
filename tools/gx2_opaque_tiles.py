#!/usr/bin/env python3
"""Summarize matched TV and GamePad opaque-tile observations from one run."""

import argparse
import json
from pathlib import Path
import re

import gx2_profile as profile


OBSERVATION = re.compile(
    r"Horizon GX2 opaque tiles pair=(\d{1,10}) output=([01]) "
    r"wall_us=(\d{1,10}) faces=(\d{1,10}) tiles=(\d{1,10}) "
    r"sections=(\d{1,10}) hidden=(\d{1,10}) indices=(\d{1,20}) "
    r"hidden_indices=(\d{1,20}) skipped_uploads=(\d{1,10})\b"
)
INCOMPLETE = re.compile(r"Horizon GX2 opaque tiles pair=(\d{1,10}) incomplete=work-or-projection-limit\b")
FIELDS = (
    "wall_us", "projected_faces", "covered_tiles", "solid_sections",
    "covered_sections", "solid_indices", "covered_indices", "skipped_uploads",
)


def analyze(lines):
    observations, limited = {}, set()
    for line in lines:
        for match in INCOMPLETE.finditer(line):
            limited.add(int(match[1]))
        for match in OBSERVATION.finditer(line):
            pair, output, *counts = map(int, match.groups())
            _, faces, tiles, sections, hidden, indices, hidden_indices, _ = counts
            if (faces > 8192 or tiles > 64 * 36 or sections > 2048 or hidden > sections
                    or hidden_indices > indices or (not hidden and hidden_indices)):
                raise ValueError(f"opaque tile counts outside the diagnostic bounds for pair {pair}")
            views = observations.setdefault(pair, {})
            record = dict(zip(FIELDS, counts))
            if views.setdefault(output, record) != record:
                raise ValueError(f"conflicting opaque tile records for pair {pair}, output {output}")

    if limited & observations.keys():
        raise ValueError("an opaque tile pair reports both a limit and numerical results")
    paired, missing = [], []
    for pair, views in sorted(observations.items()):
        if len(views) != 2:
            missing.append(pair)
            continue
        tv, gamepad = views[0], views[1]
        if tv["skipped_uploads"] != gamepad["skipped_uploads"]:
            raise ValueError(f"opaque tile records disagree on upload omissions for pair {pair}")
        paired.append({"eligible_pair_ordinal": pair, "tv": tv, "gamepad": gamepad})
    summary = {}
    for display in ("tv", "gamepad"):
        views = [pair[display] for pair in paired]
        candidates = sum(view["solid_indices"] for view in views)
        covered = sum(view["covered_indices"] for view in views)
        summary[display] = {
            "solid_indices": candidates,
            "covered_indices": covered,
            "covered_fraction": covered / candidates if candidates else None,
            "wall_us": profile.stats(view["wall_us"] for view in views),
        }
    return {"paired_samples": len(paired), "incomplete_pair_ordinals": sorted(limited),
            "missing_output_ordinals": missing, "displays": summary, "samples": paired}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
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
        if args.serial_capture:
            result = analyze(profile.uwp_serial_lines(args.serial, first, last))
        else:
            with args.serial.open(encoding="utf-8-sig", errors="replace") as serial:
                result = analyze(profile.plain_serial_lines(serial))
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
