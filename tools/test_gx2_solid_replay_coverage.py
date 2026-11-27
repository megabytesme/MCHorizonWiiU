import unittest

from gx2_solid_replay_coverage import parse, reduce


RECORD = (
    'Horizon GX2 solid list replay first=1 last=120 start=140000002 end=200000004 '
    'tv_cold=3 drc_cold=4 tv_warm=89 drc_warm=90 warm_both=80 '
    'tv_min=174 drc_min=174 tv_max=174 drc_max=174'
)


def playing_pair():
    return dict(
        phase='playing', presented=1, partial=False, screenshot_ms=0,
        tv_submissions=1, gamepad_submissions=1, world_requested_scale=75,
        world_effective_scale=75, tv_world_width=1440, tv_world_height=809,
        drc_world_width=641, drc_world_height=360, tv_sections_drawn=174,
        drc_sections_drawn=174,
    )


class SolidReplayCoverageTest(unittest.TestCase):
    def setUp(self):
        self.windows = parse('[12:00:00] [OSConsole] ' + RECORD + '\n')
        self.frames = {sequence: playing_pair() for sequence in range(1, 121)}
        self.starts = {1: 0x140000002, 120: 0x200000004}

    def test_matches_complete_independent_pairs_at_exact_clock_edges(self):
        result = reduce(self.windows, self.frames, self.starts, sections=174)
        self.assertEqual(result['qualified_pairs'], 120)
        self.assertEqual(result['warm_both'], 80)
        self.assertEqual(result['retained_sequence_ranges'], [[1, 120]])
        self.assertEqual(result['discarded'], {})

    def test_second_run_with_same_sequences_and_sections_cannot_join(self):
        result = reduce(self.windows, self.frames, self.starts | {120: 4})
        self.assertEqual(result['qualified_pairs'], 0)
        self.assertEqual(result['discarded'], {'start_ticks_do_not_match': 1})

    def test_rejects_any_missing_or_incomplete_output_in_window(self):
        self.frames[55]['partial'] = True
        result = reduce(self.windows, self.frames, self.starts)
        self.assertEqual(result['discarded'], {'ineligible_capture_rows': 1})
        del self.frames[55]
        result = reduce(self.windows, self.frames, self.starts)
        self.assertEqual(result['discarded'], {'missing_capture_rows': 1})

    def test_rejects_geometry_cohort_mismatch(self):
        result = reduce(self.windows, self.frames, self.starts, sections=163)
        self.assertEqual(result['discarded'], {'outside_requested_cohort': 1})
        self.frames[55]['drc_sections_drawn'] = 163
        result = reduce(self.windows, self.frames, self.starts)
        self.assertEqual(result['discarded'], {'section_counts_do_not_match': 1})

    def test_rejects_impossible_intersection_and_bad_window_order(self):
        for record in (
            RECORD.replace('warm_both=80', 'warm_both=58'),
            RECORD.replace('last=120', 'last=119'),
            RECORD.replace('end=200000004', 'end=4'),
            RECORD + '\n' + RECORD,
        ):
            with self.subTest(record=record), self.assertRaises(ValueError):
                parse(record)


if __name__ == '__main__':
    unittest.main()
