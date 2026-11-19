#!/usr/bin/env python3
"""Record candidate Cafe input pins for review. Does not sign or update a recipe."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "wiiu/builder"))

from core import digest, file_digest, git
from native import BRIDGE_FILES, OVERLAYS, inventory_digest, native_toolchain


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--rust-source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    _, _, toolchain = native_toolchain()
    rust_source = args.rust_source.resolve(strict=True)
    names = git(rust_source, "diff", "--name-only", "-z", "HEAD").stdout
    modified = sorted(name.decode() for name in names.split(b"\0") if name)
    diff = git(rust_source, "diff", "--binary", "HEAD").stdout
    result = {
        "schema": 1, "version": "1.0.0", "toolchain": toolchain,
        "rust_source": {
            "commit": git(rust_source, "rev-parse", "HEAD").stdout.decode().strip(),
            "tracked_diff_sha256": digest(diff), "tracked_overlay_paths": modified,
            "cafe_filesystem_sha256": file_digest(rust_source / "library/std/src/sys/fs/cafe.rs"),
        },
        "overlay_sha256": {name: inventory_digest(args.inputs / name) for name in OVERLAYS},
        "workspace_patch_sha256": file_digest(args.inputs / "workspace.patch"),
        "bridge_inputs": {name: file_digest(ROOT / path) for name, path in BRIDGE_FILES.items()},
        "gitlinks": {"wiiu/bootstrap/.cafe": "7fe6bda717628e24f3b01c67fc330c92688a44e1"},
    }
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(result, stream, indent=2, sort_keys=True)
        stream.write("\n")


if __name__ == "__main__":
    main()
