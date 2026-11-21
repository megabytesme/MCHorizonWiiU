"""Exercise the real Windows Codex CLI against the endpoint inside Cemu."""
import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import re
import shutil
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request

from mcp_codex_host_test import NoRedirect, Process


def press(window, key):
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    windows = [window]
    def append(handle, _):
        windows.append(handle)
        return True
    user32.EnumChildWindows.argtypes = (wintypes.HWND, callback_type, wintypes.LPARAM)
    user32.EnumChildWindows(window, callback_type(append), 0)
    user32.PostMessageW.argtypes = (wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
    scan = user32.MapVirtualKeyW(key, 0) << 16
    for message, flags in [(0x100, scan | 1), (0x101, scan | 0xC0000001)]:
        for handle in windows:
            user32.PostMessageW(handle, message, key, flags)
        time.sleep(0.20)


def call(codex, env, workspace, tool, arguments=None):
    """Use the installed Codex app server without making a model request."""
    client = Process([codex, "app-server", "--listen", "stdio://"], env)
    def rpc(ident, method, params):
        client.send(json.dumps({"jsonrpc": "2.0", "id": ident, "method": method, "params": params}))
        def response(line):
            if line.startswith("{"):
                data = json.loads(line)
                return data if data.get("id") == ident else None
            return None
        reply = client.until(response, 90)
        if "error" in reply:
            raise RuntimeError(f"Codex {method} failed: {reply['error'].get('message')}")
        return reply["result"]
    try:
        rpc(1, "initialize", {
            "clientInfo": {"name": "horizon-cemu-test", "version": "1.0"},
            "capabilities": {"experimentalApi": True},
        })
        client.send('{"jsonrpc":"2.0","method":"initialized"}')
        status = rpc(2, "mcpServerStatus/list", {})
        server = next(server for server in status["data"] if server["name"] == "horizon_cemu")
        if tool not in server["tools"]:
            raise RuntimeError(f"Codex did not list {tool} from the emulated Wii U. Auth={server.get('authStatus')}")
        thread = rpc(3, "thread/start", {"cwd": str(workspace), "ephemeral": True})["thread"]["id"]
        reply = rpc(4, "mcpServer/tool/call", {
            "threadId": thread, "server": "horizon_cemu", "tool": tool, "arguments": arguments or {},
        })
        if reply.get("isError") or not reply.get("content"):
            raise RuntimeError(f"Tool call {tool} returned an error or empty result")
        return reply
    finally:
        client.close()


def exercise(origin, ca, window, state, resume=False, game=False):
    codex = shutil.which("codex")
    assert codex
    state.mkdir(parents=True, exist_ok=True)
    env = os.environ | {"CODEX_HOME": str(state), "CODEX_CA_CERTIFICATE": str(ca)}
    if not resume:
        (state / "config.toml").write_text(
            'mcp_oauth_credentials_store = "file"\n[mcp_servers.horizon_cemu]\n'
            f'url = "{origin}/mcp"\n', encoding="utf-8")
        # The initial Agent Control page is focused on HTTPS connection.
        press(window, 83)
        press(window, 90)
        login = Process([codex, "mcp", "login", "horizon_cemu", "--no-browser",
                         "--scopes", "launcher.read,launcher.manage,game.observe,game.control" if game else "launcher.read",
                         "--oauth-client-registration", "dcr"], env)
        context = ssl.create_default_context(cafile=str(ca))
        browser = urllib.request.build_opener(urllib.request.HTTPSHandler(context=context), NoRedirect())
        def open_url(url, data=None, headers=None):
            req = urllib.request.Request(url, data, headers or {})
            try:
                return browser.open(req, timeout=10)
            except urllib.error.HTTPError as error:
                return error
        try:

            match = login.until(lambda line: re.search(re.escape(origin) + r"/oauth/authorize\?\S+", line))
            form = open_url(match.group()).read().decode()
            nonce = re.search(r'name="csrf" value="([^"]+)"', form)[1]
            pending = open_url(origin + "/oauth/authorize",
                urllib.parse.urlencode({"csrf": nonce}).encode(), {"Origin": origin})
            assert pending.code == 303
            status_url = origin + pending.headers["Location"]
            # Open Authorization requests, select the request, then approve it.
            for key in (83, 90, 90, 90):
                press(window, key)
            approved = open_url(status_url)
            if approved.code != 303:
                raise RuntimeError("The matching OAuth request was not approved by the Cemu GamePad")
            login.send(approved.headers["Location"])
            login.until(lambda line: "Successfully logged in" in line or "successfully authenticated" in line.lower())
            assert login.proc.wait(timeout=20) == 0
            print("Codex completed DCR and GamePad-approved OAuth on the emulated Wii U.")
            # Return to the main Agent Control page.
            press(window, 88)
        finally:
            login.close()
    for _ in range(2):
        result = call(codex, env, state, "versions", {"installed": True})
        versions = json.loads(result["content"][0]["text"])
        assert versions[0]["minecraft"] == "26.3" and versions[0]["installed"] is True
    print("Two independent Codex processes listed and called the real Night versions tool.")
    if game and not resume:
        run_game(lambda tool, args=None: call(codex, env, state, tool, args), window, "Codex")


def run_game(invoke, window, client_name):
    # Enter Night's Agent Control settings and grant both local permissions.
    for key in (83, 83, 83, 83, 90, 83, 90, 83, 90, 88, 88):
        press(window, key)
    rounds = int(os.environ.get("MCHORIZON_CEMU_ROUND_TRIPS", "1"))
    if rounds < 1:
        raise ValueError("at least one hosted game round trip is required")
    evidence = os.environ.get("MCHORIZON_CEMU_GAME_EVIDENCE")
    for index in range(1, rounds + 1):
        play_and_save(invoke, window, client_name, index, Path(evidence) if evidence else None)


def play_and_save(invoke, window, client_name, index, evidence):
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        worlds = invoke("worlds_list")
        saved = next((world["id"] for world in json.loads(worlds["content"][0]["text"])
                      if world["available"]), None)
        if evidence and saved is None:
            time.sleep(3)
            continue
        try:
            reply = invoke("play", {"world": saved} if saved else {})
        except RuntimeError as error:
            if any(expected in str(error) for expected in (
                "Return to the Night home page", "The selected installation or world is unavailable",
            )):
                time.sleep(3)
                continue
            raise
        assert "Horizon is starting" in reply["content"][0]["text"]
        break
    else:
        raise RuntimeError("Night never made the disposable save available for launch.")
    print(f"{client_name} requested hosted launch {index} through the Night HTTPS endpoint.", flush=True)

    deadline = time.monotonic() + int(os.environ.get("MCHORIZON_CEMU_READY_TIMEOUT", "1200"))
    launcher_probe = 0
    while time.monotonic() < deadline:
        try:
            reply = invoke("game_state")
        except RuntimeError as error:
            if not any(expected in str(error) for expected in (
                "Tool missing from the current catalog",
                "did not list game_state",
                "game did not accept the request in time",
            )):
                raise
            now = time.monotonic()
            if now >= launcher_probe:
                invoke("versions", {"installed": True})
                launcher_probe = now + 30
                print(f"{client_name} reached Night while waiting for the game.", flush=True)
            time.sleep(5)
            continue
        game = json.loads(reply["content"][0]["text"])
        if not saved or game.get("player") is not None:
            break
        time.sleep(2)
    else:
        raise RuntimeError("The original game never became available through MCP.")
    print(f"{client_name} used native game_state in hosted session {index}."
          + (" An isolated saved world had a live player." if saved else " The test used the native title."),
          flush=True)

    if evidence:
        # Leave time for terrain streaming before capturing both displays.
        # A live player can appear while the first visible sections are queued.
        time.sleep(int(os.environ.get("MCHORIZON_CEMU_CAPTURE_DELAY", "45")))
        shots = evidence / "portable/screenshots"
        previous = set(shots.glob("*.png"))
        press(window, 123)
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            captures = sorted(set(shots.glob("*.png")) - previous)
            try:
                complete = len(captures) == 2 and all(
                    p.read_bytes().endswith(b"IEND\xaeB\x60\x82") for p in captures
                )
            except OSError:
                complete = False
            if complete:
                for capture in captures:
                    display = "gamepad" if capture.stem.endswith("_GamePad") else "tv"
                    shutil.copyfile(capture, evidence / f"session-{index}-{display}.png")
                break
            time.sleep(0.5)
        else:
            print(f"Cemu did not record both display screenshots for session {index}.", flush=True)

    quit_reply = invoke("game_quit")
    # Launcher read tools remain callable while the original game saves.
    launcher_during_save = invoke("versions", {"installed": True})
    try:
        invoke("game_state")
    except RuntimeError as error:
        if not any(expected in str(error) for expected in (
            "Tool missing from the current catalog", "did not list game_state",
        )):
            raise
    else:
        raise RuntimeError("A fresh client could still call game_state after hosted quit.")
    print(f"{client_name} requested the original hosted quit for session {index} and called Night again.",
          flush=True)
    if evidence:
        log = evidence / "portable/log.txt"
        deadline = time.monotonic() + int(os.environ.get("MCHORIZON_CEMU_SAVE_RETURN_TIMEOUT", "120"))
        while time.monotonic() < deadline:
            trace = log.read_text(encoding="utf-8", errors="replace")
            if (trace.count("hosted save barrier completed") >= index
                    and trace.count("frame.home.play") >= index + 1):
                print(f"Session {index} completed its save barrier and returned to Night.", flush=True)
                break
            time.sleep(2)
        else:
            raise RuntimeError(f"Hosted game {index} never completed its save and return.")
        launcher_after_save = invoke("versions", {"installed": True})
        receipt = {
            "world": saved,
            "game_state": game,
            "game_quit": quit_reply,
            "launcher_during_save": launcher_during_save,
            "launcher_after_save": launcher_after_save,
            "game_tools_withdrawn": True,
        }
        with (evidence / f"session-{index}-mcp.json").open("x", encoding="utf-8") as output:
            json.dump(receipt, output, indent=2)
            output.write("\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("origin")
    parser.add_argument("ca", type=Path)
    parser.add_argument("window", type=int)
    parser.add_argument("state", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--game", action="store_true")
    args = parser.parse_args()
    exercise(args.origin, args.ca, args.window, args.state, args.resume, args.game)
