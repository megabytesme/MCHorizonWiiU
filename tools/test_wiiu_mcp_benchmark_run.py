import base64
from contextlib import contextmanager
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from test_wiiu_mcp_benchmark_capture import fixture_capture
from wiiu_mcp_benchmark_capture import CHUNK_BYTES
from wiiu_mcp_benchmark_run import BenchmarkWaitTimeout, collect, main


INSTANCE = "fixture"
SCENARIO = "cafe-artifact-smoke"
PROFILE = "4q"
REPORT = "game-20261126201800-cafe-artifact-smoke.json"
CAPTURE = "game-20261126201800-cafe-artifact-smoke-4q.hzp"


def row(phase, *, home=False, **overrides):
    state = {
        "phase": phase, "instance": INSTANCE if phase != "idle" else None,
        "scenario": SCENARIO if phase != "idle" else None,
        "profile": PROFILE if phase != "idle" else None,
        "report_id": REPORT if phase == "completed" else None,
        "capture_id": CAPTURE if phase == "completed" else None,
        "night_home_ready": home, "cancel_requested": False,
    }
    return state | overrides


def text_reply(data):
    return {"content": [{"type": "text", "text": json.dumps(data)}]}


class Fixture:
    def __init__(self, testcase, states=(), *, existing=(), installation=INSTANCE):
        self.test = testcase
        self.calls = []
        self.now = 0.0
        self.states = [row("idle", home=True), *states]
        self.existing = list(existing)
        self.installation = installation
        self.binary = fixture_capture()
        self.report = {
            "format": "horizon-benchmark-report", "version": 1, "kind": "game",
            "conditions": {"scenario": SCENARIO}, "metrics": {"paired_fps": {"mean": 27.0}},
        }
        self.start = row("queued", home=True)
        self.report_reply = {"instance": INSTANCE, "id": REPORT, "report": self.report}
        self.receipt = {"host_seconds": {}, "observations": [], "terminal_status": None}

    def call(self, tool, args=None):
        self.calls.append((tool, args))
        if tool == "wiiu_benchmark_status":
            return text_reply(self.states.pop(0) if len(self.states) > 1 else self.states[0])
        if tool == "wiiu_benchmark_reports":
            self.test.assertIsNone(args)
            return text_reply({"instance": self.installation, "reports": self.existing})
        if tool == "wiiu_benchmark_start":
            self.test.assertEqual(args, {"scenario": SCENARIO, "profile": PROFILE})
            return text_reply(self.start)
        if tool == "wiiu_benchmark_cancel":
            self.test.assertEqual(args, {"run_id": self.start["run_id"]})
            return text_reply(row("running", run_id=args["run_id"], cancel_requested=True))
        if tool == "wiiu_benchmark_report":
            self.test.assertEqual(args, {"instance": INSTANCE, "id": REPORT})
            self.now += 0.25
            return text_reply(self.report_reply)
        if tool == "wiiu_benchmark_capture":
            self.test.assertEqual((args["instance"], args["capture_id"]), (INSTANCE, CAPTURE))
            offset = args["offset"]
            block = self.binary[offset:offset + CHUNK_BYTES]
            end = offset + len(block)
            self.now += 0.5
            return text_reply({
                "capture_id": CAPTURE, "instance": INSTANCE, "format": "HZP3", "encoding": "base64",
                "offset": offset, "total_bytes": len(self.binary),
                "next_offset": end if end < len(self.binary) else None,
                "data": base64.b64encode(block).decode("ascii"),
            })
        self.test.fail(f"unexpected operation {tool}")

    def collect(self, timeout=10, call=None):
        def pause(seconds):
            self.now += seconds
        return collect(self.call if call is None else call, INSTANCE, SCENARIO, PROFILE, timeout,
                       self.receipt, clock=lambda: self.now, pause=pause)

    def tools(self):
        return [tool for tool, _ in self.calls]


