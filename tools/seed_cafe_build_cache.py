#!/usr/bin/env python3
"""Seed a new staged Nightly build from an earlier fixed-path Cargo cache."""

import argparse
import filecmp
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "wiiu" / "builder"))
from native import build_lock


def staged_files(root):
    source = root / "source"
    receipt = json.loads((root / "nightly-source-record.json").read_bytes())
    if receipt.get("kind") != "wiiu-hosted-nightly-assembly":
        raise ValueError(f"not a hosted Nightly assembly: {root}")
    if subprocess.check_output(["git", "-C", source, "write-tree"], text=True).strip() != receipt["tree"]:
        raise ValueError(f"staged source differs from its receipt: {source}")
    if subprocess.run(
        ["git", "-C", source, "diff", "--quiet", "--ignore-submodules=all"],
        check=False,
    ).returncode:
        raise ValueError(f"staged source has unstaged changes: {source}")
    rows = subprocess.check_output(["git", "-C", source, "ls-files", "--stage", "-z"])
    files = {}
    for entry in rows.split(b"\0"):
        if entry.startswith(b"100"):
            metadata, path = entry.split(b"\t", 1)
            files[Path("source", os.fsdecode(path))] = metadata.split()[1]
    return files


def stamp(previous, staged):
    old, new = previous.stat(), staged.stat()
    os.utime(staged, ns=(new.st_atime_ns, old.st_mtime_ns))


def seed(previous, staged, move_cache=False):
    previous, staged = previous.resolve(strict=True), staged.resolve(strict=True)
    if previous == staged or previous in staged.parents or staged in previous.parents:
        raise ValueError("the previous and staged outputs must be separate build directories")
    first, second = sorted((previous, staged))
    with build_lock(first), build_lock(second):
        return seed_locked(previous, staged, move_cache)


def seed_locked(previous, staged, move_cache):
    old_target = previous / "cargo-target"
    new_target = staged / "cargo-target"
    if (not old_target.is_dir() or old_target.is_symlink()
            or new_target.exists() or new_target.is_symlink()):
        raise ValueError("the previous output needs a Cargo target and the staged output must have none")
    if move_cache and old_target.stat().st_dev != staged.stat().st_dev:
        raise ValueError("moving the Cargo cache requires both outputs on one filesystem")

    old_files, new_files = staged_files(previous), staged_files(staged)
    count = 0
    for relative, digest in new_files.items():
        if old_files.get(relative) == digest:
            stamp(previous / relative, staged / relative)
            count += 1
    for new in (staged / "external").rglob("*"):
        old = previous / new.relative_to(staged)
        if (new.is_file() and old.is_file()
                and not new.is_symlink() and not old.is_symlink()
                and filecmp.cmp(old, new, shallow=False)):
            stamp(old, new)
            count += 1

    if move_cache:
        old_target.rename(new_target)
    else:
        shutil.copytree(old_target, new_target, symlinks=True)
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("previous", type=Path, help="earlier fixed-path hosted Nightly build")
    parser.add_argument("staged", type=Path, help="fresh assemble-hosted-nightly output")
    parser.add_argument("--move-cache", action="store_true",
                        help="transfer the Cargo target on the same filesystem without copying")
    args = parser.parse_args()
    try:
        count = seed(args.previous, args.staged, args.move_cache)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Could not seed Cafe build cache. {error}\n")
    action = "Moved" if args.move_cache else "Copied"
    print(f"{action} Cargo cache and restored mtimes for {count} unchanged files.")


if __name__ == "__main__":
    main()
