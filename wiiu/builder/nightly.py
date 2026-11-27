"""Prepare the pinned hosted Nightly candidate outside the active release recipe."""
import json
import tomllib

import native
import rpx
import source
from core import (ROOT, TARGET, VERSION_PATTERN, artifacts, client_input, discard_unverified_outputs,
                  digest, fields, file_digest, isolated, parse_json, pattern, verify_source,
                  write_record)

MANIFEST = ROOT / "catalog/hosted-nightly-2026-11-20.json"
OVERLAYS = ("ring", "png", "ureq")


def validate(spec):
    if not isinstance(spec, dict) or type(spec.get("schema")) is not int or spec["schema"] not in (1, 2):
        raise ValueError("unsupported hosted Nightly source recipe")
    toolchain_field = ("toolchain",) if spec["schema"] == 2 else ()
    feature_field = ("cargo_features",) if "cargo_features" in spec else ()
    if feature_field:
        features = spec["cargo_features"]
        allowed = {"stock-hud-seed", "gx2-screen-overlays", "gx2-clear-fog"}
        if (type(features) is not list or not features
                or any(type(name) is not str or name not in allowed for name in features)
                or len(features) != len(set(features))):
            raise ValueError("unsupported hosted Nightly Cargo features")
    fields(spec, "schema", "kind", "target", "upstream", "base_recipe_sha256",
           "cargo_lock_sha256", "output_tree", "series", "candidate", "resource_pin",
           *toolchain_field, *feature_field)
    if spec["kind"] != "wiiu-hosted-nightly-source" or spec["target"] != TARGET:
        raise ValueError("unsupported hosted Nightly source recipe")
    if spec["schema"] == 2:
        fields(spec["toolchain"], "version", "sha256")
        pattern(spec["toolchain"]["version"], VERSION_PATTERN, "Nightly toolchain version")
        pattern(spec["toolchain"]["sha256"], source.SHA256, "Nightly toolchain SHA-256")
    fields(spec["candidate"], "builder", "client", "patch")
    for version in spec["candidate"].values():
        pattern(version, r"\d+\.\d+\.\d+-rc\.\d+", "Nightly candidate version")
    pin = spec["resource_pin"]
    fields(pin, "minecraft", "schema", "client_sha256", "server_sha256", "exporter_sha256")
    pattern(pin["minecraft"], r"\d+\.\d+(?:\.\d+)?", "prepared resource version")
    if type(pin["schema"]) is not int or not 1 <= pin["schema"] <= 65535:
        raise ValueError("invalid prepared resource schema")
    for name in ("client_sha256", "server_sha256", "exporter_sha256"):
        pattern(pin[name], source.SHA256, "prepared resource SHA-256")
    pinned = spec["upstream"]
    fields(pinned, "commit", "tree", "inventory_sha256")
    pattern(pinned["commit"], source.SHA1, "Nightly commit")
    pattern(pinned["tree"], source.SHA1, "Nightly tree")
    pattern(pinned["inventory_sha256"], source.SHA256, "Nightly inventory SHA-256")
    pattern(spec["base_recipe_sha256"], source.SHA256, "base recipe SHA-256")
    pattern(spec["cargo_lock_sha256"], source.SHA256, "Cargo lock SHA-256")
    pattern(spec["output_tree"], source.SHA1, "Nightly output tree")
    if not isinstance(spec["series"], list) or not spec["series"]:
        raise ValueError("missing hosted Nightly patches")
    previous = pinned["tree"]
    names = set()
    for patch in spec["series"]:
        fields(patch, "file", "sha256", "input_tree", "output_tree")
        name = source.relative(patch["file"])
        if name.parts[:-1] != ("patches", pinned["commit"]) or name.suffix != ".patch":
            raise ValueError("Nightly patch must belong to its pinned source")
        if name in names:
            raise ValueError("duplicate hosted Nightly patch")
        names.add(name)
        pattern(patch["sha256"], source.SHA256, "Nightly patch SHA-256")
        pattern(patch["output_tree"], source.SHA1, "Nightly patch output tree")
        if patch["input_tree"] != previous:
            raise ValueError("noncontiguous hosted Nightly patch trees")
        previous = patch["output_tree"]
    if previous != spec["output_tree"]:
        raise ValueError("hosted Nightly final tree mismatch")


def read_recipe(base_fingerprint):
    payload = MANIFEST.read_bytes()
    spec = parse_json(payload)
    validate(spec)
    if spec["base_recipe_sha256"] != base_fingerprint:
        raise ValueError("hosted Nightly recipe does not match its toolchain recipe")
    patches = {item["file"]: source.checked_bytes(ROOT, item["file"], item["sha256"])
               for item in spec["series"]}
    return spec, digest(payload), patches


def toolchain_inputs(base, spec):
    if spec["schema"] == 1:
        return base["toolchain"], base["inputs_sha256"], native.toolchain_spec(base)
    pin = spec["toolchain"]
    version, sha256 = pin["version"], pin["sha256"]
    lock = native.toolchain_spec({"toolchain": version, "inputs_sha256": sha256},
                                 f"catalog/toolchain-cafe-v{version}.json")
    return version, sha256, lock


