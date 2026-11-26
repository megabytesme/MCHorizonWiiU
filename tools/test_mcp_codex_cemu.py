from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import mcp_codex_cemu_test as fixture


class ReusedAuthorizationTests(unittest.TestCase):
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
