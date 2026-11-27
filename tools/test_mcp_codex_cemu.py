import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import mcp_codex_cemu_test as fixture


class ReusedAuthorizationTests(unittest.TestCase):
    def test_multiple_calls_reuse_one_ephemeral_codex_thread(self):
        class Transport:
            replies = {
                "initialize": {},
                "thread/start": {"thread": {"id": "selected-thread"}},
                "mcpServerStatus/list": {"data": [{"name": "wiiu", "tools": {"versions": {}, "worlds_list": {}}}]},
                "mcpServer/tool/call": {"content": [{"type": "text", "text": "[]"}]},
            }

            def __init__(self, *_):
                self.requests = []
                self.closed = False

            def send(self, line):
                self.requests.append(json.loads(line))

            def until(self, match, _):
                request = self.requests[-1]
                return match(json.dumps({"id": request["id"], "result": self.replies[request["method"]]}))

            def close(self):
                self.closed = True

        transport = Transport()
        with patch.object(fixture, "Process", return_value=transport) as process:
            with fixture.CodexSession("codex", {}, Path("."), "wiiu") as session:
                session.call("versions")
                session.call("worlds_list", {"instance": "default"})
        process.assert_called_once()
        self.assertTrue(transport.closed)
        starts = [item for item in transport.requests if item["method"] == "thread/start"]
        self.assertEqual(len(starts), 1)
        self.assertTrue(starts[0]["params"]["ephemeral"])
        self.assertEqual(sum(item["method"] == "mcpServerStatus/list" for item in transport.requests), 1)
        calls = [item["params"] for item in transport.requests if item["method"] == "mcpServer/tool/call"]
        self.assertEqual([(item["server"], item["threadId"], item["tool"]) for item in calls],
                         [("wiiu", "selected-thread", "versions"), ("wiiu", "selected-thread", "worlds_list")])

    def test_new_game_tools_refresh_the_catalog_and_known_calls_keep_server_errors(self):
        session = fixture.CodexSession("codex", {}, Path("."), "wiiu")
        session.thread = "selected-thread"
        statuses = iter((
            {"data": [{"name": "wiiu", "tools": {"versions": {}}}]},
            {"data": [{"name": "wiiu", "tools": {"versions": {}, "game_status": {}}}]},
        ))

        def rpc(method, params):
            if method == "mcpServerStatus/list":
                return next(statuses)
            if params["tool"] == "game_status":
                return {"isError": True, "content": [{"type": "text", "text": "Grant revoked."}]}
            return {"content": [{"type": "text", "text": "[]"}]}

        with patch.object(session, "rpc", side_effect=rpc) as transport:
            session.call("versions")
            for _ in range(2):
                with self.assertRaisesRegex(RuntimeError, "Grant revoked"):
                    session.call("game_status")
        self.assertEqual([call.args[0] for call in transport.call_args_list], [
            "mcpServerStatus/list", "mcpServer/tool/call",
            "mcpServerStatus/list", "mcpServer/tool/call", "mcpServer/tool/call",
        ])

    def test_resumed_game_uses_the_same_authorization_without_touching_night_settings(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            config = state / "config.toml"
            text = '[mcp_servers.horizon_cemu]\nurl = "https://fixture.local:8443/mcp"\n'
            config.write_text(text, encoding="utf-8")
            versions = {"content": [{"text": '[{"minecraft":"26.3","installed":true}]'}]}
            with (patch.object(fixture.shutil, "which", return_value="codex"),
                  patch.object(fixture, "call", return_value=versions) as call,
                  patch.object(fixture, "press") as press,
                  patch.object(fixture, "play_and_save") as play,
                  patch("builtins.print"),
                  patch.dict(fixture.os.environ, {"MCHORIZON_CEMU_ROUND_TRIPS": "2"})):
                fixture.exercise("https://fixture.local:8443", state / "ca.pem", 0, state,
                                 resume=True, game=True)
            self.assertEqual(call.call_count, 2)
            self.assertEqual([request.args[3] for request in play.call_args_list], [1, 2])
            self.assertEqual(config.read_text(encoding="utf-8"), text)
            press.assert_not_called()

    def test_prior_saves_do_not_satisfy_current_return(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "log.txt"
            previous = b"hosted save barrier completed\nframe.home.play\n"
            log.write_bytes(previous + b"frame.home.play\nhosted save barrier completed\n")
            with (patch.object(fixture.time, "monotonic", side_effect=[0, 0, 2]),
                  patch.object(fixture.time, "sleep")):
                with self.assertRaisesRegex(RuntimeError, "complete its save and return"):
                    fixture.wait_for_save_return(log, len(previous), 1)
            with log.open("ab") as output:
                output.write(b"frame.home.play\n")
            fixture.wait_for_save_return(log, len(previous), 1)


if __name__ == "__main__":
    unittest.main()
