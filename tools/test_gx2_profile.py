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


if __name__ == "__main__":
    unittest.main()
