#!/usr/bin/env python3
"""Reject local game artifacts even when somebody uses git add -f."""

import argparse
from pathlib import PurePosixPath
import subprocess
import sys

PRIVATE_ROOTS = {".local", "decompiled", "minecraft", "assets", "run", "logs"}
PRIVATE_EXTENSIONS = {".jar", ".class", ".jfr", ".hprof", ".java", ".mca", ".mcr", ".rpx",
                      ".wuhb", ".rpl", ".elf", ".exe", ".dll", ".so", ".a", ".rlib",
                      ".pfx", ".p12", ".key", ".dpapi", ".crate", ".zip", ".7z"}
ORIGINAL_JDK_TOOLS = {"tools/JfrSummary.java", "tests/java/JfrFixture.java"}
# The prepared folder's files in one file (crates/horizon-core/src/prepared_files.rs): Mojang's data.
PRIVATE_NAMES = {"horizon-data.bin", "horizon-resources.json", "level.dat", "level.dat_old",
                 "launcher_accounts.json", "accounts.json", "access_token", "refresh_token",
                 "agent-token", "id_ed25519", "id_rsa", ".env"}
PRIVATE_FOLDERS = {".local", "saves", "playerdata", "datagen", "decompiled", ".minecraft"}


def prohibited(path):
    parsed = PurePosixPath(path)
    # Original JDK-only tools are an exact allowlist; recovered game Java stays private.
    if path in ORIGINAL_JDK_TOOLS:
        return False
    # Original Horizon mod sources may also be tracked.
    if parsed.suffix == ".java" and path.startswith(("mods/horizon/src/", "mods/horizon-lab/src/")):
        return not ("/dev/horizonlab/" in path)
    return (
        bool(parsed.parts and parsed.parts[0] in PRIVATE_ROOTS)
        or bool(set(parsed.parts[:-1]) & PRIVATE_FOLDERS)
        or parsed.suffix.lower() in PRIVATE_EXTENSIONS
        or parsed.name.lower() in PRIVATE_NAMES
    )


def paths_in_repository(history=False, revisions=None):
    if not history:
        result = subprocess.check_output(["git", "ls-files", "-z"])
        return result.decode("utf-8").split("\0")
    # An empty push (including deletions only) must not fall back to HEAD or --all.
    if revisions is not None and not revisions:
        return []
    roots = ["--all"] if revisions is None else revisions
    # Include diffs against merge parents so merge-resolution-only artifacts are caught.
    # Git pushes the stored objects, not the history presented by local replace refs.
    result = subprocess.check_output(["git", "--no-replace-objects", "log", "--format=", "--name-only",
                                      "-z", "-m", *roots, "--"])
    return [path.lstrip("\n") for path in result.decode("utf-8").split("\0")]


def pushed_revisions(stream):
    """Read Git's pre-push ref updates and retain the non-deleted local object IDs."""
    revisions = []
    for line in stream:
        _local_ref, local_oid, _remote_ref, _remote_oid = line.split()
        if local_oid.strip("0"):
            revisions.append(local_oid)
    return revisions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--history", action="store_true", help="Inspect paths in all local Git history")
    mode.add_argument("--pre-push", action="store_true", help="Inspect history reachable from Git's ref updates on stdin")
    # CI's pull requests: only the commits a merge would add, not every ref's whole history.
    mode.add_argument("--range", metavar="BASE..HEAD", help="Inspect the commits of a revision range")
    mode.add_argument("--ref", metavar="REVISION", help="Inspect all history reachable from this revision")
    args = parser.parse_args()
    selected = args.ref or args.range
    revisions = pushed_revisions(sys.stdin) if args.pre_push else [selected] if selected else None
    history = args.history or args.pre_push or bool(selected)
    rejected = sorted({path for path in paths_in_repository(history, revisions)
                       if path and prohibited(path)})
    if rejected:
        print("Private game/profiling artifacts must stay outside Git:", file=sys.stderr)
        for path in rejected:
            print(f"  {path}", file=sys.stderr)
        return 1
    print("Public tree check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
