#!/usr/bin/env python3
"""Join client tick wall detail to the matching HZP3 process-session capture."""
import argparse
from collections import Counter
from pathlib import Path
import re
from statistics import median

import gx2_profile as hzp

STAGES = ("pre", "game", "ambience", "gui_music", "sound_engine", "post")
LINE = re.compile(
    r"Horizon Cafe client tick wall seq=(\d+) start_ticks=(\d+) clock_hz=(\d+) "
    r"steps=(\d+) stages=(\d+(?:/\d+){5})(?![/\d])"
)


def join(data, serial):
    meta, frames = hzp.read(data)
    if meta["version"] != 3:
        raise ValueError("requires HZP3")
    starts = hzp.hzp3_sequence_ticks(data, frames)
    frame_by_seq = {frame["sequence"]: frame for frame in frames}
    unique = set()
    result = []
    for value in LINE.finditer(serial):
        seq, start, freq, steps = (int(x) for x in value.groups()[:4])
        stages = tuple(int(x) for x in value[5].split("/"))
        frame = frame_by_seq.get(seq)
        if frame is None or start != starts[seq] or freq != meta["clock_hz"]:
            continue
        if frame["partial"]:
            continue
        if seq in unique:
            raise ValueError(f"duplicate exact row {seq}")
        if (frame["phase"] != "playing" or frame["screenshot_ms"] or frame["presented"] != 1
                or frame["tv_submissions"] != 1 or frame["gamepad_submissions"] != 1
                or not frame["tv_sections_drawn"] or not frame["drc_sections_drawn"]
                or not 1 <= steps <= 10 or not 0 < sum(stages)
                or sum(stages) > round(frame["tick_ms"] * freq / 1000)):
            raise ValueError(f"invalid exact tick sample {seq}")
        result.append((frame, steps, tuple(v * 1000 / freq for v in stages)))
        unique.add(seq)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("serial", type=Path)
    parser.add_argument("--sections", type=int, help="require this count on both displays")
    args = parser.parse_args()
    rows = join(args.capture.read_bytes(), args.serial.read_text(errors="replace"))
    if args.sections:
        rows = [row for row in rows
                if row[0]["tv_sections_drawn"] == row[0]["drc_sections_drawn"] == args.sections]
    if not rows:
        parser.exit(2, "no exact matching tick rows\n")
    print(f"{len(rows)} exact rows, steps {dict(sorted(Counter(s for _, s, _ in rows).items()))}")
    for stage, name in enumerate(STAGES):
        print(f"{name:13} {median(v[stage] for _, _, v in rows):.3f} ms/frame median, "
              f"{median(v[stage] / steps for _, steps, v in rows):.3f} ms/step median")
    workers = [frame["integrated_tick_ms"] for frame, _, _ in rows
               if frame["integrated_tick_ms"] is not None and frame["integrated_tick_count"]]
    print(f"Integrated wall spans completed in {len(workers)}/{len(rows)} joined frame intervals")
    if workers:
        print(f"Median total per nonzero completion interval {median(workers):.3f} ms")


if __name__ == "__main__":
    main()
