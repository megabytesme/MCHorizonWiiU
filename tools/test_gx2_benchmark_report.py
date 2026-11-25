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

    def test_stationary_reports_the_existing_coverage_threshold(self):
        for wall, expected in ((24_000, True), (23_999, False)):
            with self.subTest(wall=wall):
                report = benchmark("cafe-stationary", elapsed_ms=30_000, eligible_interval_ms=wall)
                self.assertIs(
                    audit.summarize(report)["coverage"]["meets_stationary_coverage"], expected,
                )

    def test_contradictory_frame_counts_are_rejected(self):
        report = benchmark(eligible_intervals=78)
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            audit.summarize(report)
