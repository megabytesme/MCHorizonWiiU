import argparse
import contextlib
import json
import os
from pathlib import Path
import queue
import re
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None

class Process:
    def __init__(self, args, env=None):
        self.proc = subprocess.Popen(
            args, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace",
        )
        self.lines = queue.Queue()
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.proc.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def until(self, condition, seconds=45):
        deadline = time.monotonic() + seconds
        observed = []
        while time.monotonic() < deadline:
            try:
                line = self.lines.get(timeout=max(0, deadline - time.monotonic()))
            except queue.Empty:
                break
            if line is None:
                break
            observed.append(line)
            match = condition(line)
            if match:
                return match
        safe = "".join(observed)
        safe = re.sub(r"https?://\S+", "[url]", safe)
        raise RuntimeError(f"CLI did not reach the expected step. {safe[-2000:]}")

    def send(self, text):
        self.proc.stdin.write(text + "\n")
        self.proc.stdin.flush()

    def close(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(timeout=20)
        self.proc.stdout.close()
        with contextlib.suppress(OSError):
            self.proc.stdin.close()

def exercise(fixture_binary):
    with tempfile.TemporaryDirectory(prefix="horizon-mcp-cli-") as tmp:
        tmp = Path(tmp)
        profile = tmp / "console"
        profile.mkdir()
        codex_home = tmp / "codex"
        codex_home.mkdir()
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        profile_wsl = subprocess.check_output(["wsl.exe", "-d", "Ubuntu", "--exec", "wslpath", "-a", str(profile)], text=True).strip()
        origin = f"https://127.0.0.1:{port}"
        fixture = Process(["wsl.exe", "-d", "Ubuntu", "--exec", fixture_binary, profile_wsl, str(port)])
        try:
            fixture.until(lambda line: line.startswith("READY"))
            fixture.send("pair")
            fixture.until(lambda line: line.strip() == "OK")
            pem = profile / "launcher/agent-control-ca.pem"
            env = os.environ | {"CODEX_HOME": str(codex_home), "CODEX_CA_CERTIFICATE": str(pem)}
            (codex_home / "config.toml").write_text(
                f'mcp_oauth_credentials_store = "file"\n[mcp_servers.horizon_fixture]\nurl = "{origin}/mcp"\n',
                encoding="utf-8",
            )
            codex = shutil.which("codex")
            if not codex:
                raise RuntimeError("Codex is unavailable")
            oauth = Process([codex, "mcp", "login", "horizon_fixture", "--no-browser",
                             "--scopes", "launcher.read", "--oauth-client-registration", "dcr"], env=env)
            context = ssl.create_default_context(cafile=str(pem))
            browser = urllib.request.build_opener(urllib.request.HTTPSHandler(context=context), NoRedirect())
            def get(url, data=None, headers=None):
                req = urllib.request.Request(url, data, headers or {})
                try:
                    return browser.open(req, timeout=10)
                except urllib.error.HTTPError as error:
                    return error
            try:
                match = oauth.until(lambda line: re.search(r"https://127\.0\.0\.1:\d+/oauth/authorize\?\S+", line))
                auth_url = match.group()
                callback = urllib.parse.parse_qs(urllib.parse.urlsplit(auth_url).query)["redirect_uri"][0]
                form = get(auth_url).read().decode()
                nonce = re.search(r'name="csrf" value="([^"]+)"', form)[1]
                posted = get(origin + "/oauth/authorize", urllib.parse.urlencode({"csrf": nonce}).encode(), {"Origin": origin})
                assert posted.code == 303, posted.code
                wait_url = origin + posted.headers["Location"]
                fixture.send("approve")
                fixture.until(lambda line: line.strip() == "OK")
                approved = get(wait_url)
                assert approved.code == 303, approved.code
                final_url = approved.headers["Location"]
                assert final_url.startswith(callback + "?")
                oauth.send(final_url)
                oauth.until(lambda line: "Successfully logged in" in line or "successfully authenticated" in line.lower())
                assert oauth.proc.wait(timeout=20) == 0
                print("Codex CLI dynamic registration, browser approval and PKCE token exchange passed.")
            finally:
                oauth.close()
            def rpc(process, ident, method, params):
                process.send(json.dumps({"jsonrpc":"2.0", "id":ident, "method":method, "params":params}))
                def reply(line):
                    if not line.startswith("{"):
                        return None
                    value = json.loads(line)
                    return value if value.get("id") == ident else None
                return process.until(reply, 60)
            def codex_list():
                application = Process([codex, "app-server", "--listen", "stdio://"], env)
                try:
                    result = rpc(application, 1, "initialize",
                        {"clientInfo":{"name":"horizon_fixture", "version":"1"}, "capabilities":{"experimentalApi":True}})
                    assert "result" in result, result
                    application.send('{"jsonrpc":"2.0","method":"initialized"}')
                    result = rpc(application, 2, "mcpServerStatus/list", {})
                    status = next(x for x in result["result"]["data"] if x["name"] == "horizon_fixture")
                    assert "versions" in status["tools"], status.get("authStatus")
                finally:
                    application.close()
            codex_list()
            print("A separate Codex process listed tools over authenticated HTTPS.")
            fixture.send("quit")
            fixture.proc.wait(timeout=20)
            fixture.close()
            fixture = Process(["wsl.exe", "-d", "Ubuntu", "--exec", fixture_binary, profile_wsl, str(port)])
            fixture.until(lambda line: line.startswith("READY"))
            codex_list()
            print("Codex reconnected and listed tools after an independent no-PIN host restart.")
            fixture.send("expire-access")
            fixture.until(lambda line: line.strip() == "OK")
            codex_list()
            print("Codex refreshed authorization after its saved access token was invalidated.")
        finally:
            try:
                fixture.send("quit")
                fixture.proc.wait(timeout=10)
            except (ValueError, OSError, subprocess.TimeoutExpired):
                pass
            fixture.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("wsl_fixture_binary")
    exercise(parser.parse_args().wsl_fixture_binary)
