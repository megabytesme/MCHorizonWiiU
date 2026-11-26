import gzip
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from wiiu_mcp_test_world import FOLDER, Limits, main, qualify


INSTANCE = "default"
WORLD = "a" * 32
CREATE = "1" * 32
REOPEN = "2" * 32


def reply(value):
    return {"content": [{"type": "text", "text": json.dumps(value)}]}


class Clock:
    def __init__(self):
        self.seconds = 0

    def now(self):
        return self.seconds

    def wait(self, seconds):
        self.seconds += seconds


class NightWorldFixture:
    def __init__(self, root):
        self.root = root
        self.original = root / "saves" / "Ordinary world" / "level.dat"
        self.original.parent.mkdir(parents=True)
        self.original.write_bytes(b"original")
        self.calls = []
        self.launch = None
        self.playing = False
        self.quitting = False
        self.saved = False
        self.terrain_polls = self.ready_polls = 0
        self.hot_home = False
        self.never_pair = False
        self.mutate_original = False
        self.omit_files = False
        self.bad_reopen = False

    def persist(self):
        sandbox = self.root / ".night-test-worlds" / WORLD
        save = sandbox / "saves" / FOLDER
        paths = (save, save / "players" / "data", save / "dimensions" / "minecraft" / "overworld" / "region")
        for path in paths:
            path.mkdir(parents=True, exist_ok=True)
        (sandbox / "horizon-night-test-world.json").write_text(json.dumps({
            "schema": 1, "id": WORLD, "instance": INSTANCE, "owner": "offline:Guest",
        }), encoding="utf-8")
        (save / "level.dat").write_bytes(gzip.compress(b"\x0a\x00\x00\x00"))
        (paths[1] / "player.dat").write_bytes(b"persisted player")
        (paths[2] / "r.0.0.mca").write_bytes(b"\x00\x00\x02\x01" + bytes(12284))

    def state(self, timestamp, sections):
        return {
            "screen": "playing", "player": {"position": [1, 65, 2]}, "pending_chunks": 0,
            "server": {"loaded": True},
            "cafe_performance": {"phase": "gameplay", "sample_at_session_ms": timestamp,
                                 "paired_submissions": 1, "last_terrain_sections": sections},
        }

    def call(self, name, args=None):
        self.calls.append((name, args))
        if name == "wiiu_benchmark_status":
            if self.quitting and not self.hot_home:
                self.quitting = self.playing = False
                if not self.omit_files:
                    self.persist()
                if self.mutate_original:
                    self.original.write_bytes(b"modified")  # Same size as the before manifest.
                self.saved = True
            return reply({"phase": "idle", "night_home_ready": not (self.playing or self.quitting)})
        if name == "wiiu_test_worlds_list":
            return reply({"instance": INSTANCE, "worlds": [{"id": WORLD, "available": True}] if self.saved else []})
        if name in {"wiiu_test_world_create", "wiiu_test_world_open"}:
            self.launch = CREATE if name.endswith("create") else REOPEN
            self.playing = True
            self.terrain_polls = self.ready_polls = 0
            world = "b" * 32 if self.bad_reopen and self.launch == REOPEN else WORLD
            return {"structuredContent": {"launch_id": self.launch, "instance": INSTANCE, "world_id": world}}
        if name == "wiiu_loading_status":
            self.assert_launch(args["launch_id"])
            return reply({"launch_id": self.launch, "phase": "handoff_started"})
        if name == "game_wait_ready":
            self.ready_polls += 1
            if self.ready_polls == 1:
                raise RuntimeError("Codex did not list game_wait_ready from fixture. Auth=oAuth")
            return reply(self.state(1000, [2, 2]))
        if name == "game_status":
            self.terrain_polls += 1
            if self.terrain_polls == 1:
                return reply(self.state(1000, [11, 12]))  # Pre-readiness sample.
            if self.terrain_polls == 2 or self.never_pair:
                return reply(self.state(2000 + self.terrain_polls, [3, 0]))
            return reply(self.state(2000 + self.terrain_polls, [3, 4]))
        if name == "game_quit":
            self.quitting = True
            return reply({"ok": True})
        raise AssertionError(f"Unexpected call {name}")

    def assert_launch(self, launch):
        if launch != self.launch:
            raise AssertionError("Played an unrelated startup ID")


class DisposableWorldTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="night-world-host-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.device = NightWorldFixture(self.root)
        self.clock = Clock()
        self.evidence = {}

    def run_flow(self):
        return qualify(self.device.call, self.root, INSTANCE, 1234, Limits(6, 3, 3, 4, 2),
                       self.evidence, clock=self.clock.now, sleep=self.clock.wait)

    def test_ready_terrain_save_and_reopen_use_distinct_checkpoints(self):
        result = self.run_flow()
        self.assertEqual(result["stage"], "complete")
        self.assertEqual(result["world_id"], WORLD)
        self.assertEqual([result[op]["launch_id"] for op in ("create", "reopen")], [CREATE, REOPEN])
        self.assertEqual(result["ordinary_saves"]["files"], 1)
        for operation in ("create", "reopen"):
            step = result[operation]
            self.assertTrue(step["ordinary_saves_unchanged"])
            self.assertEqual(step["paired_terrain"]["sections_tv_gamepad"], [3, 4])
            self.assertEqual(step["ready_seconds"], 0.75)
            self.assertEqual(step["terrain_seconds"], 1.5)
            self.assertEqual(step["saved_files"]["player_files"], 1)
        operations = [(name, args) for name, args in self.device.calls
                      if name in {"wiiu_test_world_create", "game_save", "game_quit", "wiiu_test_world_open"}]
        self.assertEqual(operations, [
            ("wiiu_test_world_create", {"seed": 1234}), ("game_quit", None),
            ("wiiu_test_world_open", {"id": WORLD}), ("game_quit", None),
        ])

    def test_game_readiness_has_its_own_deadline(self):
        with self.assertRaisesRegex(TimeoutError, "Game readiness"):
            qualify(self.device.call, self.root, INSTANCE, 1234, Limits(0.5, 3, 3, 4, 2),
                    self.evidence, clock=self.clock.now, sleep=self.clock.wait)
        self.assertEqual(self.evidence["stage"], "create_game_ready")
        self.assertNotIn(("game_status", None), self.device.calls)

    def test_warm_reopen_uses_its_separate_deadline(self):
        with self.assertRaisesRegex(TimeoutError, "Game readiness"):
            qualify(self.device.call, self.root, INSTANCE, 1234, Limits(6, 3, 3, 0.5, 2),
                    self.evidence, clock=self.clock.now, sleep=self.clock.wait)
        self.assertEqual(self.evidence["stage"], "reopen_game_ready")
        self.assertTrue(self.evidence["create"]["ordinary_saves_unchanged"])
        self.assertEqual(self.device.calls.count(("game_quit", None)), 1)

    def test_unpaired_terrain_times_out_after_game_ready(self):
        self.device.never_pair = True
        with self.assertRaisesRegex(TimeoutError, "Paired terrain"):
            self.run_flow()
        self.assertEqual(self.evidence["stage"], "create_paired_terrain")
        self.assertIn("ready_seconds", self.evidence["create"])
        self.assertNotIn(("game_quit", None), self.device.calls)

    def test_quit_ack_does_not_prove_durable_save_or_open(self):
        self.device.hot_home = True
        with self.assertRaisesRegex(TimeoutError, "Save and return"):
            self.run_flow()
        self.assertEqual(self.evidence["stage"], "create_durable_save")
        self.assertNotIn("saved_files", self.evidence["create"])
        self.assertNotIn(("wiiu_test_world_open", {"id": WORLD}), self.device.calls)

    def test_night_available_without_files_does_not_qualify(self):
        self.device.omit_files = True
        with self.assertRaises(FileNotFoundError):
            self.run_flow()
        self.assertEqual(self.evidence["stage"], "create_durable_save")

    def test_exact_file_hash_detects_same_size_ordinary_save_change(self):
        self.device.mutate_original = True
        with self.assertRaisesRegex(ValueError, "ordinary saves tree changed"):
            self.run_flow()
        self.assertEqual(self.evidence["stage"], "create_ordinary_save_identity")
        self.assertNotIn(("wiiu_test_world_open", {"id": WORLD}), self.device.calls)

    def test_reopen_requires_same_world_id(self):
        self.device.bad_reopen = True
        with self.assertRaisesRegex(ValueError, "same disposable world"):
            self.run_flow()
        self.assertEqual(self.evidence["stage"], "reopen_launch")

    def test_cli_opens_one_session_and_passes_the_same_call(self):
        output = self.root / "receipt.json"
        args = ["wiiu_mcp_test_world.py", "--codex-home", str(self.root / "profile"),
                "--ca", str(self.root / "ca.pem"), "--server", "fixture", "--install-root",
                str(self.root), "--output", str(output)]
        with (
            patch("sys.argv", args),
            patch("wiiu_mcp_test_world.paired_session") as factory,
            patch("wiiu_mcp_test_world.qualify") as driver,
        ):
            session = factory.return_value.__enter__.return_value
            main()
            factory.assert_called_once()
            factory.return_value.__enter__.assert_called_once()
            factory.return_value.__exit__.assert_called_once()
            self.assertIs(driver.call_args.args[0], session.call)
        self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["status"], "passed")


if __name__ == "__main__":
    unittest.main()
