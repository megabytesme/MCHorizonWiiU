"""Validate opt-in GX2 solid replay coverage against paired HZP3 frames."""
import argparse
from collections import Counter
from dataclasses import dataclass
import json
from pathlib import Path
import re

from gx2_profile import hzp3_sequence_ticks, read

PAIRS_PER_WINDOW = 120
TAG = 'Horizon GX2 solid list replay first='
RECORD = re.compile(re.escape(TAG) + (
    r'(\d+) last=(\d+) start=([a-f0-9]{1,16}) end=([a-f0-9]{1,16}) '
    r'tv_cold=(\d+) drc_cold=(\d+) tv_warm=(\d+) drc_warm=(\d+) '
    r'warm_both=(\d+) tv_min=(\d+) drc_min=(\d+) tv_max=(\d+) drc_max=(\d+)$'
))


@dataclass(frozen=True)
class Window:
    first: int
    last: int
    start_ticks: int
    end_ticks: int
    tv_cold: int
    drc_cold: int
    tv_warm: int
    drc_warm: int
    warm_both: int
    tv_min: int
    drc_min: int
    tv_max: int
    drc_max: int


def parse(serial):
    windows = []
    for line in serial.splitlines():
        if TAG not in line:
            continue
        match = RECORD.search(line)
        if match is None:
            raise ValueError('malformed solid replay coverage window')
        first, last, start, end, *counts = match.groups()
        window = Window(int(first), int(last), int(start, 16), int(end, 16), *map(int, counts))
        if not 0 <= window.first <= window.last <= 0xffffffff or window.last - window.first + 1 != PAIRS_PER_WINDOW:
            raise ValueError('invalid solid replay sequence range')
        if window.start_ticks >= window.end_ticks:
            raise ValueError('invalid solid replay timestamp range')
        if windows and windows[-1].last >= window.first:
            raise ValueError('out-of-order or overlapping solid replay windows')
        if (window.tv_cold + window.tv_warm > PAIRS_PER_WINDOW
            or window.drc_cold + window.drc_warm > PAIRS_PER_WINDOW
            or not max(0, window.tv_warm + window.drc_warm - PAIRS_PER_WINDOW)
                <= window.warm_both <= min(window.tv_warm, window.drc_warm)
            or window.tv_min > window.tv_max or window.drc_min > window.drc_max):
            raise ValueError('invalid solid replay coverage counts')
        windows.append(window)
    return windows


def paired_frame(row):
    return (row['phase'] == 'playing' and row['presented'] == 1 and not row['partial']
            and not row['screenshot_ms'] and row['tv_submissions'] == row['gamepad_submissions'] == 1
            and row['world_requested_scale'] == row['world_effective_scale'] == 75
            and (row['tv_world_width'], row['tv_world_height'], row['drc_world_width'], row['drc_world_height'])
                == (1440, 809, 641, 360))


def reduce(windows, frames, starts, *, sections=None):
    joined = []
    discarded = Counter()
    for window in windows:
        rows = [frames.get(sequence) for sequence in range(window.first, window.last + 1)]
        if any(row is None for row in rows):
            discarded['missing_capture_rows'] += 1
            continue
        if (starts.get(window.first), starts.get(window.last)) != (window.start_ticks, window.end_ticks):
            discarded['start_ticks_do_not_match'] += 1
            continue
        if any(not paired_frame(row) for row in rows):
            discarded['ineligible_capture_rows'] += 1
            continue
        tv = [row['tv_sections_drawn'] for row in rows]
        drc = [row['drc_sections_drawn'] for row in rows]
        if (min(tv), max(tv), min(drc), max(drc)) != (window.tv_min, window.tv_max, window.drc_min, window.drc_max):
            discarded['section_counts_do_not_match'] += 1
            continue
        if sections is not None and (min(tv), max(tv), min(drc), max(drc)) != (sections,) * 4:
            discarded['outside_requested_cohort'] += 1
            continue
        joined.append(window)
    return {
        'windows_joined': len(joined), 'qualified_pairs': PAIRS_PER_WINDOW * len(joined),
        'cold_tv': sum(row.tv_cold for row in joined), 'cold_gamepad': sum(row.drc_cold for row in joined),
        'warm_tv': sum(row.tv_warm for row in joined), 'warm_gamepad': sum(row.drc_warm for row in joined),
        'warm_both': sum(row.warm_both for row in joined), 'discarded': discarded,
        'retained_sequence_ranges': [[row.first, row.last] for row in joined],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('serial', type=Path)
    parser.add_argument('--sections', type=int)
    args = parser.parse_args()
    capture = args.capture.read_bytes()
    header, frames = read(capture)
    if header['version'] != 3:
        parser.error('a HZP3 capture is required')
    windows = parse(args.serial.read_text(encoding='utf-8', errors='replace'))
    frames_by_sequence = {row['sequence']: row for row in frames}
    starts = hzp3_sequence_ticks(capture, frames)
    print(json.dumps(reduce(windows, frames_by_sequence, starts, sections=args.sections), indent=2))


if __name__ == '__main__':
    main()
