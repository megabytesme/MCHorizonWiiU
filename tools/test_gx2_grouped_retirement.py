import struct
import unittest

import gx2_grouped_retirement as retirement
import gx2_profile as profile
from test_gx2_frame_laps import CLOCK_HZ, capture, ticks


def record(sequence, kind, *, retired=130, offset=0, drawn=1, accepted=1, after=140):
    return (f"Horizon GX2 grouped queue retirement seq={sequence} start_ticks={ticks(sequence) + offset} "
            f"class={kind} before_flush=100 submitted=140 retired_at_flush={retired} "
            f"submitted_after_draw_done={after} retired_after_draw_done={after} draw_done={drawn} "
            f"paired_accept={accepted}")


class GroupedRetirementTests(unittest.TestCase):
    def test_matches_queue_records_to_original_paired_frames(self):
        data = capture()
        offset = profile.HEADER.size + (360 - 119) * profile.V3_ROW.size
        word = profile.ROW.size // 4 + len(profile.V2_STAGES) + profile.V2_COUNTERS.index("gamepad_submissions")
        struct.pack_into(">I", data, offset + 4 * word, 0)
        pending = record(120, "pending-after-flush")
        serial = [f"{pending} {pending}", record(120, "pending-after-flush", offset=1),
                  record(240, "retired-after-flush", retired=140), record(360, "pending-after-flush"),
                  record(480, "invalid-draw-done", drawn=0), record(0, "pending-after-flush")]
        result = retirement.analyze(data, serial)
        self.assertEqual(result["counts"], {"pending-after-flush": 1, "retired-after-flush": 1,
                                             "invalid-draw-done": 1})
        self.assertEqual(result["tick_mismatch"], [120])
        self.assertEqual(result["outside_retained"], 1)
        self.assertEqual(result["excluded_unpaired_sequences"], [360])
        self.assertEqual([item["sequence"] for item in result["records"]], [120, 240, 480])
        self.assertEqual(result["records"][0]["tokens"]["retired_at_flush"], 130)

    def test_strict_pre_wait_retirement_and_matched_finish_stage_cost(self):
        data = capture()
        for sequence, cost in [(120, 30_000), (240, 10_000), (360, 90_000), (480, 70_000)]:
            offset = profile.HEADER.size + (sequence - 119) * profile.V3_ROW.size
            def write(word, value):
                struct.pack_into(">I", data, offset + 4 * word, value)

            write(6 + profile.STAGES.index("finish_present"), cost)
            v2_start = profile.ROW.size // 4 + len(profile.V2_STAGES)
            v3_start = profile.V2_ROW.size // 4 + len(profile.V3_STAGES)
            for name, value in {"tv_width": 1920, "tv_height": 1080,
                                "drc_width": 854, "drc_height": 480,
                                "drc_sections_drawn": 8}.items():
                write(v2_start + profile.V2_COUNTERS.index(name), value)
            for name, value in {"world_requested_scale": 75, "tv_world_width": 1440,
                                "tv_world_height": 809, "drc_world_width": 641,
                                "drc_world_height": 360}.items():
                write(v3_start + profile.V3_COUNTERS.index(name), value)

        result = retirement.analyze(data, [
            record(120, "pending-after-flush"),
            record(240, "retired-after-flush", retired=140),
            record(360, "retired-after-flush", retired=140, after=145),
            record(480, "pending-after-flush"),
        ])
        self.assertEqual(result["counts"]["retired-after-flush"], 2)
        self.assertEqual(result["retired_unchanged_during_draw_done"], 1)
        self.assertEqual([item["retired_unchanged_during_draw_done"] for item in result["records"]],
                         [False, True, False, False])
        compare, = result["matched_finish_comparisons"]
        self.assertEqual(compare["sections_tv_gamepad"], [5, 8])
        self.assertEqual(compare["world_dimensions_tv_gamepad"], [[1440, 809], [641, 360]])
        self.assertFalse(compare["partial"])
        self.assertEqual(compare["retired_unchanged_samples"], 1)
        self.assertEqual(compare["pending_samples"], 1)
        self.assertAlmostEqual(compare["pending_minus_retired_finish_mean_ms"], 20_000_000 / CLOCK_HZ)
        self.assertAlmostEqual(result["records"][3]["finish_present_ms"], 70_000_000 / CLOCK_HZ)

    def test_rejects_invalid_labels_and_conflicting_records(self):
        data = capture()
        with self.assertRaisesRegex(ValueError, "classification disagrees"):
            retirement.analyze(data, [record(120, "pending-after-flush", retired=141)])
        with self.assertRaisesRegex(ValueError, "conflicting"):
            retirement.analyze(data, [record(120, "pending-after-flush"),
                                      record(120, "pending-after-flush", retired=135)])
