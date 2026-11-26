import json
import unittest

from wiiu_mcp_lifecycle_test import qualify


def text_reply(value):
    if not isinstance(value, str):
        value = json.dumps(value)
    return {"content": [{"type": "text", "text": value}]}


class NightFixture:
    def __init__(self):
        self.launch_id = "1" * 32
        self.startup = None
        self.phase = "idle"
        self.mode = None
        self.calls = []
        self.saved_reports = ["earlier-report.json"]

    def call(self, tool, args=None):
        self.calls.append((tool, args))
        if tool == "play":
            self.startup = "loading"
            return text_reply(f"Horizon is starting.\nLaunch ID: {self.launch_id}")
        if tool == "wiiu_loading_cancel":
            if args != {"launch_id": self.launch_id}:
                raise ValueError("wrong startup ownership")
            self.startup = "cancelled"
        if tool == "wiiu_loading_status":
            return text_reply({"phase": self.startup})
        if tool == "wiiu_benchmark_reports":
            return text_reply({"reports": self.saved_reports})
        if tool == "wiiu_benchmark_start":
            self.mode = args["profile"]
            self.phase = "loading"
        if tool == "wiiu_benchmark_cancel":
            self.phase = "cancelled"
        return text_reply({"phase": self.phase, "night_home_ready": self.phase != "loading",
                           "profile": self.mode, "capture_id": None, "report_id": None,
                           "instance": "default"})


class NightLifecycleTests(unittest.TestCase):
    def test_cancel_returns_to_home_before_starting_next_benchmark(self):
        device = NightFixture()
        result = qualify(device.call, "Reopen Fixture", "default", 1, close=True)
        self.assertEqual(result["ordinary_cancel_phases"], ["loading", "cancelled"])
        self.assertEqual([trial["profile"] for trial in result["stationary_cancel_trials"]], ["4q", "4r"])
        self.assertTrue(result["reports_unchanged"])
        self.assertEqual(device.calls[-1], ("wiiu_close_to_menu", None))

    def test_existing_benchmark_is_left_running(self):
        device = NightFixture()
        device.phase = "running"
        with self.assertRaisesRegex(RuntimeError, "idle Night Home"):
            qualify(device.call, "Reopen Fixture", "default", 1, close=True)
        self.assertEqual(device.calls, [("wiiu_benchmark_status", None)])

    def test_lost_trial_response_attempts_pending_cancel(self):
        device = NightFixture()

        def call(tool, args=None):
            reply = device.call(tool, args)
            return text_reply("garbled") if tool == "wiiu_benchmark_start" else reply

        with self.assertRaises(json.JSONDecodeError):
            qualify(call, "Reopen Fixture", "default", 1, close=True)
        self.assertEqual(device.calls[-1], ("wiiu_benchmark_cancel", None))


if __name__ == "__main__":
    unittest.main()
