#!/usr/bin/env python3
"""Reproduce the reviewed patch bytes from the original full-fork Git history."""
import argparse
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "wiiu/builder"))

from core import digest, git, isolated
from source import MANIFEST, NATIVE_TOOLS, read_recipe


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    recipe, _ = read_recipe(MANIFEST)
    output = isolated(args.output)
    if output.exists():
        parser.error("output directory already exists")
    excludes = [f":(exclude){x}" for x in ["wiiu", "third_party/getrandom-0.4.3", *sorted(NATIVE_TOOLS)]]
    patch_bytes = {}
    for patch in [*recipe["series"], recipe["diagnostic"]]:
        content = git(args.game_repo, "diff", "--binary", "--full-index", "--no-renames",
                      patch["from"], patch["to"], "--", ".",
                      *(excludes if patch != recipe["diagnostic"] else [])).stdout
        if digest(content) != patch["sha256"]:
            raise ValueError(f"generated patch differs from the pin: {patch['file']}")
        patch_bytes[patch["file"]] = content
    output.mkdir(parents=True)
    for name, payload in patch_bytes.items():
        path = output / name
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(payload)


if __name__ == "__main__":
    main()
