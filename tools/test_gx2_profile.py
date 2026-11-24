import unittest

import gx2_profile as profile


def row(seq, phase, start, *, pacing=0, presented=1, capture=0, worker=0):
    timings = [0] * len(profile.STAGES)
    timings[profile.STAGES.index("screenshot")] = capture
    counters = [presented, 0, 900, 12000, 0, 5000, 1000, 4000, 40]
    workers = [worker, worker, 0, 0, 0, 0, 0, 0]
    return profile.ROW.pack(seq, phase, 0, start, 16, pacing, *timings, *counters, *workers)


def capture(*rows, total=None):
    return profile.HEADER.pack(
        0x485A5031, 1, 1000, profile.ROW.size, 4096, len(rows),
        len(rows) if total is None else total,
    ) + b"".join(rows)


class ProfileTests(unittest.TestCase):
    def test_hzp2_counts_builds_and_display_results_independently(self):
        def v2(sequence, *, paired, partial, gamepad, flips, interval):
            legacy = list(profile.ROW.unpack(row(sequence, 5, sequence * 16, presented=paired)))
            legacy[6 + len(profile.STAGES) + 1] = partial
            timings = [1, 2, 3, 4, 5, 6, 7, 8]
            counters = [1, 1, gamepad, 1, flips, interval, 10, 9, 4, 3, 1280, 720, 854, 480]
            return profile.V2_ROW.pack(*legacy, *timings, *counters)

        header = profile.HEADER.pack(0x485A5032, 2, 1000, 256, 4096, 3, 3)
        metadata, frames = profile.read(header + b"".join((
            v2(0, paired=1, partial=1, gamepad=1, flips=1, interval=0),
            v2(1, paired=0, partial=0, gamepad=0, flips=2, interval=0),
            v2(2, paired=1, partial=0, gamepad=1, flips=1, interval=17),
        )))
        self.assertEqual(metadata["version"], 2)
        self.assertEqual(frames[0]["solid_terrain_drc_ms"], 6)
        self.assertEqual(frames[0]["tv_sections_tested"], 10)
        self.assertEqual(frames[0]["drc_width"], 854)
        playing = profile.summarize(frames, 0, 1)["playing"]
        self.assertEqual(playing["complete_presentations"], 1)
        self.assertEqual(playing["partial_presentations"], 1)
        self.assertEqual(playing["submission_totals"], {
            "game_frames_built": 3, "tv_submissions": 3, "gamepad_submissions": 2,
            "gx2_swap_delta": 3, "gx2_flip_delta": 4, "paired_submissions": 2,
        })
        self.assertEqual(playing["paired_flip_interval_ms"]["median"], 17)
        self.assertEqual(playing["paired_flip_interval_ms"]["samples"], 1)

    def test_pacing_skips_screen_changes_and_unpresented_or_readback_iterations(self):
        _, frames = profile.read(capture(
            row(0, 3, 0), row(1, 5, 17), row(2, 5, 34, pacing=17),
            row(3, 5, 54, pacing=20, presented=0), row(4, 5, 74, pacing=20),
            row(5, 5, 180, pacing=106, capture=1), row(6, 5, 200, pacing=20),
            row(7, 5, 254, pacing=54),
        ))
        playing = profile.summarize(frames, 0, 10)["playing"]
        self.assertEqual(playing["host_loops"], 7)
        self.assertEqual(playing["presented_loops"], 5)
        self.assertEqual(playing["pacing_stalls"], 1)
        self.assertEqual(playing["frame_pacing_ms"], {
            "samples": 2, "median": 17, "p95": 54, "p99": 54, "max": 54,
        })

    def test_reads_wrapped_counters_and_the_retained_tail(self):
        metadata, frames = profile.read(capture(
            row(398, 1, 50, worker=2**32 - 3), row(399, 1, 60, worker=2), total=400,
        ))
        self.assertEqual(metadata["retained"], 2)
        self.assertIsNone(frames[0]["streaming_worker_ms"])
        self.assertEqual(frames[1]["streaming_worker_count"], 5)
        self.assertEqual(frames[1]["streaming_worker_ms"], 5)

    def test_rejects_a_truncated_capture_or_inconsistent_sequence(self):
        with self.assertRaisesRegex(ValueError, "length"):
            profile.read(capture(row(0, 1, 0))[:-1])
        with self.assertRaisesRegex(ValueError, "indices"):
            profile.read(capture(row(1, 1, 0)))
        with self.assertRaisesRegex(ValueError, "unsupported"):
            profile.read(profile.HEADER.pack(0x485A5031, 2, 1000, 256, 4096, 0, 0))


if __name__ == "__main__":
    unittest.main()
