#!/usr/bin/env python3
"""Run the original offline Village benchmark in a prepared Windows Cemu lab."""

import argparse
import json
from pathlib import Path

import gx2_benchmark_report
import gx2_cemu
from gx2_cemu_acceptance import Lab
import gx2_profile


WORLD_DIMS = (1440, 809, 641, 360)
DISPLAY_DIMS = (1920, 1080, 854, 480)
TARGET_KEYS = ("tv_width", "tv_height", "gamepad_width", "gamepad_height",
               "tv_world_width", "tv_world_height", "gamepad_world_width", "gamepad_world_height")
FRAME_KEYS = ("tv_width", "tv_height", "drc_width", "drc_height",
              "tv_world_width", "tv_world_height", "drc_world_width", "drc_world_height")


def tap_and_wait(lab, x, y, marker):
    offset = lab.mark()
    lab.send("tap", x=x, y=y)
    lab.wait(marker + r"\r?$", 15, offset)


def verify_inputs(root, app, expected_rpx):
    manifest = root / "input-hashes.json"
    if not manifest.is_file() or (root / "process.txt").exists():
        raise ValueError("prepare a fresh disposable default-instance lab")
    if (app / ".benchmark-run").exists() or (app / "benchmarks").exists():
        raise ValueError("the original benchmark requires a fresh scratch workspace")
    marker = app / "gx2-profile.enable"
    if marker.exists() and marker.read_bytes() != b"4":
        raise ValueError("use plain profile 4 for the stock Village route")
    hashes = json.loads(manifest.read_bytes())
    archive = app / ".local/horizon-client/26.3/horizon-data.bin"
    immutable = (
        root / "HorizonWiiU.rpx", root / "Cemu.exe", root / "portable/settings.xml", archive,
        app / "options.txt", app / "horizon-graphics.txt", app / "horizon-controller.txt",
    )
    for path in immutable:
        name = path.relative_to(root).as_posix()
        if gx2_cemu.sha256(path) != hashes.get(name):
            raise ValueError(f"prepared input hash does not match for {name}")
    resource_manifest = archive.with_name("horizon-resources.json")
    manifest_name = resource_manifest.relative_to(root).as_posix()
    resource_digest = gx2_cemu.sha256(resource_manifest)
    if manifest_name in hashes and hashes[manifest_name] != resource_digest:
        raise ValueError("the prepared resource manifest changed")
    if hashes["HorizonWiiU.rpx"] != expected_rpx.lower():
        raise ValueError("the prepared RPX does not match the specified build")
    options = (app / "options.txt").read_text(encoding="utf-8").splitlines()
    graphics = (app / "horizon-graphics.txt").read_text(encoding="utf-8").splitlines()
    if not {"renderDistance:6", "simulationDistance:5"} <= set(options):
        raise ValueError("use stock Full Performance render and simulation distances")
    if not {"preset:full_performance", "resolutionScale:75", "dynamicResolution:false"} <= set(graphics):
        raise ValueError("use stock Full Performance, fixed 75% and DRS off")
    return {"rpx_sha256": hashes["HorizonWiiU.rpx"], "cemu_sha256": hashes["Cemu.exe"],
            "prepared_archive_sha256": hashes[archive.relative_to(root).as_posix()],
            "prepared_manifest_sha256": resource_digest}


