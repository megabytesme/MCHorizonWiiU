#!/usr/bin/env python3
"""Join sampled GX2 queue retirement records to their HZP3 frame starts."""

import argparse
from collections import Counter
from hashlib import sha256
import json
from pathlib import Path
import re

import gx2_profile as profile


RETIREMENT = re.compile(
    r"Horizon GX2 grouped queue retirement seq=(\d{1,10}) start_ticks=(\d{1,20}) class=([a-z-]+) "
    r"before_flush=(-?\d{1,19}) submitted=(-?\d{1,19}) retired_at_flush=(-?\d{1,19}) "
    r"submitted_after_draw_done=(-?\d{1,19}) retired_after_draw_done=(-?\d{1,19}) "
    r"draw_done=([01]) paired_accept=([01])\b"
)
TOKEN_NAMES = (
    "before_flush", "submitted", "retired_at_flush", "submitted_after_draw_done", "retired_after_draw_done",
)


def classification(before, submitted, retired, submitted_after, retired_after, drawn, accepted):
    if not drawn:
        return "invalid-draw-done"
    if before <= 0 or submitted <= before:
        return "invalid-flush-token"
    if not 0 <= retired <= submitted or submitted_after < submitted or retired_after != submitted_after:
        return "invalid-retirement"
    if not accepted:
        return "invalid-pair-rejected"
    return "pending-after-flush" if retired < submitted else "retired-after-flush"


def analyze(data, serial):
    header, frames = profile.read(data)
    if header["version"] != 3:
        raise ValueError("queue retirement join requires HZP3")
    raw_ticks = profile.hzp3_sequence_ticks(data, frames)
    by_sequence = {frame["sequence"]: frame for frame in frames}
    records, outside, mismatched = {}, set(), set()
    for line in serial:
        for match in RETIREMENT.finditer(line):
            seq, start = map(int, match.groups()[:2])
            if seq not in raw_ticks:
                outside.add((seq, start))
                continue
            if start != raw_ticks[seq]:
                mismatched.add((seq, start))
                continue
            values = tuple(map(int, match.groups()[3:]))
            if seq % 120 or any(not -(1 << 63) <= value < (1 << 63) for value in values):
                raise ValueError(f"invalid retirement record at sequence {seq}")
            kind = classification(*values)
            if kind != match[3]:
                raise ValueError(f"retirement classification disagrees with tokens at sequence {seq}")
            if records.setdefault(seq, (kind, values)) != (kind, values):
                raise ValueError(f"conflicting retirement records at sequence {seq}")

    pairs, excluded, counts = [], [], Counter()
    for seq, (kind, values) in sorted(records.items()):
        frame = by_sequence[seq]
        paired = (frame["phase"] == "playing" and not frame["screenshot_ms"]
                  and frame["game_frames_built"] == frame["presented"]
                  == frame["tv_submissions"] == frame["gamepad_submissions"] == 1)
        if not paired and kind in ("pending-after-flush", "retired-after-flush"):
            excluded.append(seq)
            continue
        counts[kind] += 1
        pairs.append({
            "sequence": seq, "start_ticks": raw_ticks[seq], "class": kind,
            "tokens": dict(zip(TOKEN_NAMES, values[:5])),
            "draw_done": bool(values[5]), "accepted": bool(values[6]), "hzp_paired": paired,
            "partial": bool(frame["partial"]), "world_scale": frame["world_effective_scale"],
            "sections_tv_gamepad": [frame["tv_sections_drawn"], frame["drc_sections_drawn"]],
        })
    return {"capture_sha256": sha256(data).hexdigest(), "records": pairs, "counts": dict(counts),
            "outside_retained": len(outside), "tick_mismatch": sorted({seq for seq, _ in mismatched}),
            "excluded_unpaired_sequences": excluded}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
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
        data = args.capture.read_bytes()
        if args.serial_capture:
            result = analyze(data, profile.uwp_serial_lines(args.serial, first, last))
        else:
            with args.serial.open(encoding="utf-8-sig", errors="replace") as serial:
                result = analyze(data, profile.plain_serial_lines(serial))
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
