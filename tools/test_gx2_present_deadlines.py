from dataclasses import replace
import unittest

from gx2_present_deadlines import Point, parse, reduce


def row():
    return {
        "phase": "playing", "presented": 1, "tv_submissions": 1, "gamepad_submissions": 1,
        "world_requested_scale": 75, "world_effective_scale": 75, "screenshot_ms": 0,
        "tv_sections_drawn": 174, "drc_sections_drawn": 174,
        "tv_world_width": 1440, "tv_world_height": 809, "drc_world_width": 641, "drc_world_height": 360,
        "loop_ms": 50, "wait_flip_ms": 4, "finish_present_ms": 2,
    }


def grouped_trace():
    return {
        "start": Point(1001, 20, 20, 901, 891, 109, 109),
        "waited": Point(1010, 20, 20, 901, 891, 109, 109),
        "ready": Point(1034, 20, 20, 917, 891, 110, 109),
        "posted": Point(1040, 21, 20, 934, 891, 120, 115),
        "finished": Point(1047, 21, 20, 934, 891, 120, 120),
        "next": Point(1056, 21, 20, 951, 891, 120, 120),
    }


def logs(points, grouped=True, start=1000):
    result = []
    for name, p in points.items():
        result.append(f"[09:39:00.000] [OSConsole] Horizon GX2 deadline seq=100 start={start:x} "
                      f"mode={int(grouped)} edge={name} cpu={p.cpu:x} s={p.swaps:x} f={p.flips:x} "
                      f"v={p.vblank:x} l={p.flip_time:x} q={p.submitted:x} r={p.retired:x}")
    return "\n".join(result)


def analyze(text, rows=None, **bounds):
    if rows is None:
        rows = {100: row(), 101: row()}
    return reduce(parse(text), rows, {100: 1000, 101: 1055}, 1000, scale=75, sections=174, **bounds)


class DeadlineTests(unittest.TestCase):
    def test_three_independent_witnesses(self):
        report = analyze(logs(grouped_trace()))
        self.assertEqual(report["joined"], 1)
        self.assertTrue(report["independent_vblank_observed"])
        self.assertEqual(set(report["refresh_observations"]), {
            "refresh_before_swap_from_start",
            "refresh_in_swap_bracket_with_gpu_pending",
            "refresh_with_retired_pair_still_unflipped",
        })
        self.assertEqual(report["frames"][0]["before_swap_ms"], 33)
        self.assertEqual(report["frames"][0]["finish_bracket_ms"], 13)

    def test_ordinary_path_without_native_flush_split(self):
        points = grouped_trace()
        points.pop("posted")
        points["start"] = replace(points["start"], flips=19, flip_time=874)
        report = analyze(logs(points, grouped=False))
        self.assertEqual(set(report["refresh_observations"]), {
            "refresh_before_swap_from_waited", "refresh_with_retired_pair_still_unflipped",
        })
        self.assertFalse(report["frames"][0]["after_flush"])

    def test_cemu_aliased_clock_yields_no_refresh_claim(self):
        points = {name: replace(p, vblank=p.flip_time) for name, p in grouped_trace().items()}
        report = analyze(logs(points))
        self.assertEqual(report["joined"], 1)
        self.assertFalse(report["independent_vblank_observed"])
        self.assertEqual(report["refresh_observations"], {})

    def test_no_following_playing_frame_after_home_or_ring_loss(self):
        report = analyze(logs(grouped_trace()), rows={100: row()})
        self.assertEqual(report["joined"], 1)
        self.assertNotIn("refresh_with_retired_pair_still_unflipped", report["refresh_observations"])
        self.assertFalse(report["frames"][0]["follow"])

    def test_discard_following_observation_beyond_window(self):
        report = analyze(logs(grouped_trace()), first=100, last=100)
        self.assertEqual(report["joined"], 1)
        self.assertFalse(report["frames"][0]["follow"])
        self.assertNotIn("refresh_with_retired_pair_still_unflipped", report["refresh_observations"])

    def test_unsafe_depth_and_incomplete_frame_are_rejected(self):
        points = grouped_trace()
        points["ready"] = replace(points["ready"], flips=18)
        self.assertEqual(analyze(logs(points))["discarded"], {"invalid_snapshot_order_or_pending_depth": 1})
        points = grouped_trace()
        points.pop("waited")
        self.assertEqual(analyze(logs(points))["discarded"], {"incomplete_or_misplaced_edges": 1})

    def test_exact_hzp_epoch_and_stock_coverage_are_mandatory(self):
        self.assertEqual(analyze(logs(grouped_trace(), start=999))["joined"], 0)
        first = row()
        first["drc_sections_drawn"] = 173
        self.assertEqual(analyze(logs(grouped_trace()), {100: first})["joined"], 0)
        first["drc_sections_drawn"] = 174
        first["screenshot_ms"] = 1
        self.assertEqual(analyze(logs(grouped_trace()), {100: first})["joined"], 0)
        first["screenshot_ms"] = 0
        first["world_requested_scale"] = 85
        self.assertEqual(analyze(logs(grouped_trace()), {100: first})["joined"], 0)

    def test_u32_queue_wrap_remains_one_ticket(self):
        points = {
            name: replace(p, swaps=(p.swaps + 0xffffffff - 20) & 0xffffffff,
                          flips=(p.flips + 0xffffffff - 20) & 0xffffffff)
            for name, p in grouped_trace().items()
        }
        self.assertEqual(len(analyze(logs(points))["refresh_observations"]), 3)

    def test_duplicate_or_fragmented_edges_fail_closed(self):
        captured = logs(grouped_trace())
        with self.assertRaisesRegex(ValueError, "duplicate"):
            parse(captured + "\n" + captured)
        with self.assertRaisesRegex(ValueError, "malformed"):
            parse(captured[:captured.find(" q=")])


if __name__ == "__main__":
    unittest.main()
