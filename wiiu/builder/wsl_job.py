"""Keep a desktop job and its native child processes in one WSL session."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time


def main():
    # --exec bypasses shell startup. Read the user's login environment and
    # pass the argument list unchanged to Python.
    command = """
cd -- "$1" || exit 1
if [[ -n $2 ]]; then export DEVKITPRO="$2" DEVKITPPC="$2/devkitPPC"; fi
if [[ -n $3 ]]; then export HORIZON_WIIU_STD_RUSTC="$3"; fi
if [[ -n $DEVKITPRO ]]; then
    export PATH="$DEVKITPRO/tools/bin:$DEVKITPRO/portlibs/wiiu/bin:$DEVKITPPC/bin:$PATH"
fi
if [[ -n $4 ]]; then export PATH="$4:$PATH"; fi
shift 4
exec python3 -Bu "$@"
"""
    process = subprocess.Popen(["bash", "-lc", command, "wiiu-builder", os.getcwd(), *sys.argv[1:4],
                                str(Path(__file__).with_name("workflow.py").resolve()), *sys.argv[4:]],
                               stdin=subprocess.DEVNULL, start_new_session=True)
    cancelled = threading.Event()

    def watch():
        os.read(sys.stdin.fileno(), 1)
        if process.poll() is not None:
            return
        cancelled.set()
        for signum, wait in ((signal.SIGINT, 5), (signal.SIGTERM, 3), (signal.SIGKILL, 0)):
            try:
                os.killpg(process.pid, signum)
            except ProcessLookupError:
                return
            deadline = time.monotonic() + wait
            while time.monotonic() < deadline:
                process.poll()
                try:
                    os.killpg(process.pid, 0)
                except ProcessLookupError:
                    return
                time.sleep(0.1)

    monitor = threading.Thread(target=watch, daemon=True)
    monitor.start()
    result = process.wait()
    if cancelled.is_set():
        monitor.join(timeout=9)
        return 130
    return result if result >= 0 else 128 - result


if __name__ == "__main__":
    sys.exit(main())
