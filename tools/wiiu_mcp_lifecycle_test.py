#!/usr/bin/env python3
"""Qualify Night startup cancellation and benchmark lifecycle with an existing Codex grant."""

import argparse
from contextlib import suppress
import json
import math
from pathlib import Path
import re
import time

from mcp_codex_cemu_test import paired_session


def response_text(reply):
    for block in reply.get("content", []):
        if block.get("type") == "text" and isinstance(block.get("text"), str):
            return block["text"]
    raise ValueError("Night returned no text result")


def launch_id(reply):
    structured = reply.get("structuredContent")
    if isinstance(structured, dict):
        identifier = structured.get("launch_id")
    else:
        match = re.search(r"\bLaunch ID: ([0-9a-f]{32})\b", response_text(reply))
        identifier = match[1] if match else None
    if not isinstance(identifier, str) or not re.fullmatch(r"[0-9a-f]{32}", identifier):
        raise ValueError("Night returned no valid ordinary launch ID")
    return identifier


def qualify(call, world, instance, timeout, close):
    pending_cancel = None

    def status(tool, args=None):
        return json.loads(response_text(call(tool, args)))

    def wait_status(tool, args, match, unexpected):
        deadline = time.monotonic() + timeout
        observed = []
        while time.monotonic() < deadline:
            row = status(tool, args)
            phase = row["phase"]
            if not observed or observed[-1] != phase:
                observed.append(phase)
            if match(row):
                return observed
            if phase in unexpected:
                raise RuntimeError(f"{tool} reached unexpected phase {phase}")
            time.sleep(0.75)
        raise TimeoutError(f"{tool} did not settle within the qualification window")

    initial = status("wiiu_benchmark_status")
    if initial["phase"] not in {"idle", "completed", "cancelled", "aborted", "failed"} or not initial["night_home_ready"]:
        raise RuntimeError("Leave the verified disposable installation at idle Night Home before testing")
    report_arguments = {"instance": instance}
    report_ids = status("wiiu_benchmark_reports", report_arguments)["reports"]
    failed = {"failed", "aborted", "completed", "handoff_started"}
    try:
        identifier = launch_id(call("play", {"instance": instance, "world": world}))
        launch = {"launch_id": identifier}
        pending_cancel = ("wiiu_loading_cancel", launch)
        phases = wait_status("wiiu_loading_status", launch,
                             lambda row: row["phase"] == "loading", failed | {"cancelled"})
        call(*pending_cancel)
        phases.extend(wait_status("wiiu_loading_status", launch,
                                  lambda row: row["phase"] == "cancelled", failed))
        pending_cancel = None
        wait_status("wiiu_benchmark_status", None, lambda row: row["night_home_ready"], set())

        trials = []
        for mode in ("4q", "4r"):
            reply = call("wiiu_benchmark_start", {"scenario": "cafe-stationary", "profile": mode})
            pending_cancel = ("wiiu_benchmark_cancel", None)
            trial = json.loads(response_text(reply))
            if trial["instance"] != instance or trial["profile"] != mode:
                raise RuntimeError("Night started the benchmark with unexpected settings")
            observed = wait_status("wiiu_benchmark_status", None,
                                   lambda row: row["phase"] in {"loading", "running"}, failed | {"cancelled"})
            call(*pending_cancel)
            observed.extend(wait_status("wiiu_benchmark_status", None,
                                        lambda row: row["phase"] == "cancelled" and row["night_home_ready"],
                                        failed))
            pending_cancel = None
            terminal = status("wiiu_benchmark_status")
            if (terminal["profile"] != mode or terminal["report_id"] is not None
                    or terminal["capture_id"] is not None
                    or status("wiiu_benchmark_reports", report_arguments)["reports"] != report_ids):
                raise RuntimeError("Cancelled trial altered the saved benchmark reports")
            trials.append({"profile": mode, "phases": observed, "home_ready": True})

        if close:
            call("wiiu_close_to_menu")
        return {"ordinary_cancel_phases": phases, "stationary_cancel_trials": trials,
                "reports_unchanged": True, "night_close_requested": close}
    finally:
        if pending_cancel:
            with suppress(RuntimeError, OSError):
                call(*pending_cancel)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex-home", type=Path, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--server", required=True, help="existing authenticated MCP server name")
    parser.add_argument("--world", required=True, help="an existing disposable save in the selected installation")
    parser.add_argument("--instance", default="default")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--close", action="store_true", help="request the ordinary close after all cancellation tests")
    args = parser.parse_args()
    if not (math.isfinite(args.timeout) and args.timeout > 0):
        parser.error("provide a positive finite timeout")
    with args.output.open("x", encoding="utf-8") as receipt:
        try:
            with paired_session(args.codex_home, args.ca, args.server) as session:
                result = qualify(session.call, args.world, args.instance, args.timeout, args.close)
        except (RuntimeError, TimeoutError, OSError, ValueError, KeyError) as error:
            json.dump({"status": "failed", "reason": str(error)}, receipt, indent=2)
            receipt.write("\n")
            raise SystemExit(f"Night lifecycle qualification failed. {error}") from error
        json.dump({"status": "passed", **result}, receipt, indent=2)
        receipt.write("\n")


if __name__ == "__main__":
    main()
