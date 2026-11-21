#!/usr/bin/env python3
"""Prepare and drive an isolated Windows Cemu fixture for the hosted GX2 port."""

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def paths(lab):
    portable = lab / "portable"
    return portable, portable / "sdcard/wiiu/apps/HorizonWiiU"


def prepare(args):
    portable, app = paths(args.lab)
    game = app / "instances/Lab"
    args.lab.mkdir(parents=True)
    game.mkdir(parents=True)
    (app / "launcher").mkdir()
    shutil.copyfile(args.cemu, args.lab / "Cemu.exe")
    shutil.copyfile(args.rpx, args.lab / "HorizonWiiU.rpx")
    shutil.copyfile(args.template / "settings.xml", portable / "settings.xml")
    shutil.copytree(args.template / "controllerProfiles", portable / "controllerProfiles")
    inputs = [args.lab / "Cemu.exe", args.lab / "HorizonWiiU.rpx", portable / "settings.xml"]
    if args.game:
        subpath = ".local/horizon-client/26.3"
        resources = args.game / subpath
        if not (resources / "horizon-resources.json").is_file():
            raise ValueError("fixture game directory has no prepared 26.3 manifest")
        result = subprocess.run(
            ["robocopy", str(resources), str(game / subpath), "/E", "/MT:32",
             "/NJS", "/NJH", "/NP", "/NFL", "/NDL"], check=False,
        )
        if result.returncode >= 8:
            raise ValueError(f"resource copy failed ({result.returncode})")
        inputs.append(game / subpath / "horizon-resources.json")
        if (game / subpath / "horizon-data.bin").is_file():
            inputs.append(game / subpath / "horizon-data.bin")
        for name in ("options.txt", "horizon-controller.txt", "horizon-graphics.txt"):
            if (args.game / name).is_file():
                shutil.copyfile(args.game / name, game / name)
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
        "instance": "Lab", "profile": 0, "profiles": [{"identity": "Player", "name": "Player"}],
        "settings": {"swap_ab": False, "swap_xy": False, "touch": True}, "servers": [], "server": None,
    }}
    (app / "launcher/settings.0.json").write_text(json.dumps(settings), encoding="utf-8")
    if args.capture:
        (app / "gx2-profile.enable").touch()
    hashes = {}
    for path in inputs:
        with path.open("rb") as handle:
            hashes[path.relative_to(args.lab).as_posix()] = hashlib.file_digest(handle, "sha256").hexdigest()
    (args.lab / "input-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n", encoding="utf-8")


def start(lab):
    if not (lab / "input-hashes.json").is_file() or (lab / "process.txt").exists():
        raise ValueError("prepare a fresh lab before starting Cemu")
    env = os.environ.copy()
    for key in ("HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME",
                "HORIZON_LAUNCHER_TRASH", "HORIZON_LAUNCHER_CLIPBOARD", "HORIZON_LAUNCHER_DESKTOP"):
        scratch = lab / "scratch" / key
        scratch.mkdir(parents=True)
        env[key] = str(scratch)
    env.update(HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1")
    info = subprocess.STARTUPINFO()
    info.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    proc = subprocess.Popen([str(lab / "Cemu.exe"), "--force-interpreter", "-g", str(lab / "HorizonWiiU.rpx")],
                            cwd=lab, env=env, startupinfo=info, stdin=subprocess.DEVNULL,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    (lab / "process.txt").write_text(str(proc.pid), encoding="ascii")


def verified_pid(lab):
    pid = int((lab / "process.txt").read_text(encoding="ascii"))
    result = subprocess.run(
        ["powershell.exe", "-NoProfile", "-Command", f"(Get-Process -Id {pid} -ErrorAction Stop).Path"],
        check=True, capture_output=True, text=True,
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
        start(args.lab)
        return
    pid = verified_pid(args.lab)
    portable, _ = paths(args.lab)
    if args.action == "stop":
        subprocess.run(["taskkill", "/PID", str(pid), "/F"], check=True)
    elif args.action == "wait":
        log = portable / "log.txt"
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            if log.is_file() and args.marker in log.read_text(encoding="utf-8", errors="replace"):
                return
            time.sleep(0.2)
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
    subs.add_parser("start")
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
