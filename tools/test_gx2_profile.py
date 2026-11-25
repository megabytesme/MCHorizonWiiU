import base64
import json
from pathlib import Path
import struct
from tempfile import TemporaryDirectory
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


def gpu_serial(sequence, endpoints, *, reported_tv=None, endpoints_only=False):
    tv_top, tv_bottom, pad_top, pad_bottom = endpoints
    span = (
        f"Horizon GX2 GPU span source=gx2-top-bottom hz=27000000 seq={sequence}"
        f" tv_ticks={tv_bottom - tv_top if reported_tv is None else reported_tv}"
        f" drc_ticks={pad_bottom - pad_top} valid=3 unpaired=0\n"
    )
    line = (
        f"Horizon GX2 GPU endpoints seq={sequence} tv={tv_top:016x}/{tv_bottom:016x}"
        f" drc={pad_top:016x}/{pad_bottom:016x}\n"
    )
    return line if endpoints_only else span + line


def gpu_capture():
    rows = []
    for sequence, scale in enumerate([100, 50, 50, 50, 50, 50]):
        legacy = profile.ROW.unpack(row(sequence, 5, sequence * 16, capture=int(sequence == 2)))
        extra = [0] * len(profile.V2_STAGES) + [
            1, 1, 1, 1, 1, 0, 10, 9, 4, 3, 1920, 1080, 854, 480,
        ]
        worlds = [0] * len(profile.V3_STAGES) + [
            scale, scale, 1920 * scale // 100, 1080 * scale // 100,
            854 * scale // 100, 480 * scale // 100, 0, 0,
        ]
        rows.append(profile.V3_ROW.pack(*legacy, *extra, *worlds))
    count = len(rows)
    header = profile.HEADER.pack(0x485A5033, 3, 1000, profile.V3_ROW.size, 4096, count, count)
    return profile.read(header + b"".join(rows))[1]