class LifecycleTests(unittest.TestCase):
    def test_waits_through_delayed_completion_and_fetches_the_real_wire_shapes(self):
        pending = [row("queued"), row("loading"), row("running"), row("closing"), row("completed")]
        fixture = Fixture(self, [*pending, row("completed", home=True)])
        report_id, report, capture_id, data, metadata = fixture.collect()
        self.assertEqual((report_id, report, capture_id, bytes(data)), (REPORT, fixture.report, CAPTURE, fixture.binary))
        self.assertEqual(metadata["version"], 3)
        self.assertEqual(fixture.receipt["terminal_status"], row("completed", home=True))
        self.assertEqual(fixture.receipt["host_seconds"], {
            "start_request": 0, "terminal_observed_after_start": 3, "home_ready_after_start": 3.75,
            "report_retrieval": 0.25, "capture_retrieval": 1,
        })
        self.assertEqual(fixture.tools()[-3:], ["wiiu_benchmark_report", "wiiu_benchmark_capture",
                                               "wiiu_benchmark_capture"])

    def test_accepts_completion_between_status_polls(self):
        fixture = Fixture(self, [row("completed", home=True)])
        self.assertEqual(fixture.collect()[0], REPORT)

    def test_rejects_another_install_before_start(self):
        fixture = Fixture(self, installation="another")
        with self.assertRaisesRegex(RuntimeError, "selected a different installation"):
            fixture.collect()
        self.assertNotIn("wiiu_benchmark_start", fixture.tools())

    def test_rejects_inflight_benchmark_at_preflight(self):
        fixture = Fixture(self)
        fixture.states[0] = row("running")
        with self.assertRaisesRegex(RuntimeError, "idle Night Home"):
            fixture.collect()
        self.assertEqual(fixture.tools(), ["wiiu_benchmark_status"])

    def test_requires_queued_acknowledgement_before_tracking_the_run(self):
        for accepted in (row("loading"), row("queued", scenario="forest"), row("queued", cancel_requested=True)):
            with self.subTest(accepted=accepted):
                fixture = Fixture(self, [row("completed", home=True)])
                fixture.start = accepted
                with self.assertRaisesRegex(RuntimeError, "requested queued benchmark"):
                    fixture.collect()
                self.assertEqual(fixture.tools()[-1], "wiiu_benchmark_start")
                self.assertEqual(fixture.receipt["last_observed_status"], accepted)

    def test_rejects_another_run_or_backward_progress_without_fetch_or_cancel(self):
        for states in (
            [row("queued", profile="4r")],
            [row("completed", home=True, instance="another")],
            [row("running"), row("queued")],
            [row("completed"), row("queued")],
        ):
            with self.subTest(states=states):
                fixture = Fixture(self, states)
                with self.assertRaisesRegex(RuntimeError, "another benchmark|moved backward|replaced"):
                    fixture.collect()
                self.assertNotIn("wiiu_benchmark_cancel", fixture.tools())
                self.assertNotIn("wiiu_benchmark_report", fixture.tools())

    def test_provided_run_id_rejects_same_scenario_replacement(self):
        fixture = Fixture(self, [row("completed", home=True, run_id="b" * 32)])
        fixture.start = row("queued", run_id="a" * 32)
        with self.assertRaisesRegex(RuntimeError, "another benchmark"):
            fixture.collect()
        self.assertNotIn("wiiu_benchmark_report", fixture.tools())
        self.assertNotIn("wiiu_benchmark_cancel", fixture.tools())

    def test_provided_run_id_pins_complete_artifacts(self):
        run = "a" * 32
        fixture = Fixture(self, [row("loading", run_id=run), row("completed", home=True, run_id=run)])
        fixture.start = row("queued", run_id=run)
        _, _, _, capture, _ = fixture.collect()
        self.assertEqual(capture, fixture.binary)
        self.assertEqual(fixture.receipt["terminal_status"]["run_id"], run)

    def test_wait_timeout_cancels_only_a_returned_run_id(self):
        run = "a" * 32
        fixture = Fixture(self, [row("running", run_id=run)])
        fixture.start = row("queued", run_id=run)
        with self.assertRaises(BenchmarkWaitTimeout):
            fixture.collect(timeout=1.0)
        self.assertEqual(fixture.calls[-1], ("wiiu_benchmark_cancel", {"run_id": run}))
        self.assertEqual(fixture.receipt["cancellation_status"]["run_id"], run)
        self.assertTrue(fixture.receipt["cancellation_status"]["cancel_requested"])
        self.assertNotIn("wiiu_benchmark_report", fixture.tools())

    def test_wait_timeout_keeps_last_observation_and_leaves_the_run_alone(self):
        fixture = Fixture(self, [row("running")])
        with self.assertRaises(BenchmarkWaitTimeout):
            fixture.collect(timeout=1.0)
        self.assertEqual(fixture.receipt["last_observed_status"], row("running"))
        self.assertIsNone(fixture.receipt["terminal_status"])
        self.assertNotIn("wiiu_benchmark_cancel", fixture.tools())

    def test_timeout_retains_a_terminal_observation_while_home_is_unavailable(self):
        fixture = Fixture(self, [row("closing"), row("completed")])
        with self.assertRaises(BenchmarkWaitTimeout):
            fixture.collect(timeout=1.0)
        self.assertEqual(fixture.receipt["terminal_status"], row("completed"))
        self.assertNotIn("wiiu_benchmark_report", fixture.tools())

    def test_reports_noncompleted_terminal_without_an_artifact_request(self):
        for phase in ("cancelled", "failed", "aborted"):
            with self.subTest(phase=phase):
                state = row(phase, home=True)
                fixture = Fixture(self, [state])
                with self.assertRaisesRegex(RuntimeError, f"ended in phase {phase}"):
                    fixture.collect()
                self.assertEqual(fixture.receipt["terminal_status"], state)
                self.assertNotIn("wiiu_benchmark_report", fixture.tools())

    def test_revoked_grant_stops_on_the_first_failed_status(self):
        fixture = Fixture(self, [row("loading"), row("completed", home=True)])

        def call(tool, args=None):
            if fixture.tools().count("wiiu_benchmark_status") == 2:
                fixture.calls.append((tool, args))
                raise RuntimeError("Auth=notLoggedIn")
            return fixture.call(tool, args)

        with self.assertRaisesRegex(RuntimeError, "Auth=notLoggedIn"):
            fixture.collect(call=call)
        self.assertEqual(fixture.tools(), ["wiiu_benchmark_status", "wiiu_benchmark_reports",
                                           "wiiu_benchmark_start", "wiiu_benchmark_status", "wiiu_benchmark_status"])
        self.assertEqual(fixture.receipt["last_observed_status"], row("loading"))

    def test_invalid_or_preexisting_ids_stop_before_artifact_request(self):
        for state, existing in (
            (row("completed", home=True, report_id=REPORT.removesuffix(".json")), []),
            (row("completed", home=True, report_id="../" + REPORT), []),
            (row("completed", home=True, capture_id=CAPTURE.replace("4q", "4r")), []),
            (row("completed", home=True), [REPORT]),
        ):
            with self.subTest(state=state, existing=existing):
                fixture = Fixture(self, [state], existing=existing)
                with self.assertRaisesRegex(ValueError, "invalid report ID|capture ID|existed"):
                    fixture.collect()
                self.assertNotIn("wiiu_benchmark_report", fixture.tools())

    def test_checks_nested_report_association_before_reading_capture(self):
        for field, replacement in (("instance", "another"), ("id", "other.json"),
                                   ("report", {"format": "horizon-benchmark-report", "version": 1,
                                               "kind": "game", "conditions": {"scenario": "village"}})):
            with self.subTest(field=field):
                fixture = Fixture(self, [row("completed", home=True)])
                fixture.report_reply[field] = replacement
                with self.assertRaisesRegex(ValueError, "different benchmark report or installation|different scenario"):
                    fixture.collect()
                self.assertNotIn("wiiu_benchmark_capture", fixture.tools())

    def test_corrupt_capture_prevents_success(self):
        fixture = Fixture(self, [row("completed", home=True)])

        def call(tool, args=None):
            result = fixture.call(tool, args)
            if tool == "wiiu_benchmark_capture":
                text = json.loads(result["content"][0]["text"])
                text["next_offset"] = 1
                return text_reply(text)
            return result

        with self.assertRaisesRegex(ValueError, "next capture offset"):
            fixture.collect(call=call)
        self.assertEqual(fixture.receipt["terminal_status"], row("completed", home=True))


