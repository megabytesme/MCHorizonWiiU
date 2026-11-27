import unittest

import gx2_opaque_tiles as tiles


def record(pair, output, *, covered=20, skipped=3):
    return (f"Horizon GX2 opaque tiles pair={pair} output={output} wall_us=400 faces=98 "
            f"tiles=76 sections=174 hidden=2 indices=100 hidden_indices={covered} skipped_uploads={skipped}")


class OpaqueTilesTests(unittest.TestCase):
    def test_groups_only_complete_pairs_and_deduplicates_identical_lines(self):
        tv = record(119, 0)
        report = tiles.analyze([
            tv, tv, record(119, 1, covered=30), record(239, 0),
            "Horizon GX2 opaque tiles pair=359 incomplete=work-or-projection-limit",
        ])
        self.assertEqual(report["paired_samples"], 1)
        self.assertEqual(report["missing_output_ordinals"], [239])
        self.assertEqual(report["incomplete_pair_ordinals"], [359])
        self.assertEqual(report["displays"]["tv"]["covered_fraction"], .2)
        self.assertEqual(report["displays"]["gamepad"]["covered_fraction"], .3)

    def test_rejects_conflicting_or_out_of_bounds_observations(self):
        for lines, error in (
            ([record(119, 0), record(119, 0, covered=21)], "conflicting"),
            ([record(119, 0, covered=101)], "outside"),
            ([record(119, 0), record(119, 1, skipped=1)], "upload omissions"),
            ([record(119, 0), "Horizon GX2 opaque tiles pair=119 incomplete=work-or-projection-limit"],
             "both a limit"),
        ):
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, error):
                tiles.analyze(lines)
