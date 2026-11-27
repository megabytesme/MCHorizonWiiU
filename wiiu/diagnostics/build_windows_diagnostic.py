#!/usr/bin/env python3
"""Build a standalone Wii U C/GLSL diagnostic with the portable Windows SDK."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

GCC_VERSION = "16.2.0"
CAFEGLSL_SHA = "9657374d1dbce77e0fc3628757a5e9c4c44f3c462081d6feb2a88f7020779377"
PACKAGES = {
    "gcc": "ab245a7a29f7f7037169d5ad99d1c6577e9bc2339ba0ca9ec501acf72f6fc5f4",
    "binutils": "2a4009e734f7e43e565c0e0fed7facf95191e30d7fbd55d2732500b056cfa376",
    "newlib": "2c5277c6b07a5558c9fd8e631d958c6d7784860aa55f5c320cb51371bc07e21b",
    "rules": "0c3394da451c9dfb3b428d9d61b044eb1eafb947b2b5090ff88525f55c785278",
    "crtls": "5a1144d515579eee73bb936ca36a8ea739b00d8cf32af3fa4dda21a039881bcd",
    "wut": "b746f1a693c58b38dafbe7f54cfe6b29bb31010f54e51d7b0c8c5c6aa43f1ca1",
    "wut-tools": "aff35e2af329e2ecdf7586d0bed89a12ad75d67bba966fcb5882129cbad349b5",
}
TARGET_PINS = {
    "wut/lib/libwut.a": "wut/lib/libwut.a",
    "wut/share/wut.specs": "wut/share/wut.specs",
    "wut/share/wut.ld": "wut/share/wut.ld",
    "devkitPPC/powerpc-eabi/lib/libc.a": "gcc/libc.a",
    "devkitPPC/powerpc-eabi/lib/libm.a": "gcc/libm.a",
}
GCC_LIB = f"devkitPPC/lib/gcc/powerpc-eabi/{GCC_VERSION}"
GCC_EXEC = f"devkitPPC/libexec/gcc/powerpc-eabi/{GCC_VERSION}"
TOOL_RELS = {
    "gcc": "devkitPPC/bin/powerpc-eabi-gcc.exe",
    "objcopy": "devkitPPC/bin/powerpc-eabi-objcopy.exe",
    "elf2rpl": "tools/bin/elf2rpl.exe",
    "readrpl": "tools/bin/readrpl.exe",
    "wuhbtool": "tools/bin/wuhbtool.exe",
}
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_+-]*")
SHA = re.compile(r"[a-f0-9]{64}")


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load(path):
    def unique(pairs):
        result = dict(pairs)
        if len(result) != len(pairs):
            raise ValueError(f"duplicate JSON field in {path}")
        return result

    return json.loads(path.read_text(encoding="utf-8-sig"), object_pairs_hook=unique)


def name(value):
    if not NAME.fullmatch(value) or os.path.isreserved(value):
        raise ValueError(f"invalid identifier {value!r}")
    return value


def relative(value):
    fields = value.split("/")
    if any(
        not part or part in (".", "..") or "\\" in part or os.path.isreserved(part)
        for part in fields
    ):
        raise ValueError(f"expected a portable relative path, got {value!r}")
    return PurePosixPath(*fields)


def checked(path, wanted):
    if not SHA.fullmatch(wanted) or sha(path) != wanted:
        raise ValueError(f"SHA-256 mismatch: {path}")
    return wanted


def verify_sdk(sdk, receipt_path, spec_path):
    receipt = load(receipt_path)
    if set(receipt["packages"]) != set(PACKAGES) or set(receipt["files"]) != set(PACKAGES):
        raise ValueError("SDK receipt package set differs from the verified Windows staging")
    verified = {}
    for package, want in PACKAGES.items():
        if receipt["packages"][package][3] != want:
            raise ValueError(f"official archive pin differs: {package}")
        for rel, size, expected in receipt["files"][package]:
            path = sdk / relative(rel)
            if path.stat().st_size != size:
                raise ValueError(f"SDK file size differs: {path}")
            verified[rel] = checked(path, expected)
    for alias, target, expected in receipt["aliases"]:
        if verified.get(target) != expected:
            raise ValueError(f"SDK alias differs from its target: {alias}")
        verified[alias] = checked(sdk / relative(alias), expected)
    required = {
        *TOOL_RELS.values(), *TARGET_PINS,
        "devkitPPC/powerpc-eabi/bin/as.exe", "devkitPPC/powerpc-eabi/bin/ld.exe",
        f"{GCC_EXEC}/cc1.exe", f"{GCC_EXEC}/collect2.exe",
        f"{GCC_LIB}/libgcc.a", f"{GCC_LIB}/ecrti.o", f"{GCC_LIB}/ecrtn.o",
    }
    if missing := required - set(verified):
        raise ValueError(f"required SDK receipts are missing: {sorted(missing)}")
    present = {path.relative_to(sdk).as_posix() for path in sdk.rglob("*") if path.is_file()}
    if extra := present - set(verified):
        raise ValueError(f"SDK has files outside the pinned receipt: {sorted(extra)}")
    linux = load(spec_path)["toolchain"]["files"]
    for rel, pin in TARGET_PINS.items():
        if verified[rel] != linux[pin]:
            raise ValueError(f"common target runtime differs from canonical pin: {rel}")
    return {key: sdk / rel for key, rel in TOOL_RELS.items()}


def inputs(recipe, roots):
    verified = {}
    lower_paths = set()
    for key, expected in recipe["inputs"].items():
        if key.casefold() in lower_paths:
            raise ValueError(f"input paths collide on Windows: {key}")
        lower_paths.add(key.casefold())
        rel = relative(key)
        root_name, *rest = rel.parts
        if root_name not in roots or not rest:
            raise ValueError(f"missing source root or relative filename: {key}")
        original = roots[root_name].joinpath(*rest)
        data = original.read_bytes()
        if not SHA.fullmatch(expected) or hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"source SHA-256 mismatch: {original}")
        verified[key] = data
    return verified


def references(recipe, verified):
    if recipe["schema"] != 1:
        raise ValueError("unsupported fixture recipe")
    names = set()

    def unique_object(stem):
        stem = name(stem)
        if stem.casefold() in names:
            raise ValueError(f"duplicate object name: {stem}")
        names.add(stem.casefold())

    if not recipe["sources"]:
        raise ValueError("need at least one C source")
    for source in recipe["sources"]:
        if source["path"] not in verified or not source["path"].endswith(".c"):
            raise ValueError(f"unpinned or non-C source: {source['path']}")
        unique_object(source["object"])
    for shader in recipe.get("shaders", []):
        unique_object(shader["name"])
        if "gfd" in shader:
            gfd = shader["gfd"]
            if "vertex" in shader or "pixel" in shader or gfd not in verified:
                raise ValueError(f"expected one pinned prebuilt GFD: {shader['name']}")
            if recipe["inputs"][gfd] != shader["sha256"]:
                raise ValueError(f"prebuilt GFD pin differs from source: {gfd}")
        elif shader["vertex"] not in verified or shader["pixel"] not in verified:
            raise ValueError(f"unpinned shader input: {shader['name']}")
        if not SHA.fullmatch(shader["sha256"]):
            raise ValueError(f"missing GFD pin: {shader['name']}")
        if shader.get("uniform_mode", "auto") not in {"auto", "block", "register"}:
            raise ValueError(f"invalid shader uniform mode: {shader['name']}")
    for include in recipe.get("includes", []):
        relative(include)
        if not any(key.startswith(include + "/") for key in verified):
            raise ValueError(f"include directory contains no pinned input: {include}")
    for define in recipe.get("defines", []):
        if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*(?:=[^" + r"\x00\r\n" + r"]+)?", define):
            raise ValueError(f"invalid preprocessor definition: {define!r}")
    for lib in recipe.get("libraries", []):
        name(lib)
    name(recipe["name"])


def environment(sdk, output):
    blocked = {
        "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "OBJC_INCLUDE_PATH",
        "LIBRARY_PATH", "COMPILER_PATH", "GCC_EXEC_PREFIX",
    }
    env = {key: value for key, value in os.environ.items() if key.upper() not in blocked}
    env.update({
        "DEVKITPRO": sdk.as_posix(), "DEVKITPPC": (sdk / "devkitPPC").as_posix(),
        "TEMP": str(output / "tmp"), "TMP": str(output / "tmp"),
        "PATH": os.pathsep.join(str(p) for p in (
            sdk / "devkitPPC/bin", sdk / "tools/bin", Path(os.environ["SystemRoot"]) / "System32"
        )),
    })
    return env


def build(args, recipe, verified, tools):
    out = args.output.resolve()
    out.mkdir()
    (out / "tmp").mkdir()
    for rel, data in verified.items():
        dest = out / "src" / relative(rel)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    sdk = args.sdk_root.resolve()
    wut = sdk / "wut"
    env = environment(sdk, out)
    commands = []

    def run(argv, log=None):
        argv = [os.fspath(arg) for arg in argv]
        commands.append(argv)
        proc = subprocess.run(argv, cwd=out, env=env, capture_output=True)
        if log or proc.returncode:
            (out / (log or "failed-tool.txt")).write_bytes(proc.stdout + proc.stderr)
        if proc.returncode:
            raise RuntimeError(f"{argv[0]} exited {proc.returncode}; see {out / (log or 'failed-tool.txt')}")

    cc = [
        tools["gcc"], f"-B{(sdk / 'devkitPPC/powerpc-eabi/bin').as_posix()}/",
        f"-B{(sdk / GCC_LIB).as_posix()}/",
        "-std=gnu11", "-O2", "-Wall", "-Wextra", "-Werror",
        "-D__WIIU__", "-mcpu=750", "-meabi", "-mhard-float",
        "-ffunction-sections", "-fdata-sections", "-fno-use-linker-plugin", "-I", wut / "include",
    ]
    cc.extend("-D" + flag for flag in recipe.get("defines", []))
    for directory in recipe.get("includes", []):
        cc.extend(("-I", f"src/{directory}"))
    compiled = []
    for source in recipe["sources"]:
        obj = source["object"] + ".o"
        run([*cc, "-c", f"src/{source['path']}", "-o", obj], log=f"{obj}.txt")
        compiled.append(obj)

    shader_obj, shaders = [], []
    for shader in recipe.get("shaders", []):
        stem = shader["name"]
        gsh = stem + ".gsh"
        mode = shader.get("uniform_mode")
        mode_args = [] if mode is None else ["--uniform-mode", mode]
        if "gfd" in shader:
            (out / gsh).write_bytes(verified[shader["gfd"]])
        else:
            run([
                args.cafeglsl.resolve(), *mode_args, "-vs", f"src/{shader['vertex']}",
                "-ps", f"src/{shader['pixel']}", "-o", gsh, "-v",
            ], log=f"{stem}.disasm.txt")
        checked(out / gsh, shader["sha256"])
        obj = stem + ".o"
        run([tools["objcopy"], "-I", "binary", "-O", "elf32-powerpc", "-B", "powerpc:common", gsh, obj])
        shaders.append(gsh)
        shader_obj.append(obj)

    label = recipe["name"]
    elf, rpx = label + ".elf", label + ".rpx"
    run([
        *cc, f"-specs={wut / 'share/wut.specs'}", "-L", wut / "lib", "-Wl,--gc-sections",
        *compiled, *shader_obj, "-lwut", *("-l" + lib for lib in recipe.get("libraries", [])),
        "-o", elf,
    ], log="link.txt")
    hdr = (out / elf).read_bytes()[:20]
    if hdr[:6] != b"\x7fELF\x01\x02" or hdr[16:20] != b"\x00\x02\x00\x14":
        raise ValueError("linker did not emit a big-endian ELF32 PowerPC executable")
    run([tools["elf2rpl"], elf, rpx])
    run([tools["readrpl"], "-a", rpx], log="readrpl.txt")
    filenames = [*shaders, *compiled, *shader_obj, elf, rpx]
    if package := recipe.get("wuhb"):
        wuhb = label + ".wuhb"
        run([
            tools["wuhbtool"], rpx, wuhb, f"--name={package['name']}",
            f"--short-name={package['short_name']}", f"--author={package['author']}",
        ])
        if (out / rpx).read_bytes() not in (out / wuhb).read_bytes():
            raise ValueError("the package does not contain the exact generated RPX")
        filenames.append(wuhb)
    outputs = {filename: sha(out / filename) for filename in filenames}
    matches = {}
    if args.reference:
        # Only compare artifact names the reference receipt declares. Compare
        # their actual bytes and their receipt pins to the fresh build.
        reference_dir = args.reference.resolve().parent
        reference = load(args.reference)["output_sha256"]
        if not reference or not set(reference) <= set(outputs) or not {elf, rpx} <= set(reference):
            raise ValueError("reference must pin the same ELF/RPX and known artifact names")
        for filename, want in reference.items():
            checked(reference_dir / filename, want)
            if outputs[filename] != want:
                raise ValueError(f"native rebuild differs: {filename}")
            matches[filename] = want
    receipt = {
        "recipe_sha256": sha(args.recipe), "sdk_receipt_sha256": sha(args.sdk_receipt),
        "toolchain_spec_sha256": sha(args.toolchain_spec), "compiler_sha256": CAFEGLSL_SHA,
        "inputs": recipe["inputs"], "output_sha256": outputs, "reference_matches": matches,
        "commands": commands,
    }
    (out / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(f"Built {out / rpx}")
    if matches:
        print(f"Byte-exact rebuild matches {len(matches)} retained artifacts")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.platform != "win32" or sys.version_info < (3, 13):
        parser.error("this diagnostic builder requires Windows and Python 3.13 or newer")
    for option in ("sdk-root", "sdk-receipt", "toolchain-spec", "cafeglsl", "recipe", "output"):
        parser.add_argument("--" + option, type=Path, required=True)
    parser.add_argument("--root", nargs=2, action="append", metavar=("NAME", "DIRECTORY"), required=True)
    parser.add_argument("--reference", type=Path, help="optional retained build-receipt.json")
    args = parser.parse_args()
    roots = {name(key): Path(path).resolve(strict=True) for key, path in args.root}
    if len({key.casefold() for key in roots}) != len(args.root):
        parser.error("source root names must be unique")
    if args.output.exists():
        parser.error("output directory already exists")
    recipe = load(args.recipe)
    verified = inputs(recipe, roots)
    references(recipe, verified)
    tools = verify_sdk(args.sdk_root.resolve(strict=True), args.sdk_receipt, args.toolchain_spec)
    checked(args.cafeglsl.resolve(strict=True), CAFEGLSL_SHA)
    build(args, recipe, verified, tools)


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, ValueError, TypeError, RuntimeError) as err:
        sys.exit(str(err))
