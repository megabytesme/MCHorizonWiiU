"""Prepare the pinned hosted Nightly candidate outside the active release recipe."""
import native
import source
from core import (ROOT, TARGET, digest, fields, file_digest, isolated, parse_json,
                  pattern, verify_source, write_record)

MANIFEST = ROOT / "catalog/hosted-nightly-2026-11-20.json"
OVERLAYS = ("ring", "png", "ureq")


def validate(spec):
    fields(spec, "schema", "kind", "target", "upstream", "base_recipe_sha256",
           "cargo_lock_sha256", "output_tree", "series")
    if (type(spec["schema"]) is not int or spec["schema"] != 1
            or spec["kind"] != "wiiu-hosted-nightly-source" or spec["target"] != TARGET):
        raise ValueError("unsupported hosted Nightly source recipe")
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


def assemble(upstream, inputs, output, base, base_fingerprint):
    spec, recipe_sha256, patches = read_recipe(base_fingerprint)
    verify_source(upstream, "HEAD", spec["upstream"])
    toolchain = native.toolchain_spec(base)
    for name in OVERLAYS:
        if native.inventory_digest(inputs / name) != toolchain["overlay_sha256"][name]:
            raise ValueError(f"external dependency overlay differs: {name}")
    tv_path = native.BRIDGE_FILES["tv_size"]
    tv_size = source.checked_bytes(ROOT, tv_path, toolchain["bridge_inputs"]["tv_size"])
    directory = isolated(output)
    directory.mkdir(parents=True, exist_ok=True)
    first, *remaining = spec["series"]
    hosted = {
        "input_tree": first["output_tree"],
        "output_tree": spec["output_tree"],
        "series": [{"file": item["file"], "tree": item["output_tree"]} for item in remaining],
        "overlay": {},
    }
    with native.build_lock(directory):
        staged, tree = native.stage_source(directory, upstream, spec["upstream"]["tree"],
                                          patches[first["file"]], (hosted, recipe_sha256, patches))
        lock = source.checked_bytes(staged, "wiiu/loading_app/Cargo.lock",
                                    spec["cargo_lock_sha256"])
        manifest = native.write_cargo_inputs(directory, staged, inputs, toolchain,
                                            {"lock": lock, "tv_size": tv_size}, overlays=OVERLAYS)
        record = directory / "nightly-source-record.json"
        write_record(record, {
            "schema": 1,
            "kind": "wiiu-hosted-nightly-assembly",
            "recipe_sha256": recipe_sha256,
            "upstream": spec["upstream"],
            "tree": tree,
            "patches": spec["series"],
            "cargo_manifest_sha256": file_digest(manifest),
            "cargo_lock_sha256": spec["cargo_lock_sha256"],
        })
    return record
