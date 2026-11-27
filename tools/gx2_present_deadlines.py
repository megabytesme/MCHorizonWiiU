#!/usr/bin/env python3
"""Join sparse GX2 swap status observations to exact HZP3 gameplay rows."""
import argparse
from collections import Counter
from dataclasses import dataclass
import json
from pathlib import Path
import re

from gx2_profile import hzp3_sequence_ticks, read

EDGES = ("start", "waited", "ready", "posted", "finished", "next")
REQUIRED = frozenset(("start", "waited", "ready", "finished"))
SWAP_MASK = 0xffffffff
RECORD = re.compile(
    r"Horizon GX2 deadline seq=(\d+) start=([0-9a-f]{1,16}) mode=([01]) "
    r"edge=(start|waited|ready|posted|finished|next) cpu=([0-9a-f]{1,16}) "
    r"s=([0-9a-f]{1,8}) f=([0-9a-f]{1,8}) v=([0-9a-f]{1,16}) "
    r"l=([0-9a-f]{1,16}) q=([0-9a-f]{1,16}) r=([0-9a-f]{1,16})$"
)

TAIL_RECORD = re.compile(
    r"Horizon GX2 render tail seq=(\d+) start=([0-9a-f]{1,16}) state=([a-z-]+) hz=(\d+) "
    r"prior=([0-9a-f]{1,16}) q=([0-9a-f]{1,16}) r=([0-9a-f]{1,16}) "
    r"final=([0-9a-f]{1,16}) t0=([0-9a-f]{1,16}) lower=([0-9a-f]{1,16})$"
)
TAIL_STATES = frozenset((
    "no-early-flush", "unclean-prior", "incomplete", "no-render-token", "no-pre-scan-poll",
    "foreground-or-keyboard", "queue-changed", "bad-clock", "already-retired", "zero-interval",
    "finish-unverified", "lower-bound",
))


@dataclass(frozen=True)
class Point:
    cpu: int
    swaps: int
    flips: int
    vblank: int
    flip_time: int
    submitted: int
    retired: int

    @property
    def pending(self):
        return (self.swaps - self.flips) & SWAP_MASK


def parse(text):
    records = {}
    for line in text.splitlines():
        if "Horizon GX2 deadline seq=" not in line:
            continue
        match = RECORD.search(line)
        if not match:
            raise ValueError("malformed deadline record")
        seq, start, grouped, edge, *words = match.groups()
        key = (int(seq), int(start, 16), bool(int(grouped)))
        points = records.setdefault(key, {})
        if edge in points:
            raise ValueError("duplicate deadline edge")
        points[edge] = Point(*(int(word, 16) for word in words))
    return records


def eligible(row, scale, sections):
    return (row["phase"] == "playing" and row["presented"] == 1 and
            row["tv_submissions"] == row["gamepad_submissions"] == 1 and
            row["world_requested_scale"] == row["world_effective_scale"] == scale and
            not row["screenshot_ms"] and
            row["tv_sections_drawn"] == row["drc_sections_drawn"] == sections and
            all(row[key] > 0 for key in ("tv_world_width", "tv_world_height",
                                        "drc_world_width", "drc_world_height")))


def monotonic(points):
    if any(p.pending > 1 or p.retired > p.submitted or not p.vblank for p in points):
        return False
    def values(p):
        return p.cpu, p.vblank, p.flip_time, p.submitted, p.retired
    return all(
        all(right >= left for left, right in zip(values(a), values(b))) and
        ((b.swaps - a.swaps) & SWAP_MASK) <= 1 and ((b.flips - a.flips) & SWAP_MASK) <= 1
        for a, b in zip(points, points[1:])
    )


def same_queue(a, b):
    return (a.swaps, a.flips) == (b.swaps, b.flips)


def refresh_without_flip(a, b):
    return a.vblank < b.vblank and a.flip_time == b.flip_time and a.flips == b.flips


def flags(points):
    start, waited, ready, finished = (points[name] for name in ("start", "waited", "ready", "finished"))
    kinds = []
    if not ready.pending:
        for edge, point in (("start", start), ("waited", waited)):
            if same_queue(point, ready) and refresh_without_flip(point, ready):
                kinds.append("refresh_before_swap_from_" + edge)
                break
    posted = points.get("posted")
    if (posted and ((posted.swaps - ready.swaps) & SWAP_MASK) == 1 and posted.pending == 1 and
            posted.submitted > ready.submitted and posted.retired < posted.submitted and
            refresh_without_flip(ready, posted)):
        kinds.append("refresh_in_swap_bracket_with_gpu_pending")
    following = points.get("next")
    if (following and ((finished.swaps - ready.swaps) & SWAP_MASK) == 1 and finished.pending == 1 and
            finished.retired == finished.submitted > ready.submitted and
            same_queue(finished, following) and refresh_without_flip(finished, following)):
        kinds.append("refresh_with_retired_pair_still_unflipped")
    return kinds


