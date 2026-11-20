#!/usr/bin/env python3
"""MCHorizonWiiU release builder. Use a Linux or WSL x86-64 build environment."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import core
import dependencies
import native
import releases
import source

VERSION = "0.5.0"
MIN_BUILD_SPACE = 8 * 1024**3
MIN_RESOURCE_SPACE = 3 * 1024**3


def require_space(path, minimum):
    """Check the volume that will actually receive the output."""
    parent = path.resolve()
    while not parent.exists():
        parent = parent.parent
    free = shutil.disk_usage(parent).free
    if free < minimum:
        raise ValueError(f"{parent} has {free / 1024**3:.1f} GiB free. This operation needs at least {minimum / 1024**3:g} GiB.")


@contextmanager
def cache_lock(root):
    import fcntl

    root.mkdir(parents=True, exist_ok=True)
    with (root / ".builder.lock").open("a") as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("another builder is using this cache") from error
        yield


def ensure_assembly(cache, recipe, recipe_hash, offline=False, upstream=None):
    pin = recipe["upstream"]["commit"]
    destination = cache / f"assembly-{recipe_hash}"
    if destination.exists():
        source.verify_assembly(destination, recipe, recipe_hash)
        return destination
    if upstream is None:
        upstream = cache / pin
        if not upstream.exists():
            if offline:
                raise ValueError("the pinned source is absent from the offline cache. Run Prepare dependencies online once.")
            with tempfile.TemporaryDirectory(prefix="horizon-source-", dir=cache) as folder:
                candidate = Path(folder) / "upstream"
                candidate.mkdir()
                core.git(candidate, "init", "--quiet", "--template=")
                core.git(candidate, "fetch", "--quiet", "--depth=1", "--no-tags",
                         "--no-recurse-submodules", releases.UPSTREAM, pin)
                core.verify_source(candidate, pin, recipe["upstream"])
                candidate.rename(upstream)
    source.assemble(upstream, pin, recipe, recipe_hash, destination)
    return destination


def check_source(assembly, recipe, recipe_hash):
    workspace = source.verify_assembly(assembly, recipe, recipe_hash)
    hosted = native.hosted_inputs(recipe, recipe_hash)
    patch = native.public_workspace_patch(recipe)
    with tempfile.TemporaryDirectory(prefix="wiiu-hosted-check-", dir=assembly.parent) as scratch:
        return native.stage_source(Path(scratch), workspace, recipe["native"]["tree"], patch, hosted, recipe)[1]


def prepare_dependencies(assembly, recipe, recipe_hash, inputs, offline):
    workspace = source.verify_assembly(assembly, recipe, recipe_hash)
    lock, patch, bridge, tools, _ = native.read_native_inputs(recipe, inputs)
    hosted = native.hosted_inputs(recipe, recipe_hash)
    bridge["lock"] = hosted[2][native.HOSTED_LOCK]
    with tempfile.TemporaryDirectory(prefix="wiiu-fetch-", dir=assembly.parent) as scratch:
        directory = Path(scratch)
        staged, _ = native.stage_source(directory, workspace, recipe["native"]["tree"], patch, hosted, recipe)
        manifest = native.write_cargo_inputs(directory, staged, inputs, lock, bridge)
        cargo = tools["files"]["cargo"]["path"]
        command = [cargo, "+nightly", "fetch", "--locked", "--target", core.TARGET,
                   "--manifest-path", str(manifest)]
        if offline:
            command.append("--offline")
        overrides = {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}
        env = {key: value for key, value in os.environ.items() if key not in overrides}
        env["RUSTC"] = tools["files"]["rustc"]["path"]
        core.run(command, env)
    print("Cafe toolchain, dependency overlays and locked Cargo cache verified.")


def check_resource_pin(prepared, pin):
    manifest = core.parse_json((prepared / "horizon-resources.json").read_bytes())
    if any(manifest.get(key) != value for key, value in pin.items()):
        raise ValueError("prepared resources are incompatible with the selected Minecraft and exporter pins")


def prepare_resources(assembly, recipe, recipe_hash, minecraft, client, server, output):
    pin = native.hosted_inputs(recipe, recipe_hash)[0]["resource_pin"]
    minecraft, client, server = (path.resolve(strict=True) for path in (minecraft, client, server))
    for name, path in (("client", client), ("server", server)):
        if core.file_digest(path) != pin[f"{name}_sha256"]:
            raise ValueError(f"the {name} JAR does not match Minecraft {pin['minecraft']}")
    output = core.isolated(output)
    if output.exists():
        raise ValueError("choose a new resource output directory or reuse a verified existing inventory")
    require_space(output, MIN_RESOURCE_SPACE)
    output.parent.mkdir(parents=True, exist_ok=True)
    workspace = source.verify_assembly(assembly, recipe, recipe_hash)
    with tempfile.TemporaryDirectory(prefix="wiiu-resources-", dir=output.parent) as scratch:
        root = Path(scratch)
        prepared = root / "prepared"
        env = {key: value for key, value in os.environ.items() if not key.startswith("HORIZON_")}
        for name in ("HORIZON_HOME", "HORIZON_LAUNCHER_HOME", "XDG_DATA_HOME", "XDG_CONFIG_HOME",
                     "HORIZON_LAUNCHER_TRASH", "HORIZON_LAUNCHER_CLIPBOARD", "HORIZON_LAUNCHER_DESKTOP"):
            env[name] = str(root / name)
            Path(env[name]).mkdir()
        env.update(HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1",
                   CARGO_TARGET_DIR=str(assembly.parent / "prepare-target"))
        core.run(["cargo", "run", "--locked", "--manifest-path", str(workspace / "Cargo.toml"),
                  "-p", "horizon-prepare", "--", "--prepare", "--root", str(workspace),
                  "--minecraft-dir", str(minecraft), "--client-jar", str(client), "--server-jar", str(server),
                  "--output", str(prepared)], env)
        check_resource_pin(prepared, pin)
        core.inspect_resources(prepared, root / "resources.json")
        (root / "resources.json").rename(root / "inventory.json")
        prepared.rename(output)
        shutil.move(root / "inventory.json", output.parent / f"{output.name}.inventory.json")
    print(f"Private resources: {output}")
    print(f"Private inventory: {output.parent / f'{output.name}.inventory.json'}")


def add_selection(parser):
    parser.add_argument("--catalog", type=Path, default=releases.CATALOG)
    parser.add_argument("--catalog-sha256", help="expected catalog bytes from the desktop selection")
    parser.add_argument("--allow-unsigned", action="store_true", help="use the checked-in development catalog")
    parser.add_argument("--signature", type=Path)
    parser.add_argument("--public-key", type=Path, help="publisher public key obtained from a trusted release")
    parser.add_argument("--client", default="0.6.0")
    parser.add_argument("--patch", default="0.3.1")
    parser.add_argument("--toolchain", default="1.0.0")
    parser.add_argument("--channel", choices=releases.CHANNELS, default="development")


def selection(args):
    if args.signature and args.public_key:
        state = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "mchorizonwiiu"
        key_id = core.file_digest(args.public_key)
        catalog, payload = releases.verify_signature(args.catalog, args.signature, args.public_key, state / f"{key_id}.json")
    elif args.allow_unsigned and args.catalog.resolve() == releases.CATALOG and not (args.signature or args.public_key):
        catalog, payload = releases.read_catalog(args.catalog)
        if catalog["signing"] != "unsigned-development":
            raise ValueError("a signed catalog requires its signature and trusted publisher key")
    else:
        raise ValueError("supply a catalog signature and trusted publisher key, or select the local unsigned development catalog")
    if args.catalog_sha256 is not None and args.catalog_sha256 != core.digest(payload):
        raise ValueError("the desktop and build environment have different catalog bytes")
    rule = releases.resolve(catalog, builder=VERSION, client=args.client, patch=args.patch,
                            toolchain=args.toolchain, channel=args.channel)
    for kind in releases.KINDS:
        record = catalog["releases"][kind][rule[kind]]
        for name, artifact in record["artifacts"].items():
            if kind == "builder" and name == "sources" and args.signature:
                releases.verify_source_inventory(artifact)
            elif "commit" in artifact:
                releases.verify_artifact(artifact)
    recipe, fingerprint = source.read_recipe(source.MANIFEST)
    hosted, _, _ = native.hosted_inputs(recipe, fingerprint)
    if (recipe["upstream"]["commit"], recipe["toolchain"], hosted["hosted_client"],
        hosted["hosted_patch"], hosted["builder"]) != (
        rule["upstream"], rule["toolchain"], rule["client"], rule["patch"], rule["builder"]
    ):
        raise ValueError("selected versions are not implemented by this checkout")
    if "sources" in catalog["releases"]["client"][rule["client"]]["artifacts"]:
        record = catalog["releases"]["client"][rule["client"]]["artifacts"]["sources"]
        expected = core.parse_json(releases.verify_artifact(record))
        if core.client_input(recipe)["files"] != expected:
            raise ValueError("client source differs from the selected client release")
    return recipe, fingerprint


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", action="version", version=VERSION)
    add_selection(parser)
    parser.add_argument("--cache", type=Path, default=Path.home() / ".cache/mchorizonwiiu")
    parser.add_argument("--upstream", type=Path, help="use a local Git repository for the exact upstream")
    parser.add_argument("--offline", action="store_true", help="use previously populated caches")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check-source")
    commands.add_parser("prepare-overlays")
    inventory = commands.add_parser("inventory-toolchain")
    inventory.add_argument("--package-cache", type=Path)
    for name in ("prepare", "build"):
        operation = commands.add_parser(name)
        operation.add_argument("--inputs", default=Path("auto"), type=Path,
                               help="override the verified dependency overlay directory")
        if name == "build":
            operation.add_argument("--output", required=True, type=Path)
            operation.add_argument("--fixed-path", action="store_true", help="use the stable Linux mount for byte-for-byte comparisons")
            operation.add_argument("--resources", type=Path)
            operation.add_argument("--resource-manifest", type=Path)
    resources = commands.add_parser("prepare-resources")
    for option in ("minecraft", "client-jar", "server-jar", "output"):
        resources.add_argument(f"--{option}", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if sys.platform != "linux":
            raise ValueError("run this CLI inside Linux or WSL. On Windows, start desktop.py.")
        recipe, recipe_hash = selection(args)
        if args.command == "inventory-toolchain":
            if args.package_cache:
                dependencies.verify_devkit_archives(args.package_cache)
            _, _, observed = native.native_toolchain()
            pinned = core.parse_json((core.ROOT / native.INPUTS_FILE).read_bytes())["toolchain"]
            print(json.dumps({"matches": observed == pinned, **observed}, indent=2))
            return int(observed != pinned)
        cache = core.isolated(args.cache)
        with cache_lock(cache):
            if args.command == "prepare-overlays":
                print(dependencies.provision(cache, recipe, offline=args.offline))
                return 0
            if args.command in ("prepare", "build", "prepare-resources"):
                require_space(cache, MIN_BUILD_SPACE)
            assembly = ensure_assembly(cache, recipe, recipe_hash, args.offline, args.upstream)
            if args.command in ("prepare", "build") and args.inputs == Path("auto"):
                args.inputs = dependencies.provision(cache, recipe, offline=args.offline or args.command == "build")
            if args.command == "check-source":
                print(f"Verified hosted source tree {check_source(assembly, recipe, recipe_hash)}")
            elif args.command == "prepare":
                prepare_dependencies(assembly, recipe, recipe_hash, args.inputs, args.offline)
            elif args.command == "prepare-resources":
                if args.offline:
                    raise ValueError("resource preparation may require asset and host dependency downloads. Reuse an existing inventory offline.")
                prepare_resources(assembly, recipe, recipe_hash, args.minecraft, args.client_jar, args.server_jar, args.output)
            elif args.command == "build":
                require_space(args.output, MIN_BUILD_SPACE)
                if bool(args.resources) != bool(args.resource_manifest):
                    raise ValueError("provide both the prepared directory and its inventory")
                resource_flags = []
                if args.resources:
                    pin = native.hosted_inputs(recipe, recipe_hash)[0]["resource_pin"]
                    check_resource_pin(args.resources, pin)
                    resource_flags = ["--resources", str(args.resources.resolve()),
                                      "--resource-manifest", str(args.resource_manifest.resolve())]
                inputs = ["--assembly", str(assembly), "--inputs", str(args.inputs.resolve()), *resource_flags]
                if args.fixed_path:
                    command = ["bash", str(core.ROOT / "tools/build_cafe_game_isolated.sh"), "--hosted", str(args.output)]
                else:
                    command = [sys.executable, "-B", str(core.ROOT / "wiiu/builder/build.py"),
                               "build-hosted", "--output", str(args.output)]
                core.run(command + inputs)
    except KeyboardInterrupt:
        print("Build cancelled. A verified cache can be reused.", file=sys.stderr)
        return 130
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr, file=sys.stderr)
        print(f"Builder step failed ({error.returncode}): {error.cmd[0]}."
              " Check the detailed output above. For missing Cargo packages, run Prepare dependencies.",
              file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Builder failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