def stage(directory, upstream, inputs, toolchain, recipe):
    spec, recipe_sha256, patches = recipe
    verify_source(upstream, "HEAD", spec["upstream"])
    for name in OVERLAYS:
        if native.inventory_digest(inputs / name) != toolchain["overlay_sha256"][name]:
            raise ValueError(f"external dependency overlay differs: {name}")
    tv_path = native.BRIDGE_FILES["tv_size"]
    tv_size = source.checked_bytes(ROOT, tv_path, toolchain["bridge_inputs"]["tv_size"])
    first, *remaining = spec["series"]
    hosted = {
        "input_tree": first["output_tree"],
        "output_tree": spec["output_tree"],
        "series": [{"file": item["file"], "tree": item["output_tree"]} for item in remaining],
        "overlay": {},
    }
    staged, tree = native.stage_source(directory, upstream, spec["upstream"]["tree"],
                                      patches[first["file"]], (hosted, recipe_sha256, patches))
    lock = source.checked_bytes(staged, "wiiu/loading_app/Cargo.lock",
                                spec["cargo_lock_sha256"])
    manifest = native.write_cargo_inputs(directory, staged, inputs, toolchain,
                                        {"lock": lock, "tv_size": tv_size}, overlays=OVERLAYS)
    receipt = {
        "schema": 1,
        "kind": "wiiu-hosted-nightly-assembly",
        "recipe_sha256": recipe_sha256,
        "upstream": spec["upstream"],
        "tree": tree,
        "patches": spec["series"],
        "cargo_manifest_sha256": file_digest(manifest),
        "cargo_lock_sha256": spec["cargo_lock_sha256"],
    }
    write_record(directory / "nightly-source-record.json", receipt)
    return staged, manifest, receipt


def assemble(upstream, inputs, output, base, base_fingerprint, build=False, resources_hash=None):
    recipe = read_recipe(base_fingerprint)
    spec, recipe_sha256, _ = recipe
    version, toolchain_sha256, toolchain = toolchain_inputs(base, spec)
    directory = isolated(output)
    directory.mkdir(parents=True, exist_ok=True)
    with native.build_lock(directory):
        staged, manifest, receipt = stage(directory, upstream, inputs, toolchain, recipe)
        if not build:
            return directory / "nightly-source-record.json"
        tools, sysroot, observed = native.native_toolchain(include_host=spec["schema"] == 2)
        if observed != toolchain["toolchain"]:
            changed = sorted(key for key, value in observed.items()
                             if value != toolchain["toolchain"].get(key))
            raise ValueError(f"installed Cafe toolchain differs from the pin: {', '.join(changed)}")
        launcher = client_input(base)
        versions = spec["candidate"]
        pin = spec["resource_pin"]
        build_id = digest(json.dumps({"recipe": recipe_sha256, "launcher": launcher["files"]},
                                     sort_keys=True).encode())
        embedded = {
            "HORIZON_NIGHT_UPSTREAM": spec["upstream"]["commit"],
            "HORIZON_WIIU_HUD_VIEW_LANES": "1",
            "HORIZON_NIGHT_CLIENT": versions["client"],
            "HORIZON_NIGHT_PATCH": versions["patch"],
            "HORIZON_NIGHT_BUILDER": versions["builder"],
            "HORIZON_NIGHT_MINECRAFT": pin["minecraft"],
            "HORIZON_NIGHT_PREP_SCHEMA": str(pin["schema"]),
            "HORIZON_NIGHT_CLIENT_JAR": pin["client_sha256"],
            "HORIZON_NIGHT_SERVER_JAR": pin["server_sha256"],
            "HORIZON_NIGHT_EXPORTER": pin["exporter_sha256"],
            "HORIZON_NIGHT_BUILD_ID": build_id,
        }
        record = directory / "build-record.json"
        packages = native.HOSTED_PACKAGES
        discard_unverified_outputs(directory, record, packages)
        record.unlink(missing_ok=True)
        commands = native.compile_game(directory, staged, manifest, tools, sysroot, packages, embedded,
                                       settings=native.BUILD_SETTINGS,
                                       cargo_features=spec.get("cargo_features", ()))
        code_heap = rpx.read_code_heap(directory / packages[0])
        write_record(directory / "code-heap.json", code_heap)
        rpx.require_aroma_fit(code_heap)
        confirmed_recipe = read_recipe(base_fingerprint)
        _, _, confirmed = stage(directory, upstream, inputs, toolchain, confirmed_recipe)
        if (confirmed != receipt or client_input(base) != launcher
                or toolchain_inputs(base, confirmed_recipe[0]) != (version, toolchain_sha256, toolchain)):
            raise ValueError("hosted Nightly inputs changed during compilation")
        if spec["schema"] == 2 and native.native_toolchain(include_host=True)[2] != observed:
            raise ValueError("hosted Nightly toolchain changed during compilation")
        write_record(record, {
            "schema": 1, "kind": "wiiu-hosted-game-build",
            "recipe_sha256": recipe_sha256, "source": receipt, "versions": versions,
            "toolchain_version": version, "toolchain_spec_sha256": toolchain_sha256,
            "toolchain": tools, "launcher": launcher, "embedded_build": embedded,
            "environment": native.BUILD_SETTINGS, "commands": commands,
            "cargo_release_profile": tomllib.loads(manifest.read_text())["profile"]["release"],
            "code_heap": code_heap,
            "resources_inventory_sha256": resources_hash, "outputs": artifacts(directory, packages),
            "assembly": "passed", "build": "passed", "cemu": "not-run", "hardware": "not-run",
        })
    return record
