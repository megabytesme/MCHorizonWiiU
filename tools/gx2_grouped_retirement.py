#!/usr/bin/env python3
"""Join sampled GX2 queue retirement records to their HZP3 frame starts."""

import argparse
from collections import Counter, defaultdict
from hashlib import sha256
import json
from pathlib import Path
import re
from statistics import fmean

import gx2_profile as profile


RETIREMENT_PREFIX = "Horizon GX2 grouped queue retirement "
OSCONSOLE = re.compile(r"\[([0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3})\] \[OSConsole\] (.*)")
OSCONSOLE_CHUNK = 270
RETIREMENT = re.compile(
    RETIREMENT_PREFIX + r"seq=(\d{1,10}) start_ticks=(\d{1,20}) class=([a-z-]+) "
    r"before_flush=(-?\d{1,19}) submitted=(-?\d{1,19}) retired_at_flush=(-?\d{1,19}) "
    r"submitted_after_draw_done=(-?\d{1,19}) retired_after_draw_done=(-?\d{1,19}) "
    r"draw_done=([01]) paired_accept=([01])\b"
)
TOKEN_NAMES = (
    "before_flush", "submitted", "retired_at_flush", "submitted_after_draw_done", "retired_after_draw_done",
)


def retirement_matches(serial):
    pending = None
    for line in serial:
        message = OSCONSOLE.fullmatch(line.rstrip("\r\n"))
        may_start = pending is None
        if pending is not None:
            timestamp, head = pending
            pending = None
            if message and message[1] == timestamp and len(message[2]) <= OSCONSOLE_CHUNK:
                match = RETIREMENT.fullmatch(head + message[2])
                if match:
                    yield match
                    continue
        yield from RETIREMENT.finditer(line)
        # Cemu splits these ASCII OSConsole payloads at 270 bytes in the
        # retained log. Only the immediately following chunk can finish one.
        # Overlapping incomplete headers leave no reliable continuation.
        if may_start and message:
            timestamp, body = message.groups()
            if (len(body) == OSCONSOLE_CHUNK and body.isascii()
                    and body.startswith(RETIREMENT_PREFIX) and not RETIREMENT.fullmatch(body)):
                pending = (timestamp, body)


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


def finish_workload(frame):
    scale = frame["world_effective_scale"]
    if scale <= 0 or scale != frame["world_requested_scale"]:
        return None
    sections = (frame["tv_sections_drawn"], frame["drc_sections_drawn"])
    outputs = tuple(frame[f"{target}_{axis}"]
                    for target in ("tv", "drc") for axis in ("width", "height"))
    worlds = tuple(frame[f"{target}_world_{axis}"]
                   for target in ("tv", "drc") for axis in ("width", "height"))
    if min(*sections, *outputs, *worlds) <= 0:
        return None
    return scale, sections, outputs, worlds, frame["indices"], bool(frame["partial"])


def matched_finish_comparisons(cohorts):
    # The HZP3 stage includes swap, flush, wait and display enable. It does
    # not isolate GX2DrawDone or measure an alternative implementation.
    comparisons = []
    for key, (ready, pending) in sorted(cohorts.items()):
        if not ready or not pending:
            continue
        scale, sections, outputs, worlds, indices, partial = key
        ready_ms, pending_ms = fmean(ready), fmean(pending)
        comparisons.append({
            "world_scale": scale, "sections_tv_gamepad": list(sections),
            "output_dimensions_tv_gamepad": [list(outputs[:2]), list(outputs[2:])],
            "world_dimensions_tv_gamepad": [list(worlds[:2]), list(worlds[2:])],
            "indices": indices, "partial": partial,
            "retired_unchanged_samples": len(ready), "pending_samples": len(pending),
            "retired_unchanged_finish_mean_ms": ready_ms,
            "pending_finish_mean_ms": pending_ms,
            "pending_minus_retired_finish_mean_ms": pending_ms - ready_ms,
        })
    return comparisons


def analyze(data, serial):
    header, frames = profile.read(data)
    if header["version"] != 3:
        raise ValueError("queue retirement join requires HZP3")
    raw_ticks = profile.hzp3_sequence_ticks(data, frames)
    by_sequence = {frame["sequence"]: frame for frame in frames}
    records, outside, mismatched = {}, set(), set()
    for match in retirement_matches(serial):
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
    unchanged_count = 0
    finish_cohorts = defaultdict(lambda: ([], []))
    for seq, (kind, values) in sorted(records.items()):
        frame = by_sequence[seq]
        paired = (frame["phase"] == "playing" and not frame["screenshot_ms"]
                  and frame["game_frames_built"] == frame["presented"]
                  == frame["tv_submissions"] == frame["gamepad_submissions"] == 1)
        if not paired and kind in ("pending-after-flush", "retired-after-flush"):
            excluded.append(seq)
            continue
        counts[kind] += 1
        # Classification already requires a fresh, accepted flush token.
        # Compare all four endpoints to detect an advance during DrawDone.
        unchanged = (paired and kind == "retired-after-flush"
                     and values[1] == values[2] == values[3] == values[4])
        unchanged_count += unchanged
        finish_ms = frame["finish_present_ms"]
        if paired and (unchanged or kind == "pending-after-flush"):
            workload = finish_workload(frame)
            if workload is not None:
                ready, pending = finish_cohorts[workload]
                (ready if unchanged else pending).append(finish_ms)
        pairs.append({
            "sequence": seq, "start_ticks": raw_ticks[seq], "class": kind,
            "tokens": dict(zip(TOKEN_NAMES, values[:5])),
            "draw_done": bool(values[5]), "accepted": bool(values[6]), "hzp_paired": paired,
            "partial": bool(frame["partial"]), "world_scale": frame["world_effective_scale"],
            "sections_tv_gamepad": [frame["tv_sections_drawn"], frame["drc_sections_drawn"]],
            "retired_unchanged_during_draw_done": unchanged, "finish_present_ms": finish_ms,
        })
    return {"capture_sha256": sha256(data).hexdigest(), "records": pairs, "counts": dict(counts),
            "retired_unchanged_during_draw_done": unchanged_count,
            "matched_finish_comparisons": matched_finish_comparisons(finish_cohorts),
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
