#!/usr/bin/env python3
"""Run the pinned Cemu CR liveness regression with a host C++ compiler."""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

PINNED = "a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98"
JIT = Path("src/Cafe/HW/Espresso/Recompiler")
OPTIMIZER = JIT / "PPCRecompilerImlOptimizer.cpp"
HERE = Path(__file__).resolve().parent
PATCH = HERE.parent / "ppc-cr-liveness-cemu-2.6.patch"


def git_environment():
    env = os.environ.copy()
    for name in (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ):
        env.pop(name, None)
    return env


def git_file(repo, path):
    return subprocess.check_output(
        ["git", "--no-replace-objects", "-C", str(repo), "show", f"{PINNED}:{path.as_posix()}"],
        encoding="utf-8",
        env=git_environment(),
    )


def section(source, start, end):
    begin = source.index(start)
    return source[begin : source.index(end, begin)]


def compile_and_run(scratch, name, source, call_mask, local_mask, live_in):
    snippet = scratch / f"iml-{name}.inc"
    snippet.write_bytes(source.encode("utf-8"))
    suffix = ".exe" if os.name == "nt" else ""
    executable = scratch / f"{name}{suffix}"
    subprocess.run(
        [
            os.environ.get("CXX", "c++"),
            "-std=c++17",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-unused-parameter",
            "-Wno-unused-variable",
            f"-I{scratch}",
            f'-DIML_INPUT="{snippet.name}"',
            f"-DEXPECT_CALL={call_mask}",
            f"-DEXPECT_INLINE={local_mask}",
            f"-DEXPECT_MFCR_INPUT={live_in}",
            str(HERE / "optimizer.cpp"),
            "-o",
            str(executable),
        ],
        check=True,
    )
    print(f"{name}:", flush=True)
    subprocess.run([str(executable)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cemu_source", type=Path, help=f"Local Git repository containing {PINNED}")
    repo = parser.parse_args().cemu_source.resolve()
    analyzer = git_file(repo, JIT / "PPCRecompilerImlAnalyzer.cpp")
    intermediate = git_file(repo, JIT / "PPCRecompilerIntermediate.cpp")
    preamble = section(
        analyzer,
        "bool PPCRecompilerImlAnalyzer_canTypeWriteCR(",
        "void PPCRecompilerImlAnalyzer_getCRTracking(",
    )
    preamble += section(
        intermediate,
        "void PPCRecompilerIML_linkSegments(",
        "void PPCRecompilerIML_isolateEnterableSegments(",
    )
    original = git_file(repo, OPTIMIZER)

    with tempfile.TemporaryDirectory(prefix="horizon-cemu-cr-") as tmp:
        scratch = Path(tmp)
        target = scratch / OPTIMIZER
        target.parent.mkdir(parents=True)
        target.write_bytes(original.encode("utf-8"))
        for args in (["--check"], []):
            subprocess.run(
                ["git", "apply", "--no-index", *args, str(PATCH)],
                cwd=scratch,
                env=git_environment(),
                check=True,
            )
        patched = target.read_text(encoding="utf-8")

        for name, source, call_mask, local_mask, live_in in (
            ("original", original, 7, 15, "0u"),
            ("patched", patched, 0, 0, "0xffffffffu"),
        ):
            functions = section(
                source,
                "uint32 _PPCRecompiler_getCROverwriteMask(",
                "bool PPCRecompiler_checkIfGPRIsModifiedInRange(",
            )
            compile_and_run(scratch, name, preamble + functions, call_mask, local_mask, live_in)


if __name__ == "__main__":
    main()
