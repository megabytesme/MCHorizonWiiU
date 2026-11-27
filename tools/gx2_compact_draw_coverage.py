"""Check sparse compact terrain draw samples against their same-run HZP3 capture."""

import argparse
from collections import Counter
from dataclasses import dataclass
import json
from pathlib import Path
import re

from gx2_profile import read
from gx2_solid_replay_coverage import paired_frame


TAG = 'Horizon GX2 half terrain'
RECORD = re.compile(re.escape(TAG) + (
    r' seq=(\d+) stride=(\d+) tv_draws=(\d+) drc_draws=(\d+) '
    r'tv_indices=(\d+) drc_indices=(\d+)$'
))
SAMPLE_INTERVAL = 120
WORK_FIELDS = (
    'tv_sections_tested', 'drc_sections_tested', 'tv_sections_drawn', 'drc_sections_drawn',
    'section_draws', 'indices', 'hud_vertices', 'tv_world_width', 'tv_world_height',
    'drc_world_width', 'drc_world_height',
)


@dataclass(frozen=True)
class Sample:
    sequence: int
    stride: int
    tv_draws: int
    drc_draws: int
    tv_indices: int
    drc_indices: int


def parse(serial):
    samples = []
    for line in serial.splitlines():
        if TAG not in line:
            continue
        match = RECORD.search(line)
        if match is None:
            raise ValueError('malformed compact terrain sample')
        values = tuple(map(int, match.groups()))
        if any(value > 0xffffffff for value in values):
            raise ValueError('compact terrain sample exceeds uint32 range')
        sample = Sample(*values)
        if sample.stride not in (20, 24) or sample.sequence % SAMPLE_INTERVAL:
            raise ValueError('invalid compact terrain stride or sampling sequence')
        if samples and sample.sequence <= samples[-1].sequence:
            raise ValueError('duplicate or out-of-order compact terrain sample')
        for draws, indices in ((sample.tv_draws, sample.tv_indices), (sample.drc_draws, sample.drc_indices)):
            if indices % 6 or indices < draws * 6 or bool(draws) != bool(indices):
                raise ValueError('inconsistent compact quad draw counts')
        samples.append(sample)
    return samples


def reduce(samples, frames, *, stride, minimum_sections=100, exact_sections=None):
    """Require positive compact draws and indices on both independently rendered outputs."""
    if stride not in (20, 24) or minimum_sections < 1:
        raise ValueError('expected stride must be 20 or 24 and the terrain floor positive')
    if exact_sections is not None and exact_sections < minimum_sections:
        raise ValueError('exact section count must meet the terrain floor')
    if not samples:
        raise ValueError('no compact terrain draw records')
    retained = []
    discarded = Counter()
    for sample in samples:
        if sample.stride != stride:
            raise ValueError(f'unexpected compact stride at sequence {sample.sequence}')
        frame = frames.get(sample.sequence)
        if frame is None:
            discarded['missing_capture_row'] += 1
            continue
        if (not paired_frame(frame) or frame['game_frames_built'] != 1
            or (frame['tv_width'], frame['tv_height'], frame['drc_width'], frame['drc_height'])
                != (1920, 1080, 854, 480)):
            discarded['ineligible_capture_row'] += 1
            continue
        sections = frame['tv_sections_drawn'], frame['drc_sections_drawn']
        if min(sections) < minimum_sections:
            discarded['below_terrain_floor'] += 1
            continue
        if exact_sections is not None and sections != (exact_sections, exact_sections):
            discarded['outside_exact_terrain_cohort'] += 1
            continue
        draws = [sample.tv_draws, sample.drc_draws]
        indices = [sample.tv_indices, sample.drc_indices]
        if (draws[0] > frame['tv_sections_drawn'] or draws[1] > frame['drc_sections_drawn']
            or sum(draws) > frame['section_draws'] or sum(indices) > frame['indices']):
            raise ValueError(f'compact draw counts exceed frame work at sequence {sample.sequence}')
        if min(draws) == 0:
            discarded['missing_compact_draws_on_one_or_both_outputs'] += 1
            continue
        retained.append({
            'sequence': sample.sequence, 'stride': sample.stride,
            'compact_draws': draws, 'compact_indices': indices,
            'work': [frame[field] for field in WORK_FIELDS],
        })
    if not retained:
        raise ValueError(f'no eligible paired compact terrain draws ({dict(discarded)})')
    return {'stride': stride, 'work_fields': WORK_FIELDS, 'samples': retained, 'discarded': discarded,
            'join_key': 'sequence_from_same_run_inputs'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('serial', type=Path, help='verified serial log from the same run as the capture')
    parser.add_argument('--stride', type=int, choices=(20, 24), required=True)
    parser.add_argument('--minimum-sections', type=int, default=100)
    parser.add_argument('--exact-sections', type=int, help='require this section count on each output')
    args = parser.parse_args()
    try:
        header, frames = read(args.capture.read_bytes())
        if header['version'] != 3:
            raise ValueError('a HZP3 capture is required')
        samples = parse(args.serial.read_text(encoding='utf-8', errors='replace'))
        result = reduce(samples, {row['sequence']: row for row in frames}, stride=args.stride,
                        minimum_sections=args.minimum_sections, exact_sections=args.exact_sections)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