def reduce(records, frames, starts, hz, *, scale, sections, first=None, last=None):
    data = {}
    discarded = Counter()
    counts = Counter()
    serial_clock_independent = False

    for (seq, start, grouped), original in sorted(records.items()):
        if (first is not None and seq < first) or (last is not None and seq > last):
            continue
        row = frames.get(seq)
        if row is None or start != starts.get(seq) or not eligible(row, scale, sections):
            discarded["unmatched_or_ineligible_hzp3"] += 1
            continue
        points = dict(original)
        if not REQUIRED <= points.keys() or ("posted" in points and not grouped):
            discarded["incomplete_or_misplaced_edges"] += 1
            continue
        if seq in data:
            raise ValueError("ambiguous deadline mode for joined HZP3 row")
        window_end = start + round(row["loop_ms"] * hz / 1000)
        if any(not start <= p.cpu <= window_end for edge, p in points.items() if edge != "next"):
            discarded["outside_hzp3_frame"] += 1
            continue
        if "next" in points:
            following = frames.get(seq+1)
            following_start = starts.get(seq+1)
            if ((last is not None and seq >= last) or not following or following_start is None
                    or not eligible(following, scale, sections) or not (
                        following_start <= points["next"].cpu <=
                        following_start + round(following["loop_ms"] * hz / 1000)
                    )):
                points.pop("next")
        ordered = [points[name] for name in EDGES if name in points]
        if not monotonic(ordered):
            discarded["invalid_snapshot_order_or_pending_depth"] += 1
            continue
        if any(refresh_without_flip(a, b) for a, b in zip(ordered, ordered[1:])):
            serial_clock_independent = True
        data[seq] = {"sequence": seq, "mode": "grouped" if grouped else "ordinary",
                     "flags": flags(points), "after_flush": "posted" in points, "follow": "next" in points,
                     "before_swap_ms": (points["ready"].cpu - points["start"].cpu) * 1000 / hz,
                     "finish_bracket_ms": (points["finished"].cpu - points["ready"].cpu) * 1000 / hz,
                     "wait_flip_ms": row["wait_flip_ms"], "finish_present_ms": row["finish_present_ms"]}
        counts.update(data[seq]["flags"])
    return {"joined": len(data), "independent_vblank_observed": serial_clock_independent,
            "refresh_observations": dict(counts), "discarded": dict(discarded), "frames": list(data.values())}


def render_tail(serial, deadlines, report, frames, hz):
    """Join render-only queue observations to accepted grouped deadline frames."""
    accepted = {row["sequence"] for row in report["frames"] if row["mode"] == "grouped"}
    states = Counter()
    discarded = Counter()
    bounds = []
    seen = set()
    for line in serial.splitlines():
        if "Horizon GX2 render tail seq=" not in line:
            continue
        match = TAIL_RECORD.search(line)
        if match is None:
            raise ValueError("malformed render-tail record")
        seq, start, state, clock, *numbers = match.groups()
        seq, start, clock = int(seq), int(start, 16), int(clock)
        prior, submitted, retired, final, began, lower = (int(value, 16) for value in numbers)
        if state not in TAIL_STATES or (state == "lower-bound") != bool(lower):
            raise ValueError("inconsistent render-tail disposition")
        key = (seq, start, True)
        if seq not in accepted or key not in deadlines or clock != hz or frames[seq]["partial"]:
            discarded["unmatched_or_ineligible_hzp3"] += 1
            continue
        if seq in seen:
            raise ValueError("duplicate exact render-tail record")
        seen.add(seq)
        if state == "lower-bound":
            points = deadlines[key]
            first, waited, ready, finished = (
                points[edge] for edge in ("start", "waited", "ready", "finished")
            )
            if not (first.pending == 1 and 0 < prior == first.submitted == first.retired < submitted
                    and waited.submitted == submitted and waited.retired <= retired < submitted
                    and retired >= prior and submitted <= ready.submitted < final
                    and retired <= ready.retired and finished.submitted == finished.retired == final
                    and began > 0 and lower <= finished.cpu - first.cpu):
                discarded["invalid_render_tail_order"] += 1
                continue
            bounds.append({"sequence": seq, "queue_residence_lower_bound_ms": lower * 1000 / hz})
        states[state] += 1
    return {"grouped_deadline_frames": len(accepted), "dispositions": dict(states),
            "discarded": dict(discarded), "bounds": bounds}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("serial", type=Path)
    parser.add_argument("--first-sequence", type=int)
    parser.add_argument("--last-sequence", type=int)
    parser.add_argument("--scale", type=int, default=75)
    parser.add_argument("--sections", type=int, default=174)
    parser.add_argument("--render-tail", action="store_true", help="join render-only queue receipts")
    args = parser.parse_args()
    if args.first_sequence is not None and args.last_sequence is not None:
        if args.first_sequence > args.last_sequence:
            parser.error("the first sequence must precede or equal the last sequence")
    try:
        raw = args.capture.read_bytes()
        meta, frames = read(raw)
        if meta["version"] != 3:
            raise ValueError("deadline records require HZP3")
        serial = args.serial.read_text(encoding="utf-8", errors="replace")
        deadlines = parse(serial)
        by_sequence = {row["sequence"]: row for row in frames}
        report = reduce(
            deadlines, by_sequence, hzp3_sequence_ticks(raw, frames), meta["clock_hz"],
            scale=args.scale, sections=args.sections, first=args.first_sequence, last=args.last_sequence,
        )
        if args.render_tail:
            report["render_tail"] = render_tail(serial, deadlines, report, by_sequence, meta["clock_hz"])
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
