import unittest

from gx2_client_tick_laps import join, join_game
from gx2_profile import HEADER, V3_ROW
from test_gx2_host_stages import hzp_frame


OUTER = (
    'Horizon Cafe client tick wall seq=8 start_ticks=100 clock_hz=1000 '
    'steps=2 stages=2/22/1/1/2/1\n'
)
INNER = (
    'Horizon Cafe client tick game wall seq=8 start_ticks=100 clock_hz=1000 '
    'steps=2 unlit_player_steps=1 stages=2/17/1/1\n'
)


def capture(**fields):
    header = HEADER.pack(0x485A5033, 3, 1000, V3_ROW.size, 4096, 1, 9)
    return header + hzp_frame(8, 100, loop=36, tick=32, **fields)


class ClientTickLapTest(unittest.TestCase):
    def test_joins_nested_work_and_keeps_legacy_outer_result(self):
        data = capture()
        frame, steps, unlit, times = join_game(data, OUTER + INNER)[0]
        self.assertEqual(frame['sequence'], 8)
        self.assertEqual((steps, unlit, times), (2, 1, (2, 17, 1, 1)))
        self.assertEqual(join(data, OUTER + INNER), [(frame, 2, (2, 22, 1, 1, 2, 1))])
        self.assertEqual(join_game(data, OUTER), [])

    def test_ignores_other_process_clocks_and_partial_frames(self):
        for mismatch in (INNER.replace('start_ticks=100', 'start_ticks=95'),
                         INNER.replace('clock_hz=1000', 'clock_hz=2000')):
            self.assertEqual(join_game(capture(), OUTER + mismatch), [])
        self.assertEqual(join_game(capture(partial=1), OUTER + INNER), [])

    def test_requires_enclosing_loop_and_unique_sample(self):
        with self.assertRaisesRegex(ValueError, 'no matching enclosing Game lap'):
            join_game(capture(), INNER)
        with self.assertRaisesRegex(ValueError, 'duplicate exact game tick sample'):
            join_game(capture(), OUTER + INNER * 2)

    def test_rejects_conflicting_counts_and_nested_time(self):
        for mismatch in (INNER.replace('steps=2', 'steps=1'),
                         INNER.replace('unlit_player_steps=1', 'unlit_player_steps=3'),
                         INNER.replace('2/17/1/1', '2/19/1/1')):
            with self.subTest(mismatch=mismatch), self.assertRaisesRegex(ValueError, 'exceeds enclosing work'):
                join_game(capture(), OUTER + mismatch)


if __name__ == '__main__':
    unittest.main()
