"""Reconstruct the exact Cargo overlays from verified public source archives."""
from pathlib import Path
import shutil
import tarfile
import tempfile
from urllib.request import urlopen

from core import ROOT, file_digest, fields, parse_json
from native import OVERLAYS, inventory_digest, toolchain_spec
from source import checked_bytes, relative

SPEC = ROOT / "catalog/dependencies-cafe-v1.json"
DELTAS = ROOT / "patches/toolchain/cafe-overlays-v1"
MAX_CRATE_SIZE = 16 * 1024**2


def get_archive(record, cache, offline):
    fields(record, "name", "url", "sha256")
    name = relative(record["name"])
    crate, separator, _ = name.name.rpartition("-")
    if len(name.parts) != 1 or not separator or record["url"] != f"https://static.crates.io/crates/{crate}/{name}":
        raise ValueError("unsupported Cargo source archive URL")
    path = cache / name
    if path.is_file():
        if file_digest(path) == record["sha256"]:
            return path
        if offline:
            raise ValueError(f"{name} failed SHA-256 validation. Prepare dependencies online to repair it.")
    elif offline:
        raise ValueError(f"{name} is missing. Prepare dependencies online once.")
    with tempfile.TemporaryDirectory(prefix="crate-", dir=cache) as scratch:
        staged = Path(scratch) / name
        with urlopen(record["url"], timeout=30) as response, staged.open("wb") as output:
            if response.geturl() != record["url"]:
                raise ValueError("unexpected crate archive redirect")
            output.write(response.read(MAX_CRATE_SIZE + 1))
        if staged.stat().st_size > MAX_CRATE_SIZE or file_digest(staged) != record["sha256"]:
            raise ValueError(f"downloaded {name} failed SHA-256 validation")
        staged.replace(path)
    return path


def extract_archive(archive, destination):
    prefix = archive.stem + "/"
    with tarfile.open(archive) as stream:
        for member in stream:
            if member.isdir():
                continue
            if not member.isfile() or not member.name.startswith(prefix):
                raise ValueError(f"invalid member in {archive.name}")
            path = destination / relative(member.name[len(prefix):])
            path.parent.mkdir(parents=True, exist_ok=True)
            with stream.extractfile(member) as payload, path.open("xb") as output:
                shutil.copyfileobj(payload, output)


def verify_devkit_archives(directory):
    spec = parse_json(SPEC.read_bytes())
    for name, expected in spec["devkitpro_archives"].items():
        path = directory / relative(name)
        if not path.is_file() or file_digest(path) != expected:
            raise ValueError(f"missing or modified official devkitPro archive: {name}")


def provision(cache, recipe, offline):
    spec = parse_json(SPEC.read_bytes())
    fields(spec, "schema", "version", "overlays", "devkitpro_archives")
    lock = toolchain_spec(recipe)
    if spec["schema"] != 1 or spec["version"] != lock["version"]:
        raise ValueError("dependency overlay recipe does not match the toolchain")
    fields(spec["overlays"], *OVERLAYS)
    archives = cache / "crates"
    archives.mkdir(exist_ok=True)
    output = cache / f"cafe-overlays-{file_digest(SPEC)}"
    output.mkdir(exist_ok=True)
    for name, record in spec["overlays"].items():
        fields(record, "archive", "replace", "remove", "inventory_sha256")
        expected = lock["overlay_sha256"][name]
        if record["inventory_sha256"] != expected:
            raise ValueError(f"conflicting inventory for {name}")
        target = output / name
        if target.exists():
            if inventory_digest(target) != expected:
                raise ValueError(f"cached {name} differs from its pin. Move that directory out of {output} and retry Prepare.")
            continue
        with tempfile.TemporaryDirectory(prefix="overlay-", dir=output) as scratch:
            candidate = Path(scratch) / name
            candidate.mkdir()
            if record["archive"]:
                archive = get_archive(record["archive"], archives, offline)
                extract_archive(archive, candidate)
            for path in record["remove"]:
                (candidate / relative(path)).unlink()
            for path, expected_file in record["replace"].items():
                output_file = candidate / relative(path)
                output_file.parent.mkdir(parents=True, exist_ok=True)
                output_file.write_bytes(checked_bytes(DELTAS / name, path, expected_file))
            if inventory_digest(candidate) != expected:
                raise ValueError(f"reconstructed {name} differs from its pin")
            candidate.rename(target)
    return output
