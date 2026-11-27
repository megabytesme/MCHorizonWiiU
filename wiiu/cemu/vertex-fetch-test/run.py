#!/usr/bin/env python3
"""Compile the pinned Cemu emitter with and without this one-file patch."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile


BASE = "a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98"
LATTE = "src/Cafe/HW/Latte"
DECODER = f"{LATTE}/LegacyShaderDecompiler/LatteDecompilerEmitGLSLAttrDecoder.cpp"
HERE = Path(__file__).resolve().parent


def run(*command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def blob(repo, path):
    return run("git", "-C", str(repo), "show", f"{BASE}:{path}",
               stdout=subprocess.PIPE, text=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cemu_source", type=Path, help="Git repo with the pinned base")
    args = parser.parse_args()
    constant_source = blob(args.cemu_source, f"{LATTE}/Core/LatteConst.h")
    formats = re.findall(r"^#define\s+FMT_\w+\s+0x[\da-fA-F]+", constant_source, re.M)
    endian = re.search(r"enum class VertexFetchEndianMode\s*\{[^}]+\};", constant_source)
    assert formats and endian
    old = blob(args.cemu_source, DECODER)
    with tempfile.TemporaryDirectory(prefix="cemu-u16-host-") as path:
        work = Path(path)
        file = work / DECODER
        file.parent.mkdir(parents=True)
        file.write_text(old)
        run("git", "apply", "--check", str(HERE.parent / "scalar-u16-fetch-cemu-2.6.patch"), cwd=work)
        run("git", "apply", str(HERE.parent / "scalar-u16-fetch-cemu-2.6.patch"), cwd=work)
        new = file.read_text()
        fixture = "\n".join(formats) + f"\nnamespace LatteConst {{ {endian[0]} }}\n"
        # The full two bodies execute unmodified with test adapters for Cemu's
        # source append, renderer identification and unsupported-format logging.
        for name, source in (("before", old), ("after", new)):
            body = source[source.index("#define _CRLF"):]
            fixture += f"namespace {name} {{\n{body}\n#undef _CRLF\n}}\n"
        (work / "exact_emitter.inc").write_text(fixture)
        binary = work / "fetch-host"
        run(os.environ.get("CXX", "g++"), "-std=c++20", "-O2", "-Wall", "-Wextra",
            "-Werror", "-Wno-unused-parameter", "-I", str(work),
            str(HERE / "fetch_host.cpp"), "-lEGL", "-lGL", "-o", str(binary))
        env = os.environ.copy()
        env.update(EGL_PLATFORM="surfaceless", LIBGL_ALWAYS_SOFTWARE="1",
                   MESA_SHADER_CACHE_DISABLE="1")
        run(str(binary), env=env)


if __name__ == "__main__":
    main()
