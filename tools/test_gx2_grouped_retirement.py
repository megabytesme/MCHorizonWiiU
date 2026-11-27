import struct
import unittest

import gx2_grouped_retirement as retirement
import gx2_profile as profile
from test_gx2_frame_laps import capture, ticks


def record(sequence, kind, *, retired=130, offset=0, drawn=1, accepted=1):
    return (f"Horizon GX2 grouped queue retirement seq={sequence} start_ticks={ticks(sequence) + offset} "
            f"class={kind} before_flush=100 submitted=140 retired_at_flush={retired} "
            f"submitted_after_draw_done=140 retired_after_draw_done=140 draw_done={drawn} "
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

    def test_rejects_invalid_labels_and_conflicting_records(self):
        data = capture()
        with self.assertRaisesRegex(ValueError, "classification disagrees"):
            retirement.analyze(data, [record(120, "pending-after-flush", retired=141)])
        with self.assertRaisesRegex(ValueError, "conflicting"):
            retirement.analyze(data, [record(120, "pending-after-flush"),
                                      record(120, "pending-after-flush", retired=135)])
