#!/usr/bin/env python3
"""Stage a reviewed asset-free combined client locally, without uploading it."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "wiiu/builder"))

import core
import native
import source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("record", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--redistribution-notices", type=Path, required=True,
                        help="reviewed linked-dependency notices and corresponding-source information")
    parser.add_argument("--approved-rpx-sha256", required=True, help="RPX hash from release review")
    args = parser.parse_args()
    try:
        record = core.parse_json(args.record.read_bytes())
        recipe, fingerprint = source.read_recipe(source.MANIFEST)
        hosted, hosted_hash, _ = native.hosted_inputs(recipe, fingerprint)
        if (record.get("kind"), record.get("build"), record.get("build_tree"),
                record.get("hosted_recipe_sha256"), record.get("resources_inventory_sha256", "missing")) != (
                "wiiu-hosted-game-build", "passed", hosted["output_tree"], hosted_hash, None):
            raise ValueError("build and verify the combined recipe without a private resource inventory")
        notices = args.redistribution_notices.read_text(encoding="utf-8")
        if not notices.strip():
            raise ValueError("reviewed redistribution notices are required")
        directory = args.record.resolve().parent
        core.verify_outputs(record, directory)
        rpx, wuhb = native.HOSTED_PACKAGES
        if record["outputs"][rpx]["sha256"] != args.approved_rpx_sha256:
            raise ValueError("RPX hash differs from the approved release candidate")
        wuhbtool = core.program("wuhbtool")
        if core.file_digest(wuhbtool) != native.toolchain_spec(recipe)["toolchain"]["files"]["wuhbtool"]:
            raise ValueError("installed wuhbtool differs from the toolchain pin")
        output = core.isolated(args.output)
        if output.exists():
            raise ValueError("choose a new candidate directory")
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="wiiu-prebuilt-", dir=output.parent) as scratch:
            candidate = Path(scratch)
            shutil.copyfile(directory / rpx, candidate / rpx)
            core.run([wuhbtool, candidate / rpx, candidate / wuhb, "--name=Horizon Wii U", "--short-name=Horizon"])
            core.verify_outputs(record, candidate)
            (candidate / "REDISTRIBUTION.md").write_text(notices, encoding="utf-8", newline="\n")
            for name in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
                shutil.copyfile(ROOT / name, candidate / name)
            core.write_record(candidate / "SHA256.json",
                              {"client": hosted["hosted_client"], "patch": hosted["hosted_patch"],
                               "build_tree": hosted["output_tree"], "hosted_recipe_sha256": hosted_hash,
                               "outputs": record["outputs"]})
            candidate.rename(output)
        print(output / "SHA256.json")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Prebuilt release preparation failed: {error}\n")


if __name__ == "__main__":
    main()
