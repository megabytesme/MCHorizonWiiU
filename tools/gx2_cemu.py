#!/usr/bin/env python3
"""Prepare and drive an isolated Windows Cemu fixture for the hosted GX2 port."""

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ET


CPU_MODES = {"interpreter": 0, "single": 1, "multi": 3}


def sha256(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def standalone_title_id(rpx):
    """Match Cemu 2.6's placeholder title ID for a standalone RPX."""
    checksum = 0x3416DCBF
    with rpx.open("rb") as handle:
        while block := handle.read(1024 * 1024):
            for byte in block:
                checksum = (((checksum << 3) | (checksum >> 29)) + byte) & 0xFFFFFFFF
    return f"ffffffff{checksum:08x}"


def paths(lab):
    portable = lab / "portable"
    return portable, portable / "sdcard/wiiu/apps/HorizonWiiU"


def prepare(args):
    portable, app = paths(args.lab)
    instance = "default" if args.default_instance else "Lab"
    game = app if args.default_instance else app / "instances/Lab"
    if args.keep_fixture_distances and (not args.game or not (args.game / "options.txt").is_file()):
        raise ValueError("preserving fixture distances requires --game with options.txt")
    args.lab.mkdir(parents=True)
    game.mkdir(parents=True)
    (app / "launcher").mkdir()
    shutil.copyfile(args.cemu, args.lab / "Cemu.exe")
    shutil.copyfile(args.rpx, args.lab / "HorizonWiiU.rpx")
    settings_file = portable / "settings.xml"
    settings_xml = ET.parse(args.template / "settings.xml")
    for element in settings_xml.iter():
        if element.tag.lower() in ("mlc_path", "gamepaths"):
            element.clear()
    settings_xml.write(settings_file, encoding="utf-8", xml_declaration=True)
    shutil.copytree(args.template / "controllerProfiles", portable / "controllerProfiles")
    inputs = [args.lab / "Cemu.exe", args.lab / "HorizonWiiU.rpx", portable / "settings.xml"]
    if args.game:
        subpath = ".local/horizon-client/26.3"
        resources = args.game / subpath
        if not (resources / "horizon-resources.json").is_file():
            raise ValueError("fixture game directory has no prepared 26.3 manifest")
        result = subprocess.run(
            ["robocopy", str(resources), str(game / subpath), "/E", "/MT:32",
             "/R:1", "/W:1", "/NJS", "/NJH", "/NP", "/NFL", "/NDL"],
            check=False, timeout=240,
        )
        if result.returncode >= 8:
            raise ValueError(f"resource copy failed ({result.returncode})")
        inputs.append(game / subpath / "horizon-resources.json")
        if (game / subpath / "horizon-data.bin").is_file():
            inputs.append(game / subpath / "horizon-data.bin")
        for name in ("options.txt", "horizon-controller.txt", "horizon-graphics.txt"):
            if (args.game / name).is_file():
                shutil.copyfile(args.game / name, game / name)
        if not args.keep_fixture_distances:
            opts = game / "options.txt"
            lines = opts.read_text(encoding="utf-8").splitlines() if opts.exists() else []
            lines = [line for line in lines if not line.startswith(("renderDistance:", "simulationDistance:"))]
            opts.write_text("\n".join(lines + ["renderDistance:12", "simulationDistance:12", ""]), encoding="utf-8")
        inputs.extend(sorted(game.glob("*.txt")))
    if args.scratch_world:
        if not (args.scratch_world / "level.dat").is_file():
            raise ValueError("provide a disposable world fixture with level.dat")
        copy = game / "saves" / args.scratch_world.name
        shutil.copytree(args.scratch_world, copy, ignore=shutil.ignore_patterns("session.lock"))
        inputs.extend(sorted(path for path in copy.rglob("*") if path.is_file()))
    settings = {"schema": 2, "generation": 1, "preferences": {
        "instance": instance, "profile": 0, "profiles": [{"identity": "Player", "name": "Player"}],
        "settings": {"swap_ab": False, "swap_xy": False, "touch": True}, "servers": [], "server": None,
    }}
    (app / "launcher/settings.0.json").write_text(json.dumps(settings), encoding="utf-8")
    if args.capture:
        (app / "gx2-profile.enable").touch()
    hashes = {path.relative_to(args.lab).as_posix(): sha256(path) for path in inputs}
    (args.lab / "input-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n", encoding="utf-8")


def start(lab, cpu_mode="interpreter", title_id=None):
    if not (lab / "input-hashes.json").is_file() or (lab / "process.txt").exists():
        raise ValueError("prepare a fresh lab before starting Cemu")
    env = os.environ.copy()
    for key in ("HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME",
                "HORIZON_LAUNCHER_TRASH", "HORIZON_LAUNCHER_CLIPBOARD", "HORIZON_LAUNCHER_DESKTOP"):
        scratch = lab / "scratch" / key
        scratch.mkdir(parents=True)
        env[key] = str(scratch)
    env.update(HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1")
    rpx = lab / "HorizonWiiU.rpx"
    title_id = title_id or standalone_title_id(rpx)
    if not re.fullmatch(r"[0-9a-f]{16}", title_id):
        raise ValueError("expected a 16-digit Cemu title ID")
    profiles = paths(lab)[0] / "gameProfiles"
    profiles.mkdir(exist_ok=True)
    profile = profiles / f"{title_id}.ini"
    with profile.open("x", encoding="ascii") as output:
        output.write(f"[General]\nstartWithPadView = true\n[CPU]\ncpuMode = {CPU_MODES[cpu_mode]}\n")
    launch = {
        "requested_cpu_mode": cpu_mode,
        "title_id": title_id,
        "cemu_sha256": sha256(lab / "Cemu.exe"),
        "rpx_sha256": sha256(rpx),
        "game_profile_sha256": sha256(profile),
    }
    (lab / "launch.json").write_text(json.dumps(launch, indent=2) + "\n", encoding="utf-8")
    flags = ["--force-interpreter"] if cpu_mode == "interpreter" else []
    with (lab / "stdout.log").open("xb") as stdout, (lab / "stderr.log").open("xb") as stderr:
        proc = subprocess.Popen([str(lab / "Cemu.exe"), *flags, "-g", str(rpx)],
                                cwd=lab, env=env, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
    (lab / "process.txt").write_text(str(proc.pid), encoding="ascii")
    return proc


def observed_mode(trace):
    if "Recompiler disabled. Command line --force-interpreter was passed" in trace:
        return "interpreter"
    mode = re.search(r"CPU-Mode:\s*(\d+)(?:\s*\(gameprofile\))?", trace)
    if mode:
        return next((name for name, number in CPU_MODES.items() if number == int(mode[1])), None)
    return None


def verified_pid(lab):
    pid = int((lab / "process.txt").read_text(encoding="ascii"))
    result = subprocess.run(
        ["powershell.exe", "-NoProfile", "-Command", f"(Get-Process -Id {pid} -ErrorAction Stop).Path"],
        check=True, capture_output=True, text=True, timeout=10,
    )
    if Path(result.stdout.strip()) != lab / "Cemu.exe":
        raise ValueError("process does not belong to the lab")
    return pid


def input_window(pid, prefix):
    user = ctypes.windll.user32
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.EnumWindows.argtypes = [callback, wintypes.LPARAM]
    user.EnumChildWindows.argtypes = [wintypes.HWND, callback, wintypes.LPARAM]
    parent = []

    @callback
    def find(window, _):
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(window, ctypes.byref(owner))
        if owner.value == pid:
            title = ctypes.create_unicode_buffer(256)
            user.GetWindowTextW(window, title, len(title))
            if title.value.startswith(prefix):
                parent.append(window)
                return False
        return True

    user.EnumWindows(find, 0)
    if not parent:
        raise ValueError(f"missing Cemu window {prefix}")
    children = []

    @callback
    def add(window, _):
        children.append(window)
        return True

    user.EnumChildWindows(parent[0], add, 0)
    return user, parent + children


def send(args, pid, key=None):
    user, windows = input_window(pid, "GamePad View" if key is None else "Cemu ")
    post = user.PostMessageW
    post.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    if key is None:
        if not (0 <= args.x < 854 and 0 <= args.y < 480):
            raise ValueError("coordinates must be within the 854×480 GamePad image")
        window = windows[min(1, len(windows) - 1)]
        rect = wintypes.RECT()
        user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        if not user.GetClientRect(window, ctypes.byref(rect)):
            raise ValueError("missing GamePad viewport")
        point = (args.x * rect.right // 854) | ((args.y * rect.bottom // 480) << 16)
        post(window, 0x200, 0, point)
        post(window, 0x201, 1, point)
        time.sleep(args.hold)
        post(window, 0x202, 0, point)
    else:
        if args.action == "key":
            try:
                _, gamepad = input_window(pid, "GamePad View")
                windows.extend(gamepad)
            except ValueError:
                pass
        scan = user.MapVirtualKeyW(key, 0) << 16
        for window in windows:
            post(window, 0x100, key, scan | 1)
        time.sleep(args.hold)
        for window in windows:
            post(window, 0x101, key, scan | 0xC0000001)


def run(args):
    if args.action == "prepare":
        prepare(args)
        return
    if args.action == "start":
        start(args.lab, args.cpu_mode, args.title_id)
        return
    pid = verified_pid(args.lab)
    portable, _ = paths(args.lab)
    if args.action == "stop":
        subprocess.run(["taskkill", "/PID", str(pid), "/F"], check=True)
    elif args.action == "wait":
        log = portable / "log.txt"
        deadline = time.monotonic() + args.timeout
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = (wintypes.HANDLE, wintypes.DWORD)
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.CloseHandle.argtypes = (wintypes.HANDLE,)
        process = kernel.OpenProcess(0x00100000, False, pid)
        if not process:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            while time.monotonic() < deadline:
                trace = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
                if args.marker in trace:
                    return
                if "Crashlog for Cemu" in trace or kernel.WaitForSingleObject(process, 200) != 258:
                    raise ValueError(f"Cemu stopped before marker: {args.marker}")
        finally:
            kernel.CloseHandle(process)
        raise ValueError(f"log marker not observed: {args.marker}")
    elif args.action == "shot":
        screenshots = portable / "screenshots"
        existing = set(screenshots.glob("*.png"))
        send(args, pid, 123)
        complete = {}
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline and len(complete) < 2:
            for path in sorted(set(screenshots.glob("*.png")) - existing):
                try:
                    with path.open("rb") as image:
                        image.seek(-12, os.SEEK_END)
                        if image.read() == bytes.fromhex("0000000049454e44ae426082"):
                            complete["gamepad" if path.stem.endswith("_GamePad") else "tv"] = path
                except OSError:
                    continue
            time.sleep(0.1)
        if len(complete) != 2:
            raise ValueError("Cemu did not produce two complete PNGs")
        if not args.name.replace("-", "").replace("_", "").isalnum():
            raise ValueError("invalid screenshot name")
        for name, path in complete.items():
            shutil.copyfile(path, args.lab / f"{args.name}-{name}.png")
    else:
        send(args, pid, args.key if args.action == "key" else None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lab", type=Path)
    subs = parser.add_subparsers(dest="action", required=True)
    setup = subs.add_parser("prepare")
    for name in ("cemu", "rpx", "template", "game", "scratch-world"):
        setup.add_argument(f"--{name}", type=Path, required=name in ("cemu", "rpx", "template"))
    setup.add_argument("--capture", action="store_true")
    setup.add_argument("--keep-fixture-distances", action="store_true",
                       help="copy the supplied options without overriding render and simulation distances")
    setup.add_argument("--default-instance", action="store_true")
    launch = subs.add_parser("start")
    launch.add_argument("--cpu-mode", choices=CPU_MODES, default="interpreter")
    launch.add_argument("--title-id", help="override the standalone RPX title ID")
    subs.add_parser("stop")
    wait = subs.add_parser("wait")
    wait.add_argument("marker")
    wait.add_argument("--timeout", type=float, default=30)
    tap = subs.add_parser("tap")
    tap.add_argument("x", type=int)
    tap.add_argument("y", type=int)
    key = subs.add_parser("key")
    key.add_argument("key", type=int)
    shot = subs.add_parser("shot")
    shot.add_argument("name")
    shot.add_argument("--timeout", type=float, default=30)
    for command in (tap, key, shot):
        command.add_argument("--hold", type=float, default=0.9)
    args = parser.parse_args()
    args.lab = args.lab.resolve()
    try:
        if os.name != "nt":
            raise ValueError("this driver requires Windows Cemu")
        run(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        print(f"Cemu fixture failed. {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