class CliTests(unittest.TestCase):
    def run_driver(self, output, fixture, *, timeout=False, entered=None):
        if entered is None:
            entered = []

        @contextmanager
        def session(profile, ca, server):
            entered.append((profile, ca, server))
            yield fixture

        args = ["driver", "--codex-home", "profile", "--ca", "ca", "--server", "wiiu",
                "--instance", INSTANCE, "--scenario", SCENARIO, "--profile", PROFILE,
                "--output", str(output)]
        with patch("sys.argv", args), patch("wiiu_mcp_benchmark_run.paired_session", session):
            if timeout:
                with patch("wiiu_mcp_benchmark_run.wait_for_terminal",
                           side_effect=BenchmarkWaitTimeout("fixture wait deadline")):
                    with self.assertRaises(SystemExit):
                        main()
            else:
                main()
        return entered

    def test_one_session_publishes_new_capture_report_and_receipt(self):
        fixture = Fixture(self, [row("completed", home=True)])
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "new"
            entered = self.run_driver(output, fixture)
            self.assertEqual(entered, [(Path("profile"), Path("ca"), "wiiu")])
            self.assertEqual(set(path.name for path in output.iterdir()), {REPORT, CAPTURE, "receipt.json"})
            self.assertEqual((output / CAPTURE).read_bytes(), fixture.binary)
            self.assertEqual(json.loads((output / REPORT).read_text()), fixture.report)
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "passed")
            self.assertEqual(receipt["terminal_status"], row("completed", home=True))
            before = (output / "receipt.json").read_bytes()
            entered = []
            with self.assertRaises(SystemExit):
                self.run_driver(output, fixture, entered=entered)
            self.assertEqual(entered, [])
            self.assertEqual((output / "receipt.json").read_bytes(), before)

    def test_timeout_writes_a_receipt_without_fabricating_a_terminal_phase(self):
        fixture = Fixture(self, [row("running")])
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "new"
            self.run_driver(output, fixture, timeout=True)
            self.assertEqual([path.name for path in output.iterdir()], ["receipt.json"])
            receipt = json.loads((output / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "timed_out")
            self.assertIsNone(receipt["terminal_status"])
            self.assertEqual(receipt["last_observed_status"], fixture.start)
            self.assertNotIn("wiiu_benchmark_cancel", fixture.tools())


if __name__ == "__main__":
    unittest.main()