def verify_flight(report, capture, serial):
    result = gx2_benchmark_report.summarize(report)
    settings = {"scenario": "village", "seed": "1234", "render_distance": 6,
                "simulation_distance": 5, "backend": "gx2", "offscreen": False}
    if (result["requested_duration_s"] != 12
        or any(result["conditions"].get(key) != value for key, value in settings.items())):
        raise ValueError("the guest did not publish the original stock Village conditions")
    targets = result["targets"]
    if (not targets or targets["configured_scale"] != 75 or targets["effective_scale"] != 75
        or tuple(targets[key] for key in TARGET_KEYS) != DISPLAY_DIMS + WORLD_DIMS):
        raise ValueError("the report did not retain the stock fixed-75 output and world targets")
    meta, frames = gx2_profile.read(capture)
    if meta["version"] != 3:
        raise ValueError("the guest did not publish a HZP3 session")
    trace = gx2_benchmark_report.village_profile(result, frames, serial=serial.splitlines())
    flight = [row for row in frames if trace["first_sequence"] <= row["sequence"] <= trace["last_sequence"]]
    if any((row["world_requested_scale"], row["world_effective_scale"]) != (75, 75)
           or tuple(row[key] for key in FRAME_KEYS) != DISPLAY_DIMS + WORLD_DIMS for row in flight):
        raise ValueError("scale or targets changed during the timed flight")
    result["village_profile"] = trace
    checkpoints = ("hosted save barrier completed", "hosted.after_game_drop bulk_free=234880920 bulk_largest=234880896",
                   "path=/vol/external01/wiiu/apps/HorizonWiiU/gx2-profile-session.hzp")
    if any(checkpoint not in serial for checkpoint in checkpoints):
        raise ValueError("save, full GX2 heap recovery or the session checkpoint was unobserved")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lab", type=Path)
    parser.add_argument("--rpx-sha256", required=True)
    args = parser.parse_args()
    root = args.lab.resolve(strict=True)
    _, app = gx2_cemu.paths(root)
    inputs = verify_inputs(root, app, args.rpx_sha256)
    marker = app / "gx2-profile.enable"
    if not marker.exists():
        with marker.open("xb") as output:
            output.write(b"4")

    lab = Lab(root, gx2_cemu.start(root, "multi"), require_distinct_captures=True)
    try:
        lab.wait(r"frame\.home\.play\r?$", 45)
        observed = gx2_cemu.observed_mode(lab.log.read_text(encoding="utf-8", errors="replace"))
        if observed != "multi":
            raise RuntimeError("Cemu did not enter multicore mode")
        tap_and_wait(lab, 165, 331, r"frame\.local\.settings")
        tap_and_wait(lab, 160, 310, r"frame\.local\.form")
        tap_and_wait(lab, 175, 222, r"frame\.home\.settings")
        lab.phase = "offline-stock-village"
        start = lab.mark()
        tap_and_wait(lab, 147, 170, r"frame\.loading")
        for stage in ("spawn", "village"):
            ready = lab.wait(
                rf"Cafe benchmark readiness {stage}: reason=ready elapsed_ms=(\d+) "
                r"columns=(\d+) minimum=90 generated=\d+ pending=0 terrain_paired=true "
                r"tv_sections=(\d+) gamepad_sections=(\d+)", 650, start,
            )
            elapsed, columns, tv, gamepad = map(int, ready.groups())
            if min(tv, gamepad) <= 0 or columns < 90:
                raise RuntimeError(f"the {stage} stock readiness gate reported inconsistent work")
            lab.record("benchmark-ready", stage=stage, elapsed_ms=elapsed, columns=columns,
                       tv_sections=tv, gamepad_sections=gamepad)
        lab.wait(r"frame\.local\.report\r?$", 90, start)
        paths = sorted((app / "benchmarks").glob("*-village.json"))
        if len(paths) != 1:
            raise RuntimeError("expected one original Village report")
        capture = app / "gx2-profile-session.hzp"
        serial = lab.log.read_text(encoding="utf-8", errors="replace")
        result = verify_flight(gx2_benchmark_report.load_report(paths[0]), capture.read_bytes(), serial)
        result["inputs"] = inputs
        lab.show("village-report")

        lab.phase = "close"
        tap_and_wait(lab, 120, 355, r"frame\.local\.reports")
        tap_and_wait(lab, 130, 221, r"frame\.local\.settings")
        tap_and_wait(lab, 128, 398, r"frame\.home\.play")
        close = lab.mark()
        tap_and_wait(lab, 123, 398, r"Horizon pregame: procui\.closed")
        lab.wait(r"coreinit\.exit\(0\)\r?$", 15, close)
        with (root / "verified-stock-village.json").open("x", encoding="utf-8", newline="\n") as output:
            json.dump(result, output, indent=2, allow_nan=False)
            output.write("\n")
        coverage = result["coverage"]["eligible_wall_fraction"]
        lab.record("village-completed", report_screenshots=True, eligible_terrain_fraction=coverage,
                   meets_terrain_coverage_target=coverage >= 0.95)
    finally:
        lab.close()


if __name__ == "__main__":
    main()
