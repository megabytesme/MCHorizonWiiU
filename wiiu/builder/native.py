"""Build the standalone Cafe game from verified assembled source."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from contextlib import contextmanager

from core import (ROOT, TARGET, artifacts, digest, directory_inventory, file_digest,
                  client_input, fields, git, inspect_toolchain, isolated, parse_json, pattern, program, run,
                  write_record, discard_unverified_outputs)
from source import apply_patch, checked_bytes, staged_tree, verify_assembly

OVERLAYS = ("winit", "ring", "png", "ureq")
INPUTS_FILE = "catalog/toolchain-cafe-v1.json"
BRIDGE_FILES = {"lock": "wiiu/game-bridge/offline/Cargo.lock",
                "tv_size": "wiiu/game-bridge/offline/winit_tv_size.c"}
GAME_PACKAGES = ["HorizonWiiU-Game.rpx", "HorizonWiiU-Game.wuhb"]
HOSTED_PACKAGES = ["HorizonWiiU.rpx", "HorizonWiiU.wuhb"]
HOSTED_SPEC = "catalog/hosted-v2.json"
HOSTED_LOCAL_FILES = {
    "wiiu/game-bridge/night_hosted.rs": "hosted.rs",
    "wiiu/game-bridge/night_store.rs": "night_store.rs",
    "wiiu/game-bridge/night_menu.rs": "night_menu.rs",
    "wiiu/pregame/src/local_wire.rs": "local_wire.rs",
}
HOSTED_LOCK = "wiiu/game-bridge/offline/night.Cargo.lock"


def toolchain_spec(recipe):
    data = checked_bytes(ROOT, INPUTS_FILE, recipe["inputs_sha256"])
    lock = parse_json(data)
    if lock.get("schema") != 1 or lock.get("version") != recipe["toolchain"]:
        raise ValueError("incompatible native input specification")
    return lock


def public_workspace_patch(recipe):
    name = f'patches/{recipe["upstream"]["commit"]}/hosted/workspace.patch'
    return decode_workspace_patch(checked_bytes(ROOT, name, toolchain_spec(recipe)["workspace_patch_sha256"]))


def decode_workspace_patch(patch):
    if b"GIT binary patch" in patch or patch.count(b"\r\n") != patch.count(b"\n"):
        raise ValueError("workspace overlay must be the archived CRLF text patch")
    return patch.replace(b"\r\n", b"\n")


def hosted_patches(recipe):
    folder = f'patches/{recipe["upstream"]["commit"]}/hosted'
    return (f"{folder}/0001-cafe-hosted-session.patch", "wiiu/game-bridge/hosted.patch",
            f"{folder}/0002-night-launch-options.patch", "wiiu/game-bridge/night-platform.patch")


def hosted_inputs(recipe, recipe_hash):
    payload = (ROOT / HOSTED_SPEC).read_bytes()
    spec = parse_json(payload)
    fields(spec, "schema", "hosted_client", "hosted_patch", "builder", "base_recipe_sha256", "input_tree",
           "output_tree", "resource_pin", "files")
    if type(spec["schema"]) is not int or spec["schema"] != 2 or spec["base_recipe_sha256"] != recipe_hash:
        raise ValueError("hosted recipe does not match the standalone pin")
    for name in ("hosted_client", "hosted_patch", "builder"):
        pattern(spec[name], r"\d+\.\d+\.\d+", name)
    if spec["builder"] != "0.5.0":
        raise ValueError("incompatible hosted builder version")
    pin = spec["resource_pin"]
    fields(pin, "minecraft", "schema", "client_sha256", "server_sha256", "exporter_sha256")
    pattern(pin["minecraft"], r"\d+\.\d+(?:\.\d+)?", "prepared resource version")
    if type(pin["schema"]) is not int or not 1 <= pin["schema"] <= 65535:
        raise ValueError("invalid prepared resource schema")
    for name in ("client_sha256", "server_sha256", "exporter_sha256"):
        pattern(pin[name], "[0-9a-f]{64}", name)
    for name in ("input_tree", "output_tree"):
        pattern(spec[name], "[0-9a-f]{40}", name)
    files = (*hosted_patches(recipe), *HOSTED_LOCAL_FILES, HOSTED_LOCK)
    fields(spec["files"], *files)
    for name in files:
        pattern(spec["files"][name], "[0-9a-f]{64}", "hosted patch hash")
    return spec, digest(payload), {name: checked_bytes(ROOT, name, spec["files"][name]) for name in files}



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
    lock = toolchain_spec(recipe)
    for name in OVERLAYS:
        if inventory_digest(inputs / name) != lock["overlay_sha256"][name]:
            raise ValueError(f"external dependency overlay differs: {name}")
    patch = public_workspace_patch(recipe)
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


def stage_source(directory, workspace, release_tree, patch, hosted=None, recipe=None):
    source = directory / "source"
    stamp = directory / "stage.json"
    # A changed cache is rejected. Allocate a new build root to reconstruct it.
    if source.exists():
        if (source / ".git/wiiu-stage.json").is_file():
            stamp = source / ".git/wiiu-stage.json"
        if not stamp.is_file():
            raise ValueError("incomplete staged game source")
        tree = parse_json(stamp.read_bytes())["tree"]
        if ((hosted is not None and tree != hosted[0]["output_tree"]) or
                staged_tree(source) != tree or
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
        if hosted is not None:
            spec, _, files = hosted
            if staged_tree(staged) != spec["input_tree"]:
                raise ValueError("hosted patch preimage does not match")
            for name in hosted_patches(recipe):
                apply_patch(staged, files[name])
            for name, destination in HOSTED_LOCAL_FILES.items():
                path = f"wiiu/loading_app/src/{destination}"
                (staged / path).write_bytes(files[name])
                git(staged, "add", "--", path)
        tree = staged_tree(staged)
        if hosted is not None and tree != hosted[0]["output_tree"]:
            raise ValueError("hosted patch result does not match")
        write_record(staged / ".git/wiiu-stage.json", {"tree": tree})
        staged.rename(source)
    return source, tree


def write_cargo_inputs(directory, source, inputs, lock, bridge):
    directory = directory.resolve()
    source = source.resolve()
    dependency_root = directory / "external"
    dependency_root.mkdir(exist_ok=True)
    for name in OVERLAYS:
        target = dependency_root / name
        if not target.exists():
            with tempfile.TemporaryDirectory(prefix=f"{name}-", dir=dependency_root) as scratch:
                candidate = Path(scratch) / name
                shutil.copytree(inputs / name, candidate)
                if inventory_digest(candidate) != lock["overlay_sha256"][name]:
                    raise ValueError(f"staged dependency overlay differs: {name}")
                candidate.rename(target)
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


@contextmanager
def build_lock(directory):
    import fcntl

    with (directory / ".native-build.lock").open("a") as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("another job is using this native build output") from error
        yield


def build_game(recipe, recipe_hash, assembly, inputs, build_root, resources_hash=None, integrated=False):
    workspace = verify_assembly(assembly, recipe, recipe_hash)
    lock, patch, bridge, tools, sysroot = read_native_inputs(recipe, inputs)
    hosted = hosted_inputs(recipe, recipe_hash) if integrated else None
    launcher = client_input(recipe) if integrated else None
    packages = HOSTED_PACKAGES if integrated else GAME_PACKAGES
    settings = {"SOURCE_DATE_EPOCH": "0", "LC_ALL": "C", "TZ": "UTC",
                "CARGO_PROFILE_RELEASE_OPT_LEVEL": "s"}
    identity = {
        "recipe": recipe_hash, "toolchain": tools, "environment": settings, "workspace_layout": 2,
        "resources": resources_hash, "assembly": str(assembly.resolve()),
    }
    if hosted:
        identity.update(hosted_recipe=hosted[1], launcher_files=launcher["files"])
    key = digest(json.dumps(identity, sort_keys=True).encode())
    output_root = isolated(build_root)
    # The combined target keeps the caller's isolated workspace stable so
    # a shell-only relink can reuse the verified Rust game archive. The
    # per-build receipt still pins both source inventories.
    directory = output_root if hosted else output_root / key
    directory.mkdir(parents=True, exist_ok=True)
    record = directory / "build-record.json"
    with build_lock(directory):
        discard_unverified_outputs(directory, record, packages)
        record.unlink(missing_ok=True)
        source, staged = stage_source(directory, workspace, recipe["native"]["tree"], patch, hosted, recipe)
        if hosted:
            bridge["lock"] = hosted[2][HOSTED_LOCK]
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
        if hosted:
            spec, hosted_hash, _ = hosted
            pin = spec["resource_pin"]
            embedded = {
                "HORIZON_NIGHT_UPSTREAM": recipe["upstream"]["commit"],
                "HORIZON_NIGHT_CLIENT": spec["hosted_client"],
                "HORIZON_NIGHT_PATCH": spec["hosted_patch"],
                "HORIZON_NIGHT_BUILDER": spec["builder"],
                "HORIZON_NIGHT_MINECRAFT": pin["minecraft"],
                "HORIZON_NIGHT_PREP_SCHEMA": str(pin["schema"]),
                "HORIZON_NIGHT_CLIENT_JAR": pin["client_sha256"],
                "HORIZON_NIGHT_SERVER_JAR": pin["server_sha256"],
                "HORIZON_NIGHT_EXPORTER": pin["exporter_sha256"],
                "HORIZON_NIGHT_BUILD_ID": digest(json.dumps({"hosted": hosted_hash,
                    "standalone": recipe_hash, "launcher": launcher["files"]}, sort_keys=True).encode()),
            }
            env.update(embedded)
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
        if hosted:
            configure.extend([
                f"-DHORIZON_WIIU_HOSTED_SHELL={ROOT / 'wiiu/pregame'}",
                f"-DCAFE_RUSTC={commands['rustc']}",
            ])
        link = [commands["cmake"], "--build", str(native), "--parallel", jobs]
        run(configure, env)
        run(link, env)
        imports = subprocess.check_output(
            [commands["readrpl"], "-i", str(native / "horizon_wiiu_loading_app.rpx")], text=True)
        if any(name not in imports for name in ("AXInit", "NSSGetRandom", "FSFlushFile")):
            raise ValueError("native game package is missing the expected Cafe imports")
        if hosted:
            symbols = subprocess.check_output([commands["powerpc-eabi-nm"], "-g", "--defined-only",
                str(native / "horizon_wiiu_loading_app.elf")], text=True)
            for name in ("main", "shell_process", "horizon_pregame_run", "horizon_hosted_game_run"):
                if sum(line.endswith(f" T {name}") for line in symbols.splitlines()) != 1:
                    raise ValueError(f"combined executable must define {name} exactly once")
        shutil.copyfile(native / "horizon_wiiu_loading_app.rpx", directory / packages[0])
        title = "Horizon Wii U" if hosted else "Horizon Wii U Game"
        pack = [commands["wuhbtool"], str(directory / packages[0]),
                str(directory / packages[1]), f"--name={title}", "--short-name=Horizon"]
        run(pack, env)
        stage_source(directory, workspace, recipe["native"]["tree"], patch, hosted, recipe)
        result = {
            "schema": 1, "kind": "wiiu-hosted-game-build" if hosted else "wiiu-native-game-build",
            "recipe_sha256": recipe_hash,
            "recipe": recipe, "assembled_tree": recipe["native"]["tree"], "build_tree": staged,
            "workspace_patch_sha256": lock["workspace_patch_sha256"], "toolchain": tools,
            "toolchain_spec_sha256": recipe["inputs_sha256"], "resources_inventory_sha256": resources_hash,
            "environment": settings, "commands": [cargo, configure, link, pack],
            "outputs": artifacts(directory, packages),
            "assembly": "passed", "build": "passed", "cemu": "not-run", "hardware": "not-run",
        }
        if hosted:
            if client_input(recipe) != launcher or hosted_inputs(recipe, recipe_hash) != hosted:
                raise ValueError("hosted build inputs changed during compilation")
            result.update(hosted_recipe=hosted[0], hosted_recipe_sha256=hosted[1],
                          launcher=launcher, build_input_sha256=key, embedded_build=embedded)
        write_record(record, result)
    return record
