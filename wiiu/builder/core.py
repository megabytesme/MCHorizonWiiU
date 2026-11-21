#!/usr/bin/env python3
"""Local, unsigned Wii U launcher recipe validation, assembly and build."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib

VERSION = "0.3.0"
TARGET = "powerpc-nintendo-cafe"
ROOT = Path(__file__).resolve().parents[2]
CLIENT_PATHS = ["wiiu/pregame", "wiiu/host", "wiiu/gx2_hud", "wiiu/package_rpx.cmake"]
THEMES = {
    "preview": "horizon_wiiu_pregame",
    "native": "HorizonLauncher-Native",
    "night": "HorizonLauncher-Night",
}
PACKAGE_NAMES = [THEMES["preview"] + ".rpx", THEMES["preview"] + ".wuhb"]
VERSION_PATTERN = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_json(data):
    return json.loads(data, object_pairs_hook=reject_duplicates)


def fields(obj, *keys):
    if type(obj) is not dict or obj.keys() != set(keys):
        raise ValueError(f"expected exactly these fields: {', '.join(keys)}")


def pattern(value, expression, name):
    if not isinstance(value, str) or not re.fullmatch(expression, value):
        raise ValueError(f"invalid {name}")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def git(directory, *args, input_bytes=None, check=True, index=None):
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
               GIT_TERMINAL_PROMPT="0", GIT_NO_REPLACE_OBJECTS="1", GIT_OPTIONAL_LOCKS="0")
    if index is not None:
        env["GIT_INDEX_FILE"] = str(index)
    return subprocess.run(
        ["git", "-C", str(directory), "-c", "core.autocrlf=false",
         "-c", f"core.hooksPath={os.devnull}", *args],
        input=input_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, check=check,
    )


def verify_source(source, revision, pinned):
    commit = git(source, "rev-parse", "--verify", "--end-of-options", f"{revision}^{{commit}}")
    if commit.stdout.strip().decode("ascii") != pinned["commit"]:
        raise ValueError("upstream revision does not match the exact pin")
    tree = git(source, "rev-parse", f'{pinned["commit"]}^{{tree}}').stdout.strip().decode("ascii")
    inventory = git(source, "ls-tree", "-rz", "--full-tree", pinned["commit"]).stdout
    if tree != pinned["tree"] or digest(inventory) != pinned["inventory_sha256"]:
        raise ValueError("upstream tree fingerprint mismatch")


def isolated(path):
    path = path.resolve()
    parent = path
    while not parent.is_dir():
        parent = parent.parent
    if git(parent, "rev-parse", "--is-inside-work-tree", check=False).returncode == 0:
        raise ValueError("output must be outside Git checkouts")
    return path


def program(name):
    path = shutil.which(name)
    if path is None:
        raise ValueError(f"missing build tool: {name}")
    return Path(path).absolute()


def run(args, env=None):
    subprocess.run([str(arg) for arg in args], check=True, env=env)


def inspect_toolchain():
    devkit_path = os.environ.get("DEVKITPRO")
    rustc_path = os.environ.get("HORIZON_WIIU_STD_RUSTC")
    if not devkit_path or not rustc_path:
        raise ValueError("set DEVKITPRO and HORIZON_WIIU_STD_RUSTC")
    devkit = Path(devkit_path).resolve(strict=True)
    rustc = Path(rustc_path).resolve(strict=True)
    commands = {
        name: program(name) for name in (
            "powerpc-eabi-cmake", "powerpc-eabi-gcc", "powerpc-eabi-objcopy",
            "cmake", "ninja", "cargo", "cargo-wiiu", "glslcompiler", "readrpl", "wuhbtool",
        )
    }
    commands["rustc"] = rustc

    def output(args):
        return subprocess.check_output(args, text=True).strip()

    rustlib = Path(output([rustc, "--print", "target-libdir", "--target", TARGET]))
    files = {name: {"path": str(path), "sha256": file_digest(path)} for name, path in commands.items()}
    # Rustc and Cargo may be small launchers. Record the selected compiler/driver
    # libraries and target core as well as executable versions.
    sysroot = Path(output([rustc, "--print", "sysroot"]))
    compiler_files = [
        *rustlib.glob("libcore-*.rlib"), *sysroot.glob("lib/librustc_driver*"),
        *sysroot.glob("lib/libLLVM*"),
    ]
    if not any(rustlib.glob("libcore-*.rlib")):
        raise ValueError(f"no Cafe target core in {rustlib}")
    for path in sorted(compiler_files):
        files[f"rust/{path.name}"] = {"path": str(path), "sha256": file_digest(path)}
    gcc = commands["powerpc-eabi-gcc"]
    for kind, names in (("prog", ("cc1", "as", "ld")), ("file", ("libgcc.a", "libc.a", "libm.a"))):
        for name in names:
            path = Path(output([gcc, f"-print-{kind}-name={name}"])).resolve(strict=True)
            files[f"gcc/{name}"] = {"path": str(path), "sha256": file_digest(path)}
    for name in ("wut/lib/libwut.a", "wut/share/wut.specs", "wut/share/wut.ld",
                 "wut/share/wut.toolchain.cmake", "wut/share/wut.cmake"):
        path = devkit / name
        files[name] = {"path": str(path), "sha256": file_digest(path)}
    version_args = {
        "rustc": ["-vV"], "powerpc-eabi-gcc": ["--version"], "cmake": ["--version"],
        "cargo": ["--version"], "ninja": ["--version"],
    }
    headers = devkit / "wut/include"
    header_inventory = [
        (path.relative_to(headers).as_posix(), file_digest(path))
        for path in sorted(headers.rglob("*.h"))
    ]
    if not header_inventory:
        raise ValueError(f"no WUT headers in {headers}")
    return {
        "files": files,
        "wut_header_inventory_sha256": digest(json.dumps(header_inventory).encode()),
        "versions": {name: output([commands[name], *args]) for name, args in version_args.items()},
    }


def artifacts(directory, names=PACKAGE_NAMES):
    result = {}
    for name in names:
        path = directory / name
        if path.stat().st_size == 0:
            raise ValueError(f"package is empty: {name}")
        result[name] = {"sha256": file_digest(path), "bytes": path.stat().st_size}
    return result


def verify_outputs(record, directory):
    if type(record) is not dict or type(record.get("schema")) is not int or record["schema"] != 1:
        raise ValueError("unsupported build record")
    if record.get("kind") == "wiiu-native-game-build":
        names = ["HorizonWiiU-Game.rpx", "HorizonWiiU-Game.wuhb"]
    elif record.get("kind") == "wiiu-hosted-game-build":
        names = ["HorizonWiiU.rpx", "HorizonWiiU.wuhb"]
    elif record.get("kind") == "wiiu-pregame-build":
        theme = record.get("theme", "preview")
        if not isinstance(theme, str) or theme not in THEMES:
            raise ValueError("unknown build theme")
        fixture = record.get("fixture", False)
        if type(fixture) is not bool or (fixture and theme != "night"):
            raise ValueError("unknown launcher fixture")
        names = launcher_packages(theme, fixture)
    else:
        raise ValueError("unsupported build record")
    expected = record.get("outputs")
    fields(expected, *names)
    for item in expected.values():
        fields(item, "sha256", "bytes")
        pattern(item["sha256"], "[0-9a-f]{64}", "package SHA-256")
        if type(item["bytes"]) is not int or item["bytes"] <= 0:
            raise ValueError("invalid package size")
    if artifacts(directory, names) != expected:
        raise ValueError("package size or SHA-256 mismatch")


def write_record(path, value):
    data = json.dumps(value, indent=2, sort_keys=True) + "\n"
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", newline="\n",
                                     dir=path.parent, delete=False) as stream:
        staged = Path(stream.name)
        stream.write(data)
    try:
        staged.replace(path)
    finally:
        staged.unlink(missing_ok=True)


def verify_client_version(recipe):
    manifest = ROOT / "wiiu/pregame/Cargo.toml"
    version = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["version"]
    if version != recipe["launcher"]:
        raise ValueError("local launcher client version differs from the recipe")


def client_input(recipe):
    verify_client_version(recipe)
    if git(ROOT, "status", "--porcelain", "--untracked-files=all", "--", *CLIENT_PATHS).stdout:
        raise ValueError("commit the launcher and GX2 build inputs before building a recipe")
    inventory = git(ROOT, "ls-tree", "-rz", "--full-tree", "HEAD", "--", *CLIENT_PATHS).stdout
    revision = git(ROOT, "rev-parse", "HEAD").stdout.strip().decode("ascii")
    # Hash actual workspace bytes too, since Git can transparently alter line endings.
    inputs = {
        name.decode("utf-8"): file_digest(ROOT / name.decode("utf-8"))
        for entry in inventory.split(b"\0") if entry
        for name in [entry.split(b"\t", 1)[1]]
    }
    return {"revision": revision, "files": inputs}


def discard_unverified_outputs(directory, record_path, names=PACKAGE_NAMES):
    try:
        verify_outputs(parse_json(record_path.read_bytes()), directory)
    except (OSError, ValueError):
        for name in names:
            (directory / name).unlink(missing_ok=True)


def launcher_packages(theme, fixture):
    stem = THEMES[theme] + ("-Fixture" if fixture else "")
    return [stem + ".rpx", stem + ".wuhb"]


def standalone_rust_archives(rustc, recipe):
    from native import inventory_digest, toolchain_spec

    libraries = Path(subprocess.check_output(
        [rustc, "--target", TARGET, "--print", "target-libdir"], text=True,
    ).strip())
    expected = toolchain_spec(recipe)["toolchain"]["target_library_inventory_sha256"]
    if inventory_digest(libraries) != expected:
        raise ValueError("standalone Cafe Rust libraries differ from the toolchain pin")
    archives = []
    for name in ("core", "compiler_builtins"):
        matches = list(libraries.glob(f"lib{name}-*.rlib"))
        if len(matches) != 1:
            raise ValueError(f"expected one verified Cafe {name} archive")
        archives.append(matches[0])
    return archives


def build(recipe, recipe_hash, build_root, fixture=False):
    theme = "night"
    client = client_input(recipe)
    toolchain = inspect_toolchain()
    libcore, builtins = standalone_rust_archives(toolchain["files"]["rustc"]["path"], recipe)
    build_settings = {"SOURCE_DATE_EPOCH": "0", "LC_ALL": "C", "TZ": "UTC"}
    ignored_flags = {"CC", "CFLAGS", "CPPFLAGS", "LDFLAGS", "CMAKE_PREFIX_PATH", "CMAKE_TOOLCHAIN_FILE"}
    env = {name: value for name, value in os.environ.items() if name not in ignored_flags}
    env.update(build_settings)
    identity = {
        "recipe_sha256": recipe_hash,
        "environment": build_settings,
        "theme": theme,
        "fixture": fixture,
        "client_files": client["files"],
        "client_root": str(ROOT),
        "toolchain": toolchain,
    }
    key = digest(json.dumps(identity, sort_keys=True).encode())
    directory = isolated(build_root) / key
    directory.mkdir(parents=True, exist_ok=True)
    lock = directory / ".building"
    lock.mkdir()  # Fail if another process is already using this CMake cache.
    record_path = directory / "build-record.json"
    try:
        names = launcher_packages(theme, fixture)
        discard_unverified_outputs(directory, record_path, names)
        record_path.unlink(missing_ok=True)
        tools = {name: item["path"] for name, item in toolchain["files"].items()}
        configure = [
            tools["powerpc-eabi-cmake"], "-S", str(ROOT / "wiiu/pregame"), "-B", str(directory),
            "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release",
            f"-DCMAKE_PROJECT_horizon_wiiu_pregame_INCLUDE={ROOT / 'wiiu/builder/standalone_night.cmake'}",
            f"-DHORIZON_CAFE_LIBCORE={libcore}",
            f"-DHORIZON_CAFE_BUILTINS={builtins}",
            f"-DHORIZON_WIIU_PREGAME_FIXTURE={'ON' if fixture else 'OFF'}",
            f'-DCMAKE_MAKE_PROGRAM={tools["ninja"]}',
            f'-DCAFE_RUSTC={tools["rustc"]}',
            f'-DCAFEGLSL_COMPILER_EXECUTABLE={tools["glslcompiler"]}',
            f'-DCARGO_EXECUTABLE={tools["cargo"]}',
            f'-DCARGO_WIIU_EXECUTABLE={tools["cargo-wiiu"]}',
            f'-DOBJCOPY_EXECUTABLE={tools["powerpc-eabi-objcopy"]}',
            f'-DREADRPL_EXECUTABLE={tools["readrpl"]}',
            f'-DWUHBTOOL_EXECUTABLE={tools["wuhbtool"]}',
        ]
        compile_command = [tools["cmake"], "--build", str(directory), "--parallel", str(os.cpu_count() or 1)]
        run(configure, env)
        run(compile_command, env)
        if client_input(recipe) != client or inspect_toolchain() != toolchain:
            raise ValueError("build inputs changed during compilation")
        record = {
            "schema": 1, "kind": "wiiu-pregame-build", "recipe": recipe, "theme": theme, "fixture": fixture,
            "recipe_sha256": recipe_hash, "client": client, "toolchain": toolchain,
            "environment": build_settings, "commands": [configure, compile_command], "outputs": artifacts(directory, names),
            "build": "passed",
        }
        write_record(record_path, record)
    finally:
        lock.rmdir()
    return record_path


def directory_inventory(root):
    root = root.resolve(strict=True)
    entries = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or getattr(path, "is_junction", lambda: False)():
            raise ValueError("file inventory does not accept links")
        if path.is_file():
            entries[path.relative_to(root).as_posix()] = {
                "bytes": path.stat().st_size, "sha256": file_digest(path),
            }
        elif not path.is_dir():
            raise ValueError("file inventory only accepts regular files and directories")
    if not entries:
        raise ValueError("input directory is empty")
    return entries


def inspect_resources(root, output):
    # Private sidecar only. No resource bytes or file paths enter the native package.
    root = root.resolve(strict=True)
    output = isolated(output)
    if output.is_relative_to(root):
        raise ValueError("resource record must be outside the input tree")
    entries = directory_inventory(root)
    output.parent.mkdir(parents=True, exist_ok=True)
    write_record(output, {"schema": 1, "kind": "private-prepared-resources", "files": entries})
    return output
