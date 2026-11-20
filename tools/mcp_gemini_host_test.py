"""Exercise the installed Gemini CLI's provider, callback, storage and MCP client."""
import argparse
import os
from pathlib import Path
import re
import socket
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

from mcp_codex_host_test import NoRedirect, Process

def main(bundle, fixture_binary):
    with tempfile.TemporaryDirectory(prefix="horizon-gemini-mcp-") as directory:
        runtime = Path(directory)
        profile = runtime / "console"
        profile.mkdir()
        gemini = runtime / "gemini"
        gemini.mkdir()
        with socket.socket() as reserve:
            reserve.bind(("127.0.0.1", 0))
            port = reserve.getsockname()[1]
        wsl = ["wsl.exe", "-d", "Ubuntu", "--exec"]
        profile_wsl = subprocess.check_output(wsl + ["wslpath", "-a", str(profile)], text=True).strip()
        launch = wsl + [fixture_binary, profile_wsl, str(port)]
        fixture = Process(launch)
        try:
            fixture.until(lambda line: line.startswith("READY"))
            fixture.send("pair")
            fixture.until(lambda line: line.strip() == "OK")
            pem = profile / "launcher/agent-control-ca.pem"
            origin = f"https://127.0.0.1:{port}"
            env = os.environ | {"GEMINI_CLI_HOME": str(gemini), "NODE_EXTRA_CA_CERTS": str(pem)}
            node = ["node", str(Path(__file__).with_suffix(".mjs")), bundle, origin + "/mcp"]
            browser = urllib.request.build_opener(urllib.request.HTTPSHandler(
                context=ssl.create_default_context(cafile=str(pem))), NoRedirect())
            def open_url(url, data=None, headers=None):
                req = urllib.request.Request(url, data, headers or {})
                try:
                    return browser.open(req, timeout=10)
                except urllib.error.HTTPError as error:
                    return error
            def wait_for_listener():
                # The Windows WSL loopback forward can become usable after READY.
                deadline = time.monotonic() + 15
                while True:
                    try:
                        with open_url(origin + "/.well-known/oauth-authorization-server") as endpoint:
                            assert endpoint.code == 200
                        return
                    except OSError:
                        if time.monotonic() > deadline:
                            raise
                        time.sleep(0.25)

            wait_for_listener()
            proc = Process(node + ["pair"], env)
            proc.send("y")
            try:
                url = proc.until(lambda line: line.removeprefix("AUTHORIZE ").strip() if line.startswith("AUTHORIZE ") else None)
                content = open_url(url).read().decode()
                csrf = re.search(r'name="csrf" value="([^"]+)"', content)[1]
                response = open_url(origin + "/oauth/authorize",
                    urllib.parse.urlencode({"csrf": csrf}).encode(), {"Origin": origin})
                assert response.code == 303
                wait = origin + response.headers["Location"]
                fixture.send("approve")
                fixture.until(lambda line: line.strip() == "OK")
                callback = open_url(wait).headers["Location"]
                assert callback.startswith("http://localhost:") or callback.startswith("http://127.0.0.1:")
                assert open_url(callback).code == 200
                proc.until(lambda line: line.startswith("PASS pair"))
                assert proc.proc.wait(timeout=15) == 0
                print("Gemini CLI provider paired through its real browser callback and listed and called a tool.")
            finally:
                proc.close()
            fixture.send("quit")
            assert fixture.proc.wait(timeout=10) == 0
            fixture.close()
            fixture = Process(launch)
            fixture.until(lambda line: line.startswith("READY"))
            wait_for_listener()
            for mode in ["reuse", "refresh"]:
                proc = Process(node + [mode], env)
                try:
                    proc.until(lambda line: line.startswith("PASS " + mode))
                    assert proc.proc.wait(timeout=15) == 0
                finally:
                    proc.close()
            print("Gemini CLI reused authorization after the host restarted and refreshed the grant after simulated client expiry.")
        finally:
            try:
                fixture.send("quit")
                fixture.proc.wait(timeout=10)
            except (ValueError, OSError, subprocess.TimeoutExpired):
                pass
            fixture.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("gemini_cli_bundle_chunk", help="Installed bundle chunk exporting MCPOAuthProvider and Client")
    parser.add_argument("wsl_fixture_binary")
    args = parser.parse_args()
    main(args.gemini_cli_bundle_chunk, args.wsl_fixture_binary)
