#!/usr/bin/env python3
"""Run a Night benchmark and retrieve its report and HZP3 through one paired Codex session."""

import argparse
from hashlib import sha256
import json
import math
from pathlib import Path
import re
import time

from mcp_codex_cemu_test import paired_session
from wiiu_mcp_benchmark_capture import download, publish
from wiiu_mcp_lifecycle_test import response_text


PROFILES = ("4", "4q", "4r")
TERMINAL = {"completed", "cancelled", "failed", "aborted"}
PROGRESS = {phase: index for index, phase in enumerate(("queued", "loading", "running", "closing"))}
KNOWN_PHASES = PROGRESS.keys() | TERMINAL | {"idle"}
STATUS_FIELDS = ("phase", "instance", "scenario", "profile", "report_id", "capture_id",
                 "cancel_requested", "night_home_ready")
POLL_SECONDS = 0.75


class BenchmarkWaitTimeout(TimeoutError):
    pass


def json_object(call, name, args=None):
    reply = call(name, args)
    if reply.get("isError"):
        raise RuntimeError(f"{name} failed. {response_text(reply)[:300]}")
    value = json.loads(response_text(reply))
    if not isinstance(value, dict):
        raise ValueError(f"{name} returned an invalid object")
    return value


def benchmark_status(call, name="wiiu_benchmark_status", args=None):
    value = json_object(call, name, args)
    state = {field: value[field] for field in STATUS_FIELDS}
    if (not isinstance(state["phase"], str) or state["phase"] not in KNOWN_PHASES
            or any(state[field] is not None and not isinstance(state[field], str)
                   for field in ("instance", "scenario", "profile", "report_id", "capture_id"))
            or any(type(state[field]) is not bool for field in ("cancel_requested", "night_home_ready"))):
        raise ValueError(f"{name} returned an invalid benchmark state")
    if "run_id" in value:
        run_id = value["run_id"]
        if run_id is not None and (not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f]{32}", run_id)):
            raise ValueError(f"{name} returned an invalid benchmark run ID")
        state["run_id"] = run_id
    return state


def identity(state):
    return state["instance"], state["scenario"], state["profile"]


def wait_for_terminal(call, accepted, timeout, receipt, clock, pause):
    started = clock()
    deadline = started + timeout
    current = accepted
    terminal = None
    while clock() < deadline:
        state = benchmark_status(call)
        received = clock()
        receipt["last_observed_status"] = state
        if (identity(state) != identity(accepted) or state["phase"] == "idle"
                or state.get("run_id") != accepted.get("run_id")):
            raise RuntimeError("Night switched to another benchmark or restarted")
        if terminal is not None:
            if any(state[key] != terminal[key] for key in STATUS_FIELDS if key != "night_home_ready"):
                raise RuntimeError("Night replaced the observed terminal benchmark")
        elif state["phase"] in TERMINAL:
            terminal = receipt["terminal_status"] = state
            receipt["host_seconds"]["terminal_observed_after_start"] = received - started
        elif PROGRESS[state["phase"]] < PROGRESS[current["phase"]]:
            raise RuntimeError("Night benchmark status moved backward")
        if state != current:
            receipt["observations"].append({"seconds_after_start": received - started, **state})
            current = state
        if terminal is not None and state["night_home_ready"] and received <= deadline:
            receipt["terminal_status"] = state
            receipt["host_seconds"]["home_ready_after_start"] = received - started
            return state
        pause(max(0.0, min(POLL_SECONDS, deadline - received)))
    raise BenchmarkWaitTimeout("Night did not report the benchmark terminal at Home within the polling window")


def artifact_ids(state, existing):
    scenario, profile = state["scenario"], state["profile"]
    report_id, capture_id = state["report_id"], state["capture_id"]
    if not report_id or not re.fullmatch(rf"game-[0-9]{{14}}-{re.escape(scenario)}\.json", report_id):
        raise ValueError("Night returned an invalid report ID for this scenario")
    if report_id in existing:
        raise ValueError("Night returned a report that existed before this benchmark")
    if capture_id != f"{report_id.removesuffix('.json')}-{profile}.hzp":
        raise ValueError("Night returned a capture ID for another report or profile")
    return report_id, capture_id


def read_report(call, instance, scenario, report_id):
    envelope = json_object(call, "wiiu_benchmark_report", {"instance": instance, "id": report_id})
    if (envelope.get("instance"), envelope.get("id")) != (instance, report_id):
        raise ValueError("Night returned a different benchmark report or installation")
    report = envelope.get("report")
    if (not isinstance(report, dict)
            or (report.get("format"), report.get("kind")) != ("horizon-benchmark-report", "game")
            or type(report.get("version")) is not int or report["version"] != 1
            or not isinstance(report.get("conditions"), dict)
            or report["conditions"].get("scenario") != scenario):
        raise ValueError("Night returned an invalid game report or a different scenario")
    return report


