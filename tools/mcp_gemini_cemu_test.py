"""Pair the installed Gemini CLI provider and exercise genuine Cafe MCP."""
import argparse
import json
import os
from pathlib import Path
import re
import ssl
import urllib.error
import urllib.parse
import urllib.request

from mcp_codex_cemu_test import press, run_game
from mcp_codex_host_test import NoRedirect, Process


def exercise(bundle, origin, cafile, window, state, resume, game, refresh, game_grant):
    state.mkdir(parents=True, exist_ok=True)
    env = os.environ | {"GEMINI_CLI_HOME": str(state), "NODE_EXTRA_CA_CERTS": str(cafile)}
    cli = ["node", str(Path(__file__).with_name("mcp_gemini_host_test.mjs")), bundle, origin + "/mcp"]
    browser = urllib.request.build_opener(
        NoRedirect(), urllib.request.HTTPSHandler(context=ssl.create_default_context(cafile=str(cafile)))
    )

    def open_url(url, data=None, headers=None):
        request = urllib.request.Request(url, data, headers or {})
        try:
            return browser.open(request, timeout=15)
        except urllib.error.HTTPError as error:
            return error

    def execute(mode, tool=None, arguments=None, game_scopes=game or game_grant):
        cli_mode = f"{mode}-game" if game_scopes else mode
        args = cli + [cli_mode, "cafe"]
        if tool:
            args += [tool, json.dumps(arguments or {})]
        program = Process(args, env)
        try:
            if mode.startswith("pair"):
                program.send("y")
                url = program.until(lambda line: line.removeprefix("AUTHORIZE ").strip() if line.startswith("AUTHORIZE ") else None)
                login = open_url(url).read().decode()
                nonce = re.search(r'name="csrf" value="([^"]+)"', login)[1]
                pending = open_url(origin + "/oauth/authorize",
                    urllib.parse.urlencode({"csrf": nonce}).encode(), {"Origin": origin})
                assert pending.code == 303
                for key in (83, 90, 90, 90):
                    press(window, key)
                approved = open_url(origin + pending.headers["Location"])
                assert approved.code == 303, "The Cemu GamePad approval was not accepted"
                callback = open_url(approved.headers["Location"])
                assert callback.code == 200
            result = program.until(lambda line: json.loads(line.removeprefix("RESULT ")) if line.startswith("RESULT ") else None)
            assert program.proc.wait(timeout=15) == 0
            return result
        finally:
            program.close()

    def verify_stateless(access_token):
        version = "2026-07-28"

        def rpc(method, parameters, routing_name=None):
            params = {**parameters, "_meta": {
                "io.modelcontextprotocol/protocolVersion": version,
                "io.modelcontextprotocol/clientCapabilities": {},
            }}
            headers = {
                "Content-Type": "application/json", "Accept": "application/json",
                "Mcp-Protocol-Version": version, "Mcp-Method": method,
                "Authorization": "Bearer " + access_token,
            }
            if routing_name is not None:
                headers["Mcp-Name"] = routing_name
            data = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
            with open_url(origin + "/mcp", data, headers) as response:
                return response.code, json.load(response)

        status, discovery = rpc("server/discover", {})
        assert status == 200 and version in discovery["result"]["supportedVersions"]
        status, listing = rpc("tools/list", {})
        assert status == 200 and listing["result"]["cacheScope"] == "private"
        assert any(tool["name"] == "versions" for tool in listing["result"]["tools"])
        parameters = {"name": "versions", "arguments": {"installed": True}}
        status, reply = rpc("tools/call", parameters, "versions")
        assert status == 200 and json.loads(reply["result"]["content"][0]["text"])[0]["minecraft"] == "26.3"
        status, error = rpc("tools/call", parameters, "incorrect")
        assert status == 400 and error["error"]["code"] == -32020
        print("Cemu passed stateless discovery, tool listing, tool call and routing-header rejection.")

    if not resume:
        press(window, 83)
        press(window, 90)
        execute("pair")
        print("Gemini paired through its desktop callback and called the Cemu-hosted Night versions tool.")
        press(window, 88)
    credentials = state / ".gemini/mcp-oauth-tokens.json"
    if refresh and game_grant:
        execute("refresh", "versions", {"installed": True}, game_scopes=False)
        reduced, = json.loads(credentials.read_bytes())
        assert reduced["token"]["scope"] == "launcher.read"
    reply = execute("refresh" if refresh else "reuse", "versions", {"installed": True})
    assert json.loads(reply["content"][0]["text"])[0]["minecraft"] == "26.3"
    print("A new Gemini process reused the grant and called Night at the same HTTPS URL.")
    if not resume:
        saved, = json.loads(credentials.read_bytes())
        verify_stateless(saved["token"]["accessToken"])
    if refresh:
        print("Gemini refreshed the stored grant on the Cemu guest and called versions with the rotated authorization.")
        if game_grant:
            restored, = json.loads(credentials.read_bytes())
            assert set(restored["token"]["scope"].split()) == {
                "launcher.read", "launcher.manage", "game.observe", "game.control",
            }
            print("Gemini regained every consented scope after a restricted access-token refresh.")
    if game and not resume:
        run_game(lambda tool, args=None: execute("reuse", tool, args), window, "Gemini")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle")
    parser.add_argument("origin")
    parser.add_argument("ca", type=Path)
    parser.add_argument("window", type=int)
    parser.add_argument("state", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--game", action="store_true")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--game-grant", action="store_true")
    args = parser.parse_args()
    exercise(args.bundle, args.origin, args.ca, args.window, args.state,
             args.resume, args.game, args.refresh, args.game_grant)
