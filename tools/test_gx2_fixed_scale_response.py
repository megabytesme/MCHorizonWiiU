from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from gx2_fixed_scale_response import compare
from gx2_profile import HEADER, V3_ROW
from gx2_saved_view_compare import retained_window
from test_gx2_host_stages import hzp_frame


def capture(path, scale, *, step, partial=False, effective=None, tv_width=1920):
    count = 60_000 // step + 1
    dimensions = (1440, 809, 641, 360) if scale == 75 else (1248, 702, 555, 312)
    rows = [
        hzp_frame(
            seq, seq * step, game_frames_built=1, world_requested_scale=scale,
            world_effective_scale=effective if seq == 1 and effective is not None else scale,
            tv_width=tv_width, tv_height=1080, drc_width=854, drc_height=480,
            tv_world_width=dimensions[0], tv_world_height=dimensions[1],
            drc_world_width=dimensions[2], drc_world_height=dimensions[3],
            partial=int(partial and seq == 1), section_draws=510, indices=1053348,
        )
        for seq in range(count)
    ]
    path.write_bytes(HEADER.pack(0x485A5033, 3, 1000, V3_ROW.size, 4096, count, count) + b"".join(rows))
    return path, 0, count - 1


class FixedScaleResponseTest(unittest.TestCase):
    def test_scale_comparison_preserves_whole_windows_and_primary_scope(self):
        with TemporaryDirectory() as directory:
            base = Path(directory)
            stock = retained_window(*capture(base / "75.hzp", 75, step=30_000, partial=True), fixed_scale=True)
            trial = retained_window(*capture(base / "65.hzp", 65, step=20_000), scale=65, fixed_scale=True)
            report = compare([(65, trial), (75, stock)])
            self.assertEqual(report["scales_in_capture_order"], [65, 75])
            summary = report["scale_summaries"]
            self.assertEqual(summary[75]["world_pixels_per_pair"], 1395720)
            self.assertEqual(summary[75]["median_complete_pairs_per_second"], 0.017)
            self.assertEqual(summary[65]["world_pixels_per_pair"], 1049256)
            self.assertEqual(summary[65]["paired_rate_change_from_fixed75"], 0.017)
            self.assertEqual(summary[65]["reported_median_indices_each_window"], [1053348])
            self.assertTrue(summary[75]["eligible_scale_for_primary_stock_measurement"])
            self.assertFalse(summary[65]["eligible_scale_for_primary_stock_measurement"])

    def test_rejects_a_presented_effective_scale_transition(self):
        with TemporaryDirectory() as directory:
            case = capture(Path(directory) / "mixed.hzp", 65, step=20_000, effective=75)
            with self.assertRaisesRegex(ValueError, "outside fixed scale 65"):
                retained_window(*case, scale=65, fixed_scale=True)

    def test_rejects_nonmatched_outputs_and_overlapping_windows(self):
        with TemporaryDirectory() as directory:
            base = Path(directory)
            stock = retained_window(*capture(base / "75.hzp", 75, step=30_000))
            trial = retained_window(*capture(base / "65.hzp", 65, step=20_000, tv_width=1280), scale=65)
            with self.assertRaisesRegex(ValueError, "display dimensions changed"):
                compare([(75, stock), (65, trial)])
            with self.assertRaisesRegex(ValueError, "overlap"):
                compare([(75, stock), (65, stock)])
            with self.assertRaisesRegex(ValueError, "fixed-75 reference"):
                compare([(65, trial)])


if __name__ == "__main__":
    unittest.main()
