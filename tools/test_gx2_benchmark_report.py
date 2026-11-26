import unittest

import gx2_benchmark_report as audit


def metric(value):
    return {"n": 1, "mean": value, "min": value, "max": value, "stdev": None, "ci95": None}


def benchmark(scenario="village", **coverage):
    phase = "village" if scenario == "cafe-diagnostic-village" else scenario
    counts = {
        "host_frames": 80,
        "paired_frames": 80,
        "terrain_paired_frames": 80,
        "eligible_intervals": 79,
        "paired_terrain_intervals": 79,
        "longest_ineligible_run_frames": 0,
        "elapsed_ms": 12_300,
        "eligible_interval_ms": 12_000,
        **coverage,
    }
    return {
        "format": "horizon-benchmark-report",
        "version": 1,
        "kind": "game",
        "conditions": {"backend": "gx2", "scenario": scenario},
        "metrics": {
            **{f"cafe.{phase}.{name}": metric(value) for name, value in counts.items()},
            "cafe.configured_scale": metric(75),
            "cafe.effective_scale": metric(100),
            "cafe.world_surface_bytes": metric(0),
        },
        "frames": [{
            "phase": phase,
            "repeat": 0,
            "frames": counts["paired_terrain_intervals"],
            "seconds": counts["eligible_interval_ms"] / 1000,
            "p50_ms": 150.0,
            "p95_ms": 160.0,
            "p99_ms": 160.0,
        }],
    }


def village_frame(sequence, drawn):
    return {
        "sequence": sequence,
        "phase": "playing",
        "start_ms": sequence * 150,
        "loop_ms": 15,
        "tv_submissions": 1,
        "gamepad_submissions": 1,
        "tv_sections_drawn": drawn,
        "drc_sections_drawn": drawn,
        "tv_sections_tested": 12,
        "drc_sections_tested": 11,
        "terrain_upload_bytes": 3500,
    }


