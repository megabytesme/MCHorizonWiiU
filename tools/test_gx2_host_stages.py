import unittest

from gx2_host_stages import analyze
from gx2_profile import (
    COUNTERS, HEADER, STAGES, V2_COUNTERS, V2_STAGES, V3_COUNTERS, V3_ROW,
    V3_STAGES, WORKERS, read,
)


def hzp_frame(sequence, start, **values):
    keys = (
        ("sequence", "phase", "clock_high", "clock_low", "loop", "pacing")
        + STAGES + COUNTERS
        + tuple(key for worker in WORKERS for key in (f"{worker}_count", f"{worker}_total"))
        + V2_STAGES + V2_COUNTERS + V3_STAGES + V3_COUNTERS
    )
    defaults = {
        "sequence": sequence, "phase": 5, "clock_low": start, "loop": 20,
        "presented": 1, "tv_submissions": 1, "gamepad_submissions": 1,
        "tv_sections_drawn": 174, "drc_sections_drawn": 174,
    }
    fields = {**defaults, **values}
    unknown = fields.keys() - set(keys)
    if unknown:
        raise ValueError(unknown)
    return V3_ROW.pack(*(fields.get(key, 0) for key in keys))


def run(*frames):
    header = HEADER.pack(0x485A5033, 3, 1000, V3_ROW.size, 4096, len(frames), 8 + len(frames))
    return read(header + b"".join(frames))[1]


class HostStagesTests(unittest.TestCase):
    def test_nested_intervals_remain_wall_spans_and_worker_work_is_separate(self):
        rows = run(
            hzp_frame(8, 0, submit_tv=8, submit_drc=4, hud_submit_tv=3, hud_submit_drc=1,
                      wait_flip=2, finish_present=1, terrain_wait=1,
                      streaming=1, tick=3, world_update=4, terrain_upload=1,
                      terrain_upload_bytes=128, mesh_worker_count=9, mesh_worker_total=100),
            hzp_frame(9, 20, loop=40, submit_tv=9, submit_drc=6, hud_submit_tv=5,
                      hud_submit_drc=2, world_update=2, mesh_worker_count=11,
                      mesh_worker_total=180),
        )
        result = analyze(rows, 8, 9)["cohorts"]["paired_with_terrain"]
        spans = result["spans_ms"]
        self.assertEqual(spans["host_loop"]["mean"], 30)
        self.assertEqual(spans["graphics_submit_including_hud"]["mean"], 13.5)
        self.assertEqual(spans["hud_submit_including_internal_sync"]["mean"], 5.5)
        self.assertEqual(spans["graphics_submit_outside_hud"]["mean"], 8)
        self.assertEqual(spans["explicit_sync_api_wall"]["mean"], 2)
        self.assertEqual(spans["world_update"]["mean"], 3)
        self.assertEqual(spans["streaming"]["mean"], 0.5)
        self.assertEqual(spans["tick"]["mean"], 1.5)
        self.assertEqual(result["mesh_upload"]["active_loops"], 1)
        self.assertEqual(result["mesh_upload"]["vertex_bytes"], 128)
        self.assertEqual(result["mesh_upload"]["active_loop_ms"]["mean"], 1)
        self.assertEqual(result["parallel_worker_completions"]["mesh_worker"],
                         {"available_intervals": 1, "jobs": 2, "completed_wall_ms": 80})
        self.assertIsNone(result["parallel_worker_completions"]["integrated_tick"])
        self.assertEqual(result["largest_host_loop"],
                         {"sequence": 9, "ms": 40, "next_cohort_frame_is_adjacent": False})

    def test_cadence_does_not_bridge_screenshots_or_missing_terrain(self):
        rows = run(
            hzp_frame(8, 0, partial=1),
            hzp_frame(9, 20, screenshot=5),
            hzp_frame(10, 40, tv_sections_drawn=0, drc_sections_drawn=0),
            hzp_frame(11, 60, phase=6),
            hzp_frame(12, 80, presented=0, gamepad_submissions=0),
            hzp_frame(13, 98),
            hzp_frame(14, 116),
        )
        result = analyze(rows, 8, 14)
        self.assertEqual(result["excluded_screenshot"], 1)
        self.assertEqual(result["excluded_other_phase"], 1)
        self.assertEqual(result["client_update_spans_observed"], [])
        self.assertEqual(result["worker_spans_observed"], [])
        cohorts = result["cohorts"]
        self.assertEqual(cohorts["paired_with_terrain"]["loops"], 3)
        self.assertEqual(cohorts["paired_with_terrain"]["partial_content_loops"], 1)
        self.assertEqual(cohorts["paired_missing_terrain"]["loops"], 1)
        self.assertEqual(cohorts["without_one_pair"]["loops"], 1)
        self.assertIsNone(cohorts["paired_missing_terrain"]["spans_ms"]["world_update"])
        self.assertEqual(cohorts["paired_with_terrain"]["adjacent_frame_start_interval_ms"]["samples"], 1)
        self.assertEqual(cohorts["all_gameplay"]["adjacent_frame_start_interval_ms"]["samples"], 2)

    def test_impossible_nesting_fails_instead_of_hiding_bad_accounting(self):
        rows = run(hzp_frame(8, 0, submit_tv=3, hud_submit_tv=4))
        with self.assertRaisesRegex(ValueError, "enclosing tv submit at 8"):
            analyze(rows, 8, 8)

    def test_rejects_unretained_bounds_and_empty_gameplay(self):
        rows = run(hzp_frame(8, 0, phase=6))
        with self.assertRaisesRegex(ValueError, "entirely inside"):
            analyze(rows, 7, 8)
        with self.assertRaisesRegex(ValueError, "no screenshot-free gameplay"):
            analyze(rows, 8, 8)


if __name__ == "__main__":
    unittest.main()
