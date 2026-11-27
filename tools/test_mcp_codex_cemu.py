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

    def test_cached_game_tools_survive_two_load_save_handoffs_in_one_process(self):
        launch_tools = ("versions", "worlds_list", "play")
        game_tools = ("game_state", "game_quit")

        class Transport:
            def __init__(self, *_):
                self.calls = []
                self.phase = "night"
                self.handoffs = 0
                self.listings = 0
                self.unavailable = 0
                self.closed = False

            def send(self, line):
                self.calls.append(json.loads(line))

            def until(self, response, _):
                request = self.calls[-1]
                return response(json.dumps({"id": request["id"], "result": self.reply(request)}))

            def close(self):
                self.closed = True

            def reply(self, request):
                method = request["method"]
                if method == "initialize":
                    return {}
                if method == "thread/start":
                    return {"thread": {"id": "one-thread"}}
                if method == "mcpServerStatus/list":
                    self.listings += 1
                    names = launch_tools if self.listings <= 2 else launch_tools + game_tools
                    return {"data": [{"name": "wiiu", "tools": dict.fromkeys(names)}]}
                assert method == "mcpServer/tool/call"
                params = request["params"]
                assert params["threadId"] == "one-thread"
                assert params["server"] == "wiiu"
                tool = params["tool"]
                is_error = False
                if tool == "versions":
                    text = "[]"
                elif tool == "worlds_list":
                    text = '[{"id":"Fixture","available":true}]'
                elif tool == "play":
                    assert params["arguments"] == {"world": "Fixture"}
                    self.phase = "loading"
                    self.handoffs += 1
                    text = "Horizon is starting."
                elif tool == "game_quit":
                    assert self.phase == "game"
                    self.phase = "night"
                    text = "Saving the world."
                else:
                    assert tool == "game_state"
                    if self.phase == "game":
                        text = '{"player":{"name":"Fixture"}}'
                    else:
                        if self.phase == "loading":
                            self.phase = "game"
                        self.unavailable += 1
                        is_error = True
                        text = ("No Horizon game is running under Agent Control. "
                                "Start one with play, or enable game control locally.")
                return {"isError": is_error, "content": [{"type": "text", "text": text}]}

        transport = Transport()
        ticks = iter(range(100))
        with (patch.object(fixture, "Process", return_value=transport) as process,
              patch.object(fixture.time, "monotonic", side_effect=lambda: next(ticks)),
              patch.object(fixture.time, "sleep"),
              patch.dict(fixture.os.environ, {"MCHORIZON_CEMU_REQUIRE_CAFE_PERFORMANCE": "0"}),
              patch("builtins.print")):
            with fixture.CodexSession("codex", {}, Path("."), "wiiu") as session:
                for index in (1, 2):
                    fixture.play_and_save(session.call, None, "Codex", index, None)
        process.assert_called_once()
        self.assertTrue(transport.closed)
        self.assertEqual((transport.handoffs, transport.unavailable), (2, 4))
        self.assertEqual(transport.listings, 3)
        methods = [entry["method"] for entry in transport.calls]
        self.assertEqual(methods.count("thread/start"), 1)
        self.assertEqual([entry["params"]["tool"] for entry in transport.calls
                          if entry["method"] == "mcpServer/tool/call"], [
            "worlds_list", "play", "versions", "game_state", "game_state",
            "game_quit", "versions", "game_state",
            "worlds_list", "play", "game_state", "versions", "game_state",
            "game_quit", "versions", "game_state",
        ])

    def test_absent_server_invalidates_the_preceding_listing_without_a_stale_call(self):
        session = fixture.CodexSession("codex", {}, Path("."), "wiiu")
        session.thread = "selected-thread"
        session.tools.add("versions")
        restored = {"data": [{"name": "wiiu", "tools": {"versions": {}}}]}
        output = {"content": [{"text": "[]"}]}
        with patch.object(session, "rpc", side_effect=[{"data": []}, restored, output]) as rpc:
            with self.assertRaisesRegex(RuntimeError, "did not list the configured MCP server"):
                session.call("game_state")
            self.assertEqual(session.call("versions"), output)
        self.assertEqual([call.args[0] for call in rpc.call_args_list],
                         ["mcpServerStatus/list", "mcpServerStatus/list", "mcpServer/tool/call"])

    def test_transport_failure_refreshes_only_the_failed_cached_tool(self):
        session = fixture.CodexSession("codex", {}, Path("."), "wiiu")
        session.thread = "selected-thread"
        session.tools.update({"versions", "game_state"})
        status = {"data": [{"name": "wiiu", "tools": {"versions": {}, "game_state": {}}}]}
        output = {"content": [{"text": "[]"}]}
        with patch.object(session, "rpc", side_effect=[
            OSError("connection closed"), output, status, output
        ]) as rpc:
            with self.assertRaisesRegex(OSError, "connection closed"):
                session.call("game_state")
            self.assertEqual(session.call("versions"), output)
            self.assertEqual(session.call("game_state"), output)
        self.assertEqual([call.args[0] for call in rpc.call_args_list], [
            "mcpServer/tool/call", "mcpServer/tool/call", "mcpServerStatus/list", "mcpServer/tool/call",
        ])

    def test_reentering_session_starts_discovery_and_ephemeral_thread_for_new_process(self):
        session = fixture.CodexSession("codex", {}, Path("."), "wiiu")
        state = {"data": [{"name": "wiiu", "tools": {"versions": {}}}]}
        output = {"content": [{"text": "[]"}]}
        with (patch.object(fixture, "Process") as process,
              patch.object(session, "rpc", side_effect=[
                  {}, state, {"thread": {"id": "first"}}, output,
                  {}, state, {"thread": {"id": "second"}}, output,
              ]) as rpc):
            for _ in range(2):
                with session:
                    self.assertEqual(session.call("versions"), output)
        self.assertEqual(process.call_count, 2)
        self.assertEqual([call.args[1]["threadId"] for call in rpc.call_args_list
                          if call.args[0] == "mcpServer/tool/call"], ["first", "second"])
        self.assertEqual(sum(call.args[0] == "mcpServerStatus/list" for call in rpc.call_args_list), 2)

    def test_resumed_game_uses_the_same_authorization_without_touching_night_settings(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            config = state / "config.toml"
            text = '[mcp_servers.horizon_cemu]\nurl = "https://fixture.local:8443/mcp"\n'
            config.write_text(text, encoding="utf-8")
            versions = {"content": [{"text": '[{"minecraft":"26.3","installed":true}]'}]}
            with (patch.object(fixture.shutil, "which", return_value="codex"),
                  patch.object(fixture, "call", return_value=versions) as call,
                  patch.object(fixture, "CodexSession") as session,
                  patch.object(fixture, "press") as press,
                  patch.object(fixture, "play_and_save") as play,
                  patch("builtins.print"),
                  patch.dict(fixture.os.environ, {"MCHORIZON_CEMU_ROUND_TRIPS": "2"})):
                fixture.exercise("https://fixture.local:8443", state / "ca.pem", 0, state,
                                 resume=True, game=True)
            self.assertEqual(call.call_count, 2)
            session.assert_called_once()
            active = session.return_value.__enter__.return_value
            self.assertEqual([request.args[0] for request in play.call_args_list], [active.call] * 2)
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
