import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from gx2_cemu import filesystem_path, require_fixture_space, start
from gx2_cemu_acceptance import MIN_MOVEMENT_BLOCKS, movement_along_input


class MovementEvidenceTests(unittest.TestCase):
    def test_displacement_in_requested_world_direction(self):
        start = (-13.0, 68.0, 96.0)
        cases = [
            ("backward", 0.0, (-13.0, 68.0, 94.0)),
            ("forward", -90.0, (-11.0, 68.0, 96.0)),
            ("left", 90.0, (-13.0, 68.0, 98.0)),
            ("right", 0.0, (-15.0, 68.0, 96.0)),
        ]
        for direction, yaw, end in cases:
            with self.subTest(direction=direction, yaw=yaw):
                self.assertAlmostEqual(movement_along_input(start, end, yaw, direction), 2.0)

    def test_other_movement_does_not_verify_backward_input(self):
        start = (0.0, 68.0, 96.0)
        for end in ((0.0, 68.0, 97.0), (1.0, 68.0, 96.0), (0.0, 69.0, 96.0),
                    (0.0, 68.0, 96.0 - 1e-6)):
            with self.subTest(end=end):
                self.assertLess(movement_along_input(start, end, 0.0, "backward"), MIN_MOVEMENT_BLOCKS)


class CemuProfileTests(unittest.TestCase):
    def test_start_uses_tv_in_main_window_and_keeps_selected_cpu_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            lab = Path(folder)
            (lab / "portable").mkdir()
            (lab / "input-hashes.json").write_text("{}", encoding="utf-8")
            for file in ("Cemu.exe", "HorizonWiiU.rpx"):
                (lab / file).write_bytes(b"stub")
            title_id = "0123456789abcdef"
            with patch("gx2_cemu.subprocess.Popen", return_value=SimpleNamespace(pid=12)):
                start(lab, "multi", title_id)
            profile = (lab / "portable/gameProfiles" / f"{title_id}.ini").read_text(encoding="ascii")
            self.assertEqual(profile.splitlines(), [
                "[General]", "startWithPadView = false", "[CPU]", "cpuMode = 3",
            ])


@unittest.skipUnless(os.name == "nt", "Windows long-path handling")
class FixtureSpaceTests(unittest.TestCase):
    def test_counts_resources_beyond_max_path(self):
        temp_root = filesystem_path(Path(tempfile.gettempdir()))
        with tempfile.TemporaryDirectory(dir=temp_root) as folder:
            lab = Path(folder)
            plain_root = Path(tempfile.gettempdir()) / lab.name
            resources = lab / "resources"
            nested = resources
            for name in ("a", "b", "c"):
                nested /= name * 90
            nested.mkdir(parents=True)
            (nested / "terrain.json").write_bytes(b"fixture")
            with patch("gx2_cemu.shutil.disk_usage", return_value=SimpleNamespace(free=0)):
                with self.assertRaisesRegex(ValueError, "fixture copy needs"):
                    require_fixture_space(plain_root / "new-lab", [plain_root / "resources"])


if __name__ == "__main__":
    unittest.main()
