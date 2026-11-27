import unittest

from gx2_compact_draw_coverage import parse, reduce


RECORD = (
    'Horizon GX2 half terrain seq=240 stride=20 tv_draws=56 drc_draws=38 '
    'tv_indices=15132 drc_indices=8922'
)


def playing_pair(**changes):
    return dict(
        phase='playing', presented=1, partial=False, screenshot_ms=0,
        tv_submissions=1, gamepad_submissions=1, world_requested_scale=75,
        world_effective_scale=75, tv_world_width=1440, tv_world_height=809,
        drc_world_width=641, drc_world_height=360, tv_sections_drawn=174,
        drc_sections_drawn=130, tv_sections_tested=635, drc_sections_tested=635,
        game_frames_built=1, tv_width=1920, tv_height=1080, drc_width=854, drc_height=480,
        section_draws=314, indices=522008, hud_vertices=936,
    ) | changes


class CompactDrawCoverageTest(unittest.TestCase):
    def test_retains_distinct_output_counts_at_the_requested_stride(self):
        for stride in (20, 24):
            with self.subTest(stride=stride):
                serial = '[12:00:00] [OSConsole] ' + RECORD.replace('stride=20', f'stride={stride}')
                result = reduce(parse(serial), {240: playing_pair()}, stride=stride)
                sample, = result['samples']
                self.assertEqual(sample['compact_draws'], [56, 38])
                self.assertEqual(sample['compact_indices'], [15132, 8922])
                self.assertEqual(sample['stride'], stride)
                self.assertEqual(result['discarded'], {})

    def test_shader_ready_and_resident_payload_cannot_substitute_for_draws(self):
        log = 'Horizon GX2 half20 solid terrain shader ready\ngx2_vertex_payload_bytes=28264480'
        with self.assertRaisesRegex(ValueError, 'no compact terrain draw records'):
            reduce(parse(log), {240: playing_pair()}, stride=20)

    def test_zero_gamepad_work_cannot_qualify_the_pair(self):
        tv_only = RECORD.replace('drc_draws=38', 'drc_draws=0').replace('drc_indices=8922', 'drc_indices=0')
        with self.assertRaisesRegex(ValueError, 'missing_compact_draws_on_one_or_both_outputs'):
            reduce(parse(tv_only), {240: playing_pair()}, stride=20)

    def test_excludes_missing_partial_screenshot_and_underfilled_rows(self):
        serial = '\n'.join(RECORD.replace('seq=240', f'seq={seq}') for seq in (120, 240, 360, 480, 600))
        result = reduce(parse(serial), {
            240: playing_pair(partial=True), 360: playing_pair(screenshot_ms=1),
            480: playing_pair(drc_sections_drawn=90), 600: playing_pair(),
        }, stride=20)
        self.assertEqual([row['sequence'] for row in result['samples']], [600])
        self.assertEqual(result['discarded'], {
            'missing_capture_row': 1, 'ineligible_capture_row': 2, 'below_terrain_floor': 1,
        })

    def test_rejects_fallback_and_impossible_work(self):
        with self.assertRaisesRegex(ValueError, 'unexpected compact stride'):
            reduce(parse(RECORD), {240: playing_pair()}, stride=24)
        for frame in (playing_pair(section_draws=90), playing_pair(indices=18000),
                      playing_pair(drc_sections_drawn=30)):
            with self.subTest(frame=frame), self.assertRaisesRegex(ValueError, 'exceed frame work'):
                reduce(parse(RECORD), {240: frame}, stride=20, minimum_sections=10)

    def test_requires_a_fresh_frame_with_real_scanouts_and_optional_exact_sections(self):
        samples = parse(RECORD)
        for invalid in (playing_pair(game_frames_built=0), playing_pair(drc_width=0, drc_height=0)):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, 'ineligible_capture_row'):
                reduce(samples, {240: invalid}, stride=20)
        with self.assertRaisesRegex(ValueError, 'outside_exact_terrain_cohort'):
            reduce(samples, {240: playing_pair()}, stride=20, exact_sections=174)
        result = reduce(samples, {240: playing_pair(drc_sections_drawn=174)}, stride=20, exact_sections=174)
        self.assertEqual(result['samples'][0]['compact_indices'], [15132, 8922])

    def test_rejects_bad_cadence_duplicate_and_inconsistent_counts(self):
        for serial in (RECORD.replace('seq=240', 'seq=241'), RECORD + '\n' + RECORD,
                       RECORD.replace('drc_indices=8922', 'drc_indices=8921'),
                       RECORD.replace('drc_indices=8922', 'drc_indices=0')):
            with self.subTest(serial=serial), self.assertRaises(ValueError):
                parse(serial)


if __name__ == '__main__':
    unittest.main()