def collect(call, instance, scenario, profile, timeout, receipt, *, clock=time.monotonic, pause=time.sleep):
    initial = receipt["initial_status"] = benchmark_status(call)
    if initial["phase"] not in TERMINAL | {"idle"} or not initial["night_home_ready"]:
        raise RuntimeError("Start with idle Night Home")
    # Omitting instance asks Night which installation it would launch.
    listing = json_object(call, "wiiu_benchmark_reports")
    if listing["instance"] != instance:
        raise RuntimeError("Night selected a different installation")
    if not isinstance(listing["reports"], list) or any(not isinstance(id_, str) for id_ in listing["reports"]):
        raise ValueError("Night returned an invalid benchmark report listing")
    existing = set(listing["reports"])
    if initial["instance"] == instance and initial["report_id"]:
        existing.add(initial["report_id"])

    args = {"scenario": scenario, "profile": profile}
    timings = receipt["host_seconds"]
    started = clock()
    accepted = benchmark_status(call, "wiiu_benchmark_start", args)
    timings["start_request"] = clock() - started
    receipt["start_status"] = receipt["last_observed_status"] = accepted
    if (identity(accepted) != (instance, scenario, profile) or accepted["phase"] != "queued"
            or accepted["report_id"] is not None or accepted["capture_id"] is not None
            or accepted["cancel_requested"]):
        raise RuntimeError("Night did not acknowledge the requested queued benchmark")
    receipt["observations"].append({"seconds_after_start": 0, **accepted})

    terminal = wait_for_terminal(call, accepted, timeout, receipt, clock, pause)
    if terminal["phase"] != "completed":
        raise RuntimeError(f"Night benchmark ended in phase {terminal['phase']}")
    report_id, capture_id = artifact_ids(terminal, existing)
    started = clock()
    report = read_report(call, instance, scenario, report_id)
    timings["report_retrieval"] = clock() - started
    started = clock()
    data, metadata = download(call, instance, capture_id)
    timings["capture_retrieval"] = clock() - started
    return report_id, report, capture_id, data, metadata


def main():
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog="Use the paired profile exclusively during this run. The polling window is checked "
               "between synchronous MCP calls. On timeout the waiter exits and leaves the console run "
               "alone because the current cancel tool has no run identity.",
    )
    parser.add_argument("--codex-home", type=Path, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--server", required=True, help="existing authenticated MCP server name")
    parser.add_argument("--instance", default="default", help="expected selected Night installation")
    parser.add_argument("--scenario", required=True, help="scenario supported by the selected Night build")
    parser.add_argument("--profile", required=True, choices=PROFILES)
    parser.add_argument("--wait-timeout", type=float, default=1800,
                        help="host polling window after start, in seconds")
    parser.add_argument("--output", type=Path, required=True, help="new directory for receipt and artifacts")
    args = parser.parse_args()
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", args.scenario) or len(args.scenario) > 60:
        parser.error("choose a valid Night scenario name")
    if not math.isfinite(args.wait_timeout) or args.wait_timeout <= 0:
        parser.error("provide a positive finite polling window")
    try:
        args.output.mkdir()
    except OSError as error:
        parser.exit(1, f"Choose a new output directory. {error}\n")

    receipt = {
        "status": "failed", "requested": {"instance": args.instance, "scenario": args.scenario, "profile": args.profile},
        "scope": "benchmark lifecycle and artifact retrieval",
        "timing_basis": "computer-side monotonic elapsed seconds", "host_seconds": {}, "observations": [],
        "terminal_status": None, "last_observed_status": None, "files": {},
    }
    error = None
    try:
        with paired_session(args.codex_home, args.ca, args.server) as session:
            result = collect(session.call, args.instance, args.scenario, args.profile, args.wait_timeout, receipt)
        report_id, report, capture_id, data, metadata = result
        report_bytes = (json.dumps(report, indent=2, allow_nan=False) + "\n").encode()
        for filename, content in ((report_id, report_bytes), (capture_id, data)):
            publish(args.output / filename, content)
            receipt["files"][filename] = {"bytes": len(content), "sha256": sha256(content).hexdigest()}
        receipt["capture_metadata"] = metadata
        receipt["status"] = "passed"
    except BenchmarkWaitTimeout as failure:
        receipt["status"] = "timed_out"
        error = failure
    except KeyboardInterrupt as failure:
        receipt["status"] = "interrupted"
        error = failure
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as failure:
        error = failure
    finally:
        if error is not None:
            receipt["error"] = str(error) or type(error).__name__
        with (args.output / "receipt.json").open("x", encoding="utf-8") as file:
            json.dump(receipt, file, indent=2, allow_nan=False)
            file.write("\n")
    if error is not None:
        parser.exit(1, f"Benchmark collection stopped. {receipt['error']}\n")


if __name__ == "__main__":
    main()
