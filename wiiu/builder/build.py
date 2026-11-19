#!/usr/bin/env python3
"""Verify and build a local Horizon Wii U port recipe."""
import argparse
from pathlib import Path
import subprocess
import sys

import core
import native
import source


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", action="version", version=core.VERSION)
    parser.add_argument("--manifest", type=Path, default=source.MANIFEST)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("validate", "assemble", "build"):
        command = commands.add_parser(name)
        command.add_argument("--source", type=Path, required=True)
        command.add_argument("--revision", default="HEAD")
        if name != "validate":
            command.add_argument("--output", type=Path, required=True)
        if name == "assemble":
            command.add_argument("--diagnostic", action="store_true")
    game = commands.add_parser("build-game")
    game.add_argument("--assembly", type=Path, required=True)
    game.add_argument("--inputs", type=Path, required=True)
    game.add_argument("--output", type=Path, required=True)
    game.add_argument("--resources", type=Path)
    game.add_argument("--resource-manifest", type=Path)
    resources = commands.add_parser("inventory-resources")
    resources.add_argument("root", type=Path)
    resources.add_argument("--output", type=Path, required=True)
    verify = commands.add_parser("verify-output")
    verify.add_argument("record", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "verify-output":
            core.verify_outputs(core.parse_json(args.record.read_bytes()), args.record.resolve().parent)
            print("RPX and WUHB hashes verified.")
        elif args.command == "inventory-resources":
            print(core.inspect_resources(args.root, args.output))
        else:
            recipe, fingerprint = source.read_recipe(args.manifest)
            if args.command == "build-game":
                if bool(args.resources) != bool(args.resource_manifest):
                    raise ValueError("provide both --resources and --resource-manifest")
                asset_hash = None
                if args.resources:
                    asset_hash = native.verify_resources(args.resources, args.resource_manifest)
                print(native.build_game(recipe, fingerprint, args.assembly.resolve(),
                                        args.inputs.resolve(), args.output, asset_hash))
            elif args.command == "assemble":
                print(source.assemble(args.source, args.revision, recipe, fingerprint,
                                      args.output, args.diagnostic))
            else:
                core.verify_source(args.source, args.revision, recipe["upstream"])
                source.native_files(recipe, core.ROOT)
                patches = core.ROOT / "patches" / recipe["upstream"]["commit"]
                for item in [*recipe["series"], recipe["diagnostic"]]:
                    source.checked_bytes(patches, item["file"], item["sha256"])
                source.checked_bytes(core.ROOT, native.INPUTS_FILE, recipe["inputs_sha256"])
                if args.command == "build":
                    print(core.build(recipe, fingerprint, args.output))
                else:
                    print(f"Verified local recipe {fingerprint}, exact upstream pin and patch hashes.")
    except subprocess.CalledProcessError as err:
        print(f"Builder failed (exit {err.returncode}): {err.cmd[0]}", file=sys.stderr)
        if err.stderr:
            print(err.stderr.decode(errors="replace").strip(), file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError) as err:
        print(f"Builder failed: {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