class BenchmarkReportTests(unittest.TestCase):
    def test_reports_fallback_and_coverage_separately(self):
        result = audit.summarize(benchmark())
        self.assertEqual(result["targets"]["configured_scale"], 75)
        self.assertEqual(result["targets"]["effective_scale"], 100)
        self.assertEqual(result["targets"]["world_surface_bytes"], 0)
        self.assertEqual(result["coverage"]["evidence"], "uninterrupted")
        self.assertAlmostEqual(result["coverage"]["eligible_wall_fraction"], 12000 / 12300)
        self.assertNotIn("gpu_p50_ms", result["frame_series"])

    def test_completed_legacy_report_has_missing_evidence(self):
        report = benchmark()
        del report["metrics"]["cafe.village.elapsed_ms"]
        result = audit.summarize(report)
        self.assertEqual(result["coverage"]["evidence"], "missing")
        self.assertEqual(result["coverage"]["missing"], ["elapsed_ms"])
        self.assertNotIn("eligible_wall_fraction", result["coverage"])

    def test_relaxed_route_retains_its_scenario_and_shows_gaps(self):
        report = benchmark(
            "cafe-diagnostic-village", terrain_paired_frames=3, eligible_intervals=2,
            paired_terrain_intervals=2, eligible_interval_ms=300, longest_ineligible_run_frames=77,
        )
        result = audit.summarize(report)
        self.assertEqual(result["scenario"], "cafe-diagnostic-village")
        self.assertEqual(result["coverage"]["evidence"], "gaps")
        self.assertAlmostEqual(result["coverage"]["eligible_wall_fraction"], 300 / 12300)

    def test_village_profile_keeps_uploads_and_tested_sections_during_terrain_loss(self):
        result = audit.summarize(benchmark(
            host_frames=5, paired_frames=5, terrain_paired_frames=3,
            eligible_intervals=2, paired_terrain_intervals=2,
            eligible_interval_ms=300, longest_ineligible_run_frames=2,
        ))
        rows = [village_frame(seq, 5 if seq < 103 else 0) for seq in range(100, 105)]
        trace = (
            "Cafe Village streaming seq=103 elapsed_ms=450 reason=lost camera=-960.00,87.20"
            " sections=0/0 columns=60 client_pending=12 uploads=9 remote_pending=2"
            " forward_resident=3 forward_lit=1"
        )
        profile = audit.village_profile(result, rows, serial=[trace, trace])
        self.assertEqual(profile["first_sequence"], 100)
        self.assertEqual(profile["last_sequence"], 104)
        self.assertEqual(profile["eligible_ms_from_profile_starts"], 300)
        gap, = profile["ineligible_runs"]
        self.assertEqual(gap["frames"], 2)
        self.assertEqual(gap["start_offset_ms"], 450)
        self.assertEqual(gap["sections_tested_at_start"], [12, 11])
        self.assertEqual(gap["terrain_upload_vertex_bytes"], 7_000)
        sample, = profile["forward_streaming_samples"]
        self.assertEqual(sample["camera_xz"], [-960.0, 87.2])
        self.assertEqual(sample["forward_resident_mask"], 3)
        self.assertEqual(sample["forward_lit_mask"], 1)
        with self.assertRaisesRegex(ValueError, "sections disagree"):
            audit.village_profile(result, rows, serial=[trace.replace("sections=0/0", "sections=3/0")])

    def test_village_serial_rejects_wrong_edge_or_incompatible_frame_clock(self):
        rows = [village_frame(100, 5), village_frame(101, 0)]
        first = (
            "Cafe Village streaming seq=100 elapsed_ms=12 reason=progress camera=-960.00,87.20"
            " sections=5/5 columns=60 client_pending=12 uploads=9 remote_pending=2"
            " forward_resident=3 forward_lit=1"
        )
        loss = first.replace("seq=100 elapsed_ms=12 reason=progress", "seq=101 elapsed_ms=162 reason=lost")
        loss = loss.replace("sections=5/5", "sections=0/0")
        samples = audit.village_streaming_samples(rows, [first, loss])
        self.assertEqual([sample["reason"] for sample in samples], ["progress", "lost"])
        with self.assertRaisesRegex(ValueError, "reason disagrees"):
            audit.village_streaming_samples(rows, [first, loss.replace("reason=lost", "reason=recovered")])
        with self.assertRaisesRegex(ValueError, "timing does not match"):
            audit.village_streaming_samples(rows, [first, loss.replace("elapsed_ms=162", "elapsed_ms=100")])

    def test_village_profile_rejects_ambiguous_or_mismatched_retained_frames(self):
        result = audit.summarize(benchmark(
            host_frames=3, paired_frames=3, terrain_paired_frames=3,
            eligible_intervals=2, paired_terrain_intervals=2, eligible_interval_ms=300,
        ))
        rows = [village_frame(seq, 5) for seq in range(100, 105)]
        with self.assertRaisesRegex(ValueError, "multiple Village windows"):
            audit.village_profile(result, rows)
        self.assertEqual(audit.village_profile(result, rows, 101)["last_sequence"], 103)
        rows[2]["gamepad_submissions"] = 0
        with self.assertRaisesRegex(ValueError, "no retained gameplay window"):
            audit.village_profile(result, rows, 101)

    def test_village_profile_rejects_empty_measurements(self):
        with self.assertRaisesRegex(ValueError, "requires measured host frames"):
            audit.village_window([], {"host_frames": 0})

    def test_village_profile_rejects_equal_counts_with_other_frame_timing(self):
        result = audit.summarize(benchmark(
            host_frames=3, paired_frames=3, terrain_paired_frames=3,
            eligible_intervals=2, paired_terrain_intervals=2, eligible_interval_ms=306,
        ))
        rows = [village_frame(seq, 5) for seq in range(100, 107)]
        rows[0]["start_ms"], rows[1]["start_ms"], rows[2]["start_ms"] = 100, 150, 200
        rows[3]["phase"] = "menu"
        self.assertEqual(audit.village_profile(result, rows)["first_sequence"], 104)

    def test_stationary_reports_the_existing_coverage_threshold(self):
        for wall, expected in ((24_000, True), (23_999, False)):
            with self.subTest(wall=wall):
                report = benchmark("cafe-stationary", elapsed_ms=30_000, eligible_interval_ms=wall)
                self.assertIs(
                    audit.summarize(report)["coverage"]["meets_stationary_coverage"], expected,
                )

    def test_stationary_camera_and_section_counts(self):
        report = benchmark("cafe-stationary", elapsed_ms=30_000, eligible_interval_ms=25_000)
        self.assertNotIn("camera", audit.summarize(report))
        camera = {"x": -960.0, "y": 126.62, "z": 80.0, "yaw": 0.0, "pitch": 35.0}
        metrics = report["metrics"]
        metrics.update({f"cafe.stationary.camera.{key}": metric(value)
                        for key, value in camera.items()})
        sections = {"tv_before": 562, "tv_after": 564, "gamepad_before": 560, "gamepad_after": 563}
        for name, value in sections.items():
            output, boundary = name.split("_")
            metrics[f"cafe.stationary.{output}_sections_{boundary}"] = metric(value)
        result = audit.summarize(report)
        self.assertEqual(result["camera"], camera)
        self.assertEqual(result["terrain_sections"], sections)
        del metrics["cafe.stationary.camera.yaw"]
        with self.assertRaisesRegex(ValueError, "incomplete stationary camera"):
            audit.summarize(report)

    def test_contradictory_frame_counts_are_rejected(self):
        report = benchmark(eligible_intervals=78)
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            audit.summarize(report)


