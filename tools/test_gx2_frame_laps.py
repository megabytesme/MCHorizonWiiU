import unittest

import gx2_frame_laps as laps
import gx2_profile as profile


CLOCK_HZ = 62_156_250
ORIGIN = 1 << 60


def ticks(sequence):
    return ORIGIN + 4096 * sequence


def capture():
    first, total, capacity = 119, 481, 4096
    data = bytearray(profile.HEADER.pack(
        0x485A5033, 3, CLOCK_HZ, profile.V3_ROW.size, capacity, total - first, total,
    ))
    for sequence in range(first, total):
        tick = ticks(sequence)
        prefix = [sequence, 5, tick >> 32, tick & 0xFFFFFFFF, 130_000, 145_000]
        timings = {"world_update": 100_000, "tick": 7000, "terrain_wait": 50_000}
        stages = [timings.get(key, 0) for key in profile.STAGES]
        counters = [1, int(sequence == 480), *([0] * (len(profile.COUNTERS) - 2))]
        workers = [0, 0, sequence, sequence * 1000, 0, 0, 0, 0]
        v2 = {"game_frames_built": 1, "tv_submissions": 1, "gamepad_submissions": 1,
              "tv_sections_drawn": 5, "drc_sections_drawn": 0 if sequence == 360 else 8}
        gpu = [v2.get(key, 0) for key in profile.V2_COUNTERS]
        world = [75 if key == "world_effective_scale" else 0 for key in profile.V3_COUNTERS]
        data.extend(profile.V3_ROW.pack(*prefix, *stages, *counters, *workers,
                                       *([0] * len(profile.V2_STAGES)), *gpu,
                                       *([0] * len(profile.V3_STAGES)), *world))
    return data


def line(sequence, group, offset=0, duration=2213):
    return (f"Horizon Cafe frame wall us seq={sequence} start_ticks={ticks(sequence) + offset} "
            f"group={group} samples=[{duration}, 0, 4294967295, 100, 42, 0]")


def lines(sequence, offset=0):
    return [line(sequence, group, offset) for group in range(4)]


class FrameLapsTests(unittest.TestCase):
    def test_exact_start_ticks_and_pair_admission(self):
        self.assertEqual(float(ticks(120)), float(ticks(120) + 1))
        normal = lines(120)
        duplicate = f"[OSConsole] {normal[0]} [OSConsole] {normal[0]}"
        serial = [duplicate, *normal[1:], *lines(120, offset=1),
                  *lines(240)[:3], *lines(360), *lines(480)]
        result = laps.join(capture(), serial)
        self.assertEqual([row["sequence"] for row in result["paired_samples"]], [120, 480])
        self.assertEqual(result["tick_mismatch"], [120])
        self.assertEqual(result["incomplete"], [240])
        self.assertEqual(result["excluded"], [360])
        first, partial = result["paired_samples"]
        self.assertEqual(first["start_ticks"], ticks(120))
        self.assertEqual(first["wall_us"]["prepare"], 2213)
        self.assertIsNone(first["wall_us"]["pr_cleanup"])
        self.assertAlmostEqual(first["hzp_wall_ms"]["tick"], 7000 * 1000 / CLOCK_HZ)
        self.assertAlmostEqual(first["hzp_worker_delta_ms"]["mesh_worker"], 1000 * 1000 / CLOCK_HZ)
        self.assertFalse(first["partial"])
        self.assertTrue(partial["partial"])

    def test_rejects_conflicts_and_invalid_values(self):
        data = capture()
        normal = lines(120)
        for invalid, reason in (
            (line(120, 0, duration=3001), "conflicting"),
            (line(120, 0, duration=2**32), "invalid"),
            (line(121, 0), "non-sampled"),
        ):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                laps.join(data, [*normal, invalid])