def uwp_event(sequence, payload=None):
    event = {"seq": sequence, "time": "2026-11-24T20:20:01Z", "port": "COM3",
             "kind": "capture_start" if payload is None else "rx"}
    if payload is not None:
        event["bytes"] = base64.b64encode(payload).decode("ascii")
    return json.dumps(event) + "\n"


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

    def test_hzp3_measures_world_targets_separately_from_output_surfaces(self):
        original = row(0, 5, 0)
        output = struct.pack(">22I", *([0] * 18), 1920, 1080, 854, 480)
        world = struct.pack(">24I", *range(16), 75, 75, 1440, 810, 636, 358, 11018240, 512)
        header = profile.HEADER.pack(0x485A5033, 3, 1000, 352, 4096, 1, 1)
        metadata, frames = profile.read(header + original + output + world)
        playing = profile.summarize(frames, 0, 1)["playing"]
        self.assertEqual(metadata["version"], 3)
        self.assertEqual(frames[0]["tv_width"], 1920)
        self.assertEqual(playing["world_targets"]["tv_world_width"]["median"], 1440)
        self.assertEqual(playing["world_targets"]["world_effective_scale"]["median"], 75)
        self.assertEqual(playing["workload"]["translucent_index_upload_bytes"]["median"], 512)
        self.assertEqual(playing["stages_ms_on_presented_loops"]["world_entities_drc"]["median"], 15)

    def test_reads_wrapped_counters_and_the_retained_tail(self):
        metadata, frames = profile.read(capture(
            row(398, 1, 50, worker=2**32 - 3), row(399, 1, 60, worker=2), total=400,
        ))
        self.assertEqual(metadata["retained"], 2)
        self.assertIsNone(frames[0]["streaming_worker_ms"])
        self.assertEqual(frames[1]["streaming_worker_count"], 5)
        self.assertEqual(frames[1]["streaming_worker_ms"], 5)

    def test_gpu_serial_join_preserves_overlap_and_scale_without_summing_spans(self):
        frames = gpu_capture()
        serial = "".join([
            gpu_serial(0, [27, 108_027, 81_027, 162_027]),
            gpu_serial(1, [600_000, 627_000, 654_000, 681_000]),
            gpu_serial(2, [800_000, 827_000, 854_000, 881_000]),
            gpu_serial(3, [900_000, 927_000, 954_000, 981_000], endpoints_only=True),
            gpu_serial(4, [1_000_000, 1_027_000, 1_054_000, 1_081_000], reported_tv=1),
            gpu_serial(5, [1_100_000, 1_127_000, 1_154_000, 1_181_000]),
            gpu_serial(100, [1_200_000, 1_227_000, 1_254_000, 1_281_000]),
        ])
        result = profile.correlate_gpu_spans(frames, serial.splitlines(), 0, 0.075)
        self.assertFalse(result["qualified_for_drs"])
        self.assertEqual(result["matched_samples"], 2)
        self.assertEqual(result["excluded_samples"], dict(
            missing_serial_line=1, not_retained=1, outside_interval=1,
            ineligible_frame=1, invalid_endpoints=1,
        ))
        tv_and_pad, sequential = result["samples"]
        self.assertEqual(tv_and_pad["world_dimensions_tv_gamepad"], [[1920, 1080], [854, 480]])
        self.assertEqual(tv_and_pad["endpoint_order"], "overlap")
        self.assertAlmostEqual(tv_and_pad["endpoint_overlap_ms"], 1.0)
        self.assertAlmostEqual(tv_and_pad["tv_span_ms"], 4.0)
        self.assertAlmostEqual(tv_and_pad["gamepad_span_ms"], 3.0)
        self.assertEqual(sequential["endpoint_order"], "tv_then_gamepad")
        self.assertAlmostEqual(sequential["endpoint_gap_ms"], 1.0)
        self.assertEqual(result["by_effective_scale"]["50"]["tv_span_ms"]["median"], 1.0)

    def test_gpu_serial_join_refuses_conflicting_sequences_and_deduplicates_copies(self):
        frames = gpu_capture()
        first = gpu_serial(0, [27, 108_027, 81_027, 162_027])
        self.assertEqual(profile.correlate_gpu_spans(frames, [first, first], 0, 1)["matched_samples"], 1)
        different = gpu_serial(0, [100, 120, 130, 150])
        with self.assertRaisesRegex(ValueError, "conflicting GPU serial records"):
            profile.correlate_gpu_spans(frames, [first, different], 0, 1)

    def test_uwp_serial_capture_restores_split_lines_across_segments(self):
        trace = gpu_serial(0, [27, 108_027, 81_027, 162_027]).replace("\n", "\r\n").encode()
        with TemporaryDirectory() as folder:
            root = Path(folder)
            marker = b"Horizon Wii U startup: Cafe Game handoff begin\r"
            first = marker + b"\x1b[36m" + trace[:19]
            (root / "serial-00000001.jsonl").write_text(uwp_event(1) + uwp_event(2, first), encoding="utf-8")
            (root / "serial-00000002.jsonl").write_text(uwp_event(3, trace[19:]), encoding="utf-8")
            lines = profile.one_uwp_game_session(profile.uwp_serial_lines(root))
            report = profile.correlate_gpu_spans(gpu_capture(), lines, 0, 1)
        self.assertEqual(report["matched_samples"], 1)
        self.assertEqual(report["samples"][0]["tv_span_ms"], 4.0)

    def test_uwp_serial_capture_rejects_two_games_and_accepts_a_bounded_run(self):
        start = b"Horizon Wii U startup: Cafe Game handoff begin\r"
        previous = gpu_serial(0, [100, 120, 130, 150]).encode()
        next_game = gpu_serial(0, [27, 108_027, 81_027, 162_027]).encode()
        with TemporaryDirectory() as folder:
            path = Path(folder) / "serial-00000001.jsonl"
            path.write_text(
                uwp_event(10, start + previous + b"hosted save barrier completed\rpartial")
                + uwp_event(11, b" message\r" + start + next_game)
                + uwp_event(12, b"hosted save barrier completed\r"),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "more than one game session"):
                list(profile.one_uwp_game_session(profile.uwp_serial_lines(path)))
            lines = list(profile.one_uwp_game_session(profile.uwp_serial_lines(path, 11, 12)))
        self.assertEqual(lines[0], start.decode().rstrip())
        report = profile.correlate_gpu_spans(gpu_capture(), lines, 0, 1)
        self.assertEqual(report["matched_samples"], 1)
        self.assertEqual(report["samples"][0]["tv_span_ms"], 4.0)

    def test_plain_serial_reader_rejects_raw_uwp_jsonl(self):
        with self.assertRaisesRegex(ValueError, "requires --gpu-serial-capture"):
            list(profile.plain_serial_lines([uwp_event(1)]))
        lines = ["[11:22:33.456] [OSConsole] Horizon GX2 capture enabled\n", '{"phase":"menu"}\n']
        self.assertEqual(list(profile.plain_serial_lines(lines)), lines)

    def test_rejects_a_truncated_capture_or_inconsistent_sequence(self):
        with self.assertRaisesRegex(ValueError, "length"):
            profile.read(capture(row(0, 1, 0))[:-1])
        with self.assertRaisesRegex(ValueError, "indices"):
            profile.read(capture(row(1, 1, 0)))
        with self.assertRaisesRegex(ValueError, "unsupported"):
            profile.read(profile.HEADER.pack(0x485A5031, 2, 1000, 256, 4096, 0, 0))


if __name__ == "__main__":
    unittest.main()