def stationary(paired=301, p50=90.0, p95=103.0, p99=130.0):
    report = benchmark(
        "cafe-stationary", host_frames=paired, paired_frames=paired, terrain_paired_frames=paired,
        eligible_intervals=paired - 1, paired_terrain_intervals=paired - 1,
        elapsed_ms=30_000, eligible_interval_ms=29_900,
    )
    report["app"] = {"version": "0.8.0-rc.38"}
    report["hardware"] = {"arch": "powerpc"}
    report["conditions"].update({"seed": "1234", "render_distance": 6, "simulation_distance": 5})
    targets = {
        "configured_scale": 100,
        "effective_scale": 100,
        "tv_width": 1920,
        "tv_height": 1080,
        "gamepad_width": 854,
        "gamepad_height": 480,
        "tv_world_width": 1920,
        "tv_world_height": 1079,
        "gamepad_world_width": 854,
        "gamepad_world_height": 480,
        "world_surface_bytes": 0,
    }
    camera = {"x": -960.0, "y": 126.62, "z": 80.0, "yaw": 0.0, "pitch": 35.0}
    report["metrics"].update({f"cafe.{key}": metric(value) for key, value in targets.items()})
    report["metrics"]["cafe.entity_distance_percent"] = metric(50)
    report["metrics"]["cafe.omitted_pass_mask"] = metric(33280)
    report["metrics"].update({f"cafe.stationary.camera.{key}": metric(value) for key, value in camera.items()})
    for output in ("tv", "gamepad"):
        for boundary in ("before", "after"):
            report["metrics"][f"cafe.stationary.{output}_sections_{boundary}"] = metric(562)
    report["frames"][0].update({"p50_ms": p50, "p95_ms": p95, "p99_ms": p99})
    return report


class StationaryComparisonTests(unittest.TestCase):
    def test_same_workload_reports_wall_cadence_and_tail_deltas(self):
        control = stationary(paired=300)
        trial = stationary(paired=375, p50=81.0, p95=95.0, p99=140.0)
        comparison = audit.compare_stationary(control, trial)
        self.assertEqual(comparison["control"]["paired_submissions_per_s"], 10)
        self.assertEqual(comparison["trial"]["paired_submissions_per_s"], 12.5)
        self.assertEqual(comparison["trial_minus_control"]["elapsed_per_pair_ms"], -20)
        self.assertEqual(comparison["trial_minus_control"]["p95_ms"], -8)
        self.assertEqual(comparison["trial_minus_control"]["p99_ms"], 10)
        self.assertEqual(comparison["trial"]["gap_to_30_hz_ms"], 46.667)

    def test_rejects_different_camera_and_inconsistent_terrain(self):
        control = stationary()
        trial = stationary(paired=315)
        trial["metrics"]["cafe.stationary.camera.yaw"] = metric(1)
        with self.assertRaisesRegex(ValueError, "mismatched camera"):
            audit.compare_stationary(control, trial)
        trial = stationary(paired=315)
        trial["metrics"]["cafe.stationary.tv_sections_after"] = metric(563)
        with self.assertRaisesRegex(ValueError, "stable nonzero section counts"):
            audit.compare_stationary(control, trial)
        trial["metrics"]["cafe.stationary.tv_sections_before"] = metric(563)
        with self.assertRaisesRegex(ValueError, "mismatched terrain_sections"):
            audit.compare_stationary(control, trial)

    def test_rejects_scale_fallback_and_undercovered_runs(self):
        control = stationary()
        trial = stationary(paired=315)
        trial["metrics"]["cafe.configured_scale"] = metric(75)
        with self.assertRaisesRegex(ValueError, "fixed-scale target evidence"):
            audit.compare_stationary(control, trial)
        trial = stationary(paired=315)
        trial["metrics"]["cafe.cafe-stationary.eligible_interval_ms"] = metric(23_999)
        with self.assertRaisesRegex(ValueError, "stationary coverage gate"):
            audit.compare_stationary(control, trial)

    def test_rejects_mismatched_settings_and_a_repeated_capture(self):
        control = stationary()
        trial = stationary(paired=315)
        trial["conditions"]["render_distance"] = 8
        with self.assertRaisesRegex(ValueError, "mismatched or missing conditions"):
            audit.compare_stationary(control, trial)
        with self.assertRaisesRegex(ValueError, "distinct captures"):
            audit.compare_stationary(control, control)

    def test_rejects_changed_render_work(self):
        control = stationary()
        trial = stationary(paired=315)
        trial["metrics"]["cafe.omitted_pass_mask"] = metric(0)
        with self.assertRaisesRegex(ValueError, "mismatched or missing cafe.omitted_pass_mask"):
            audit.compare_stationary(control, trial)
