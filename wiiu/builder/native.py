"""Build the standalone Cafe game from verified assembled source."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from core import (ROOT, TARGET, artifacts, digest, directory_inventory, file_digest,
                  git, inspect_toolchain, isolated, parse_json, program, run,
                  write_record, discard_unverified_outputs)
from source import apply_patch, checked_bytes, staged_tree, verify_assembly

OVERLAYS = ("winit", "ring", "png", "ureq")
INPUTS_FILE = "catalog/toolchain-cafe-v1.json"
BRIDGE_FILES = {"lock": "wiiu/game-bridge/offline/Cargo.lock",
                "tv_size": "wiiu/game-bridge/offline/winit_tv_size.c"}
GAME_PACKAGES = ["HorizonWiiU-Game.rpx", "HorizonWiiU-Game.wuhb"]


def inventory_digest(directory):
    return digest(json.dumps(directory_inventory(directory), sort_keys=True).encode())


def native_toolchain():
    tools = inspect_toolchain()
    rustc = tools["files"]["rustc"]["path"]
    cargo = tools["files"]["cargo"]["path"]
    target_lib = Path(subprocess.check_output(
        [rustc, "--print", "target-libdir", "--target", TARGET], text=True).strip())
    sysroot = subprocess.check_output([rustc, "--print", "sysroot"], text=True).strip()
    for name in ("powerpc-eabi-ar", "powerpc-eabi-nm", "powerpc-eabi-g++"):
        path = program(name)
        tools["files"][name] = {"path": str(path), "sha256": file_digest(path)}
    nightly = Path(subprocess.check_output(
        [str(program("rustup")), "which", "--toolchain", "nightly", "cargo"],
        text=True).strip())
    result = {
        "files": {name: entry["sha256"] for name, entry in tools["files"].items()},
        "versions": tools["versions"],
        "wut_header_inventory_sha256": tools["wut_header_inventory_sha256"],
        "target_library_inventory_sha256": inventory_digest(target_lib),
        "nightly_cargo_sha256": file_digest(nightly),
        "nightly_cargo_version": subprocess.check_output([cargo, "+nightly", "-vV"], text=True).strip(),
    }
    return tools, sysroot, result


def read_native_inputs(recipe, inputs):
    data = checked_bytes(ROOT, INPUTS_FILE, recipe["inputs_sha256"])
    lock = parse_json(data)
    if lock.get("schema") != 1 or lock.get("version") != recipe["toolchain"]:
        raise ValueError("incompatible native input specification")
    for name in OVERLAYS:
        if inventory_digest(inputs / name) != lock["overlay_sha256"][name]:
            raise ValueError(f"external dependency overlay differs: {name}")
    patch = checked_bytes(inputs, "workspace.patch", lock["workspace_patch_sha256"])
    # This archived PowerShell text patch has uniform CRLF transport endings.
    # Its raw bytes are pinned above. Git's upstream patch series stays binary-safe.
    if b"GIT binary patch" in patch or patch.count(b"\r\n") != patch.count(b"\n"):
        raise ValueError("workspace overlay must be the archived CRLF text patch")
    patch = patch.replace(b"\r\n", b"\n")
    bridge = {name: checked_bytes(ROOT, path, lock["bridge_inputs"][name])
              for name, path in BRIDGE_FILES.items()}
    tools, sysroot, observed = native_toolchain()
    if observed != lock["toolchain"]:
        changed = sorted(key for key, value in observed.items() if value != lock["toolchain"].get(key))
        raise ValueError(f"installed Cafe toolchain differs from the pin: {', '.join(changed)}")
    return lock, patch, bridge, tools, sysroot


def verify_resources(root, manifest):
    payload = manifest.read_bytes()
    recorded = parse_json(payload)
    if recorded.get("schema") != 1 or recorded.get("kind") != "private-prepared-resources":
        raise ValueError("expected a private prepared-resource inventory")
    if directory_inventory(root) != recorded.get("files"):
        raise ValueError("prepared resources do not match their private inventory")
    return digest(payload)


def stage_source(directory, workspace, release_tree, patch):
    source = directory / "source"
    stamp = directory / "stage.json"
    # A changed cache is rejected. Allocate a new build root to reconstruct it.
    if source.exists():
        if not stamp.is_file():
            raise ValueError("incomplete staged game source")
        tree = parse_json(stamp.read_bytes())["tree"]
        if (staged_tree(source) != tree or
                git(source, "diff", "--quiet", "--ignore-submodules=all",
                    check=False).returncode or
                git(source, "ls-files", "--others", "--exclude-standard", "-z").stdout):
            raise ValueError("cached game source was modified")
        return source, tree
    with tempfile.TemporaryDirectory(prefix="native-source-", dir=directory) as folder:
        staged = Path(folder) / "source"
        # The exact staged tree is unreachable from the shallow upstream HEAD,
        # so link its object store directly into a fresh repository and index.
        staged.mkdir()
        git(staged, "init", "--quiet", "--template=")
        alternates = staged / ".git/objects/info/alternates"
        alternates.write_text(str(workspace.resolve() / ".git/objects") + "\n", newline="\n")
        upstream = git(workspace, "rev-parse", "HEAD").stdout.decode().strip()
        git(staged, "update-ref", "HEAD", upstream)
        git(staged, "read-tree", release_tree)
        git(staged, "checkout-index", "--all", "--index")
        apply_patch(staged, patch)
        tree = staged_tree(staged)
        staged.rename(source)
    write_record(stamp, {"tree": tree})
    return source, tree


def write_cargo_inputs(directory, source, inputs, lock, bridge):
    dependency_root = directory / "external"
    dependency_root.mkdir(exist_ok=True)
    for name in OVERLAYS:
        target = dependency_root / name
        if not target.exists():
            shutil.copytree(inputs / name, target)
        if inventory_digest(target) != lock["overlay_sha256"][name]:
            raise ValueError(f"staged dependency overlay differs: {name}")
    loading_app = source / "wiiu/loading_app"
    base = (loading_app / "Cargo.toml").read_text()
    # Cargo's local package identities include their paths relative to the
    # top-level workspace. Keep all dependencies below the same root.
    base = base.replace("[lib]\n", "[lib]\npath = " + json.dumps(
        (loading_app / "src/lib.rs").relative_to(directory).as_posix()) + "\n")
    base = re.sub(r'path = "([.][^"]+)"', lambda match: "path = " + json.dumps(
        (loading_app / match[1]).resolve().relative_to(directory).as_posix()), base)
    # Exact exclusions preserve the nested Horizon workspace's inherited
    # package metadata and avoid adding our patched dependencies as members.
    excluded = sorted({Path("source") / Path(os.fsdecode(name)).parent
                       for name in git(source, "ls-files", "-z", "*Cargo.toml").stdout.split(b"\0")
                       if name} | {Path("external") / name for name in OVERLAYS})
    base = base.replace("[workspace]\n", "[workspace]\nexclude = " + json.dumps(
        [path.as_posix() for path in excluded]) + "\n")
    add = {
        "wgpu-hal": source / "third_party/wgpu-hal", "naga": source / "third_party/naga",
        "libmimalloc-sys": source / "third_party/libmimalloc-sys",
        **{name: dependency_root / name for name in OVERLAYS},
    }
    manifest = directory / "Cargo.toml"
    manifest.write_text(base.rstrip() + "\n" + "".join(
        f'{name} = {{ path = {json.dumps(path.relative_to(directory).as_posix())} }}\n'
        for name, path in add.items()), newline="\n")
    (directory / "Cargo.lock").write_bytes(bridge["lock"])
    (directory / "winit_tv_size.c").write_bytes(bridge["tv_size"])
    return manifest


def build_game(recipe, recipe_hash, assembly, inputs, build_root, resources_hash=None):
    workspace = verify_assembly(assembly, recipe, recipe_hash)
    lock, patch, bridge, tools, sysroot = read_native_inputs(recipe, inputs)
    settings = {"SOURCE_DATE_EPOCH": "0", "LC_ALL": "C", "TZ": "UTC",
                "CARGO_PROFILE_RELEASE_OPT_LEVEL": "s"}
    identity = {
        "recipe": recipe_hash, "toolchain": tools, "environment": settings, "workspace_layout": 2,
        "resources": resources_hash, "assembly": str(assembly.resolve()),
    }
    key = digest(json.dumps(identity, sort_keys=True).encode())
    directory = isolated(build_root) / key
    directory.mkdir(parents=True, exist_ok=True)
    lockfile = directory / ".building"
    lockfile.mkdir()
    record = directory / "build-record.json"
    try:
        discard_unverified_outputs(directory, record, GAME_PACKAGES)
        record.unlink(missing_ok=True)
        source, staged = stage_source(directory, workspace, recipe["native"]["tree"], patch)
        manifest = write_cargo_inputs(directory, source, inputs, lock, bridge)
        commands = {name: entry["path"] for name, entry in tools["files"].items()}
        jobs = str(os.cpu_count() or 1)
        target = directory / "cargo-target"
        # Inherit tool and registry locations. Keep ambient compiler overrides
        # and diagnostic build switches outside the verified recipe.
        variables = {"PATH", "HOME", "USER", "LOGNAME", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME",
                     "DEVKITPRO", "DEVKITPPC", "SYSTEMROOT"}
        env = {k: v for k, v in os.environ.items() if k in variables}
        env.update(settings)
        env.update({
            "RUSTC": commands["rustc"], "CARGO_TARGET_DIR": str(target),
            f"CC_{TARGET}": commands["powerpc-eabi-gcc"],
            "CARGO_ENCODED_RUSTFLAGS": "\x1f".join([
                "--sysroot", sysroot, "--cfg", 'getrandom_backend="custom"',
                "--check-cfg=cfg(horizon_wiiu_cemu_global_lock)",
                f"--remap-path-prefix={directory}=/horizon-build",
            ]),
        })
        cargo = [commands["cargo"], "+nightly", "build", "--offline", "--locked", "--release",
                 "--lib", "--manifest-path", str(manifest), "--target", TARGET, "--jobs", jobs]
        run(cargo, env)
        archive = target / TARGET / "release/libhorizon_wiiu_loading_app.a"
        native = directory / "native"
        configure = [
            commands["powerpc-eabi-cmake"], "-S", str(source / "wiiu/loading_app"), "-B", str(native),
            "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release",
            f"-DCMAKE_MAKE_PROGRAM={commands['ninja']}",
            f"-DHORIZON_WIIU_PREBUILT_STATICLIB={archive}",
            f"-DHORIZON_WIIU_WINIT_TV_SIZE_SOURCE={manifest.parent / 'winit_tv_size.c'}",
            f"-DCAFEGLSL_COMPILER_EXECUTABLE={commands['glslcompiler']}",
            "-DHORIZON_WIIU_DISABLE_AX_AUDIO=OFF", "-DHORIZON_WIIU_CEMU_GLOBAL_SESSION_LOCK=OFF",
        ]
        link = [commands["cmake"], "--build", str(native), "--parallel", jobs]
        run(configure, env)
        run(link, env)
        imports = subprocess.check_output(
            [commands["readrpl"], "-i", str(native / "horizon_wiiu_loading_app.rpx")], text=True)
        if any(name not in imports for name in ("AXInit", "NSSGetRandom", "FSFlushFile")):
            raise ValueError("native game package is missing the expected Cafe imports")
        shutil.copyfile(native / "horizon_wiiu_loading_app.rpx", directory / GAME_PACKAGES[0])
        pack = [commands["wuhbtool"], str(directory / GAME_PACKAGES[0]),
                str(directory / GAME_PACKAGES[1]), "--name=Horizon Wii U Game", "--short-name=Horizon"]
        run(pack, env)
        stage_source(directory, workspace, recipe["native"]["tree"], patch)
        write_record(record, {
            "schema": 1, "kind": "wiiu-native-game-build", "recipe_sha256": recipe_hash,
            "recipe": recipe, "assembled_tree": recipe["native"]["tree"], "build_tree": staged,
            "workspace_patch_sha256": lock["workspace_patch_sha256"], "toolchain": tools,
            "toolchain_spec_sha256": recipe["inputs_sha256"], "resources_inventory_sha256": resources_hash,
            "environment": settings, "commands": [cargo, configure, link, pack],
            "outputs": artifacts(directory, GAME_PACKAGES),
            "assembly": "passed", "build": "passed", "cemu": "not-run", "hardware": "not-run",
        })
    finally:
        lockfile.rmdir()
    return record
