#!/usr/bin/env python3
"""Copy or verify a private prepared archive for the resource path seen by CafeOS."""

import argparse
from dataclasses import dataclass
from hashlib import file_digest, sha256
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import struct
import tempfile


HEADER = struct.Struct("<8sIIQQQ")
ENTRY = struct.Struct("<IIQQ")
MAGIC = b"HZDATA\0\0"
ARCHIVE = "horizon-data.bin"
MANIFEST = "horizon-resources.json"
DEFAULT_ROOT = "/vol/external01/wiiu/apps/HorizonWiiU/.local/horizon-client/26.3"
DEFAULT_RECIPE = Path(__file__).resolve().parents[1] / "catalog/hosted-nightly-2026-11-20.json"


@dataclass(frozen=True)
class Index:
    root: str
    entries: bytes
    pool: bytes
    pool_size: int
    payload_at: int
    size: int


def padded(length):
    return (length + 7) & ~7


def guest_path(text):
    root = PurePosixPath(text)
    if (root.as_posix() != text or root.parts[:2] != ("/", "vol")
            or len(root.parts) < 4 or ".." in root.parts or "\\" in text or "\0" in text):
        raise ValueError("provide a normalized absolute Cafe resource path")
    return text


def read_index(source, manifest):
    size = os.fstat(source.fileno()).st_size
    magic, version, count, root_size, source_size, pool_size = HEADER.unpack(source.read(HEADER.size))
    if (magic, version) != (MAGIC, 2) or count == 0 or source_size != len(manifest):
        raise ValueError("expected a version 2 archive with the preparation's manifest")
    payload_at = (
        HEADER.size + padded(root_size) + padded(source_size)
        + count * ENTRY.size + padded(pool_size)
    )
    if payload_at > size:
        raise ValueError("prepared archive index extends beyond the input")
    root = source.read(padded(root_size))[:root_size].decode("utf-8")
    if source.read(padded(source_size)) != manifest.ljust(padded(source_size), b"\0"):
        raise ValueError("prepared archive belongs to a different manifest")
    entries = source.read(count * ENTRY.size)
    pool = source.read(padded(pool_size))
    if source.tell() != payload_at:
        raise ValueError("truncated prepared archive index")

    last = ""
    pool[:pool_size].decode("utf-8")
    for name_at, name_len, offset, length in ENTRY.iter_unpack(entries):
        if (not name_len or name_at + name_len > pool_size
                or not payload_at <= offset <= offset + length <= size):
            raise ValueError("prepared archive contains an out-of-range entry")
        name = pool[name_at:name_at + name_len].decode("utf-8")
        if name <= last:
            raise ValueError("prepared archive entry names are out of order or duplicated")
        last = name
    return Index(root, entries, pool, pool_size, payload_at, size)


def payload_digest(source, index):
    """Hash the name table, normalized entry offsets and payload, independent of the bound root."""
    digest = sha256()
    digest.update(struct.pack("<QQ", len(index.entries), index.pool_size))
    digest.update(index.pool[:index.pool_size])
    for name_at, name_len, offset, length in ENTRY.iter_unpack(index.entries):
        digest.update(ENTRY.pack(name_at, name_len, offset - index.payload_at, length))
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(chunk)
    if source.tell() != index.size:
        raise ValueError("prepared archive changed while hashing")
    return digest.hexdigest()


def preflight(preparation, archive, guest_root, recipe, expected_sha256=None, payload_from=None):
    guest_path(guest_root)
    manifest = (preparation / MANIFEST).read_bytes()
    metadata = json.loads(manifest)
    spec = json.loads(recipe.read_bytes())
    pin = spec.get("resource_pin") if isinstance(spec, dict) else None
    keys = ("minecraft", "schema", "client_sha256", "server_sha256", "exporter_sha256")
    if not isinstance(pin, dict) or not isinstance(metadata, dict) or any(
        key not in pin or pin[key] != metadata.get(key) for key in keys
    ):
        raise ValueError("prepared manifest does not match the recipe's Minecraft, schema and exporter pins")
    with archive.open("rb") as source:
        index = read_index(source, manifest)
        if index.root != guest_root:
            raise ValueError(
                f"prepared archive root mismatch: expected {guest_root!r}, found {index.root!r}; "
                "rebind a private copy before deploying"
            )
        content = payload_digest(source, index)
        source.seek(0)
        digest = file_digest(source, "sha256").hexdigest()
    if expected_sha256 is not None and digest != expected_sha256:
        raise ValueError("prepared archive SHA-256 does not match the verified copy")
    if payload_from is not None:
        with payload_from.open("rb") as original:
            original_index = read_index(original, manifest)
            if content != payload_digest(original, original_index):
                raise ValueError("prepared archive entries and payload do not match the original archive")
    return {
        "guest_root": guest_root,
        "minecraft": pin["minecraft"],
        "schema": pin["schema"],
        "archive_version": 2,
        "entries": len(index.entries) // ENTRY.size,
        "manifest_sha256": sha256(manifest).hexdigest(),
        "archive_sha256": digest,
        "content_sha256": content,
    }


def rebind(preparation, output, guest_root):
    guest = guest_path(guest_root).encode("utf-8")
    manifest = (preparation / MANIFEST).read_bytes()
    if output.exists() or output.is_symlink():
        raise ValueError("choose a new output file for the private archive")
    with (preparation / ARCHIVE).open("rb") as source:
        index = read_index(source, manifest)
        shift = padded(len(guest)) - padded(len(index.root.encode("utf-8")))
        with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as target:
            scratch = Path(target.name)
            try:
                target.write(HEADER.pack(
                    MAGIC, 2, len(index.entries) // ENTRY.size, len(guest), len(manifest), index.pool_size,
                ))
                target.write(guest.ljust(padded(len(guest)), b"\0"))
                target.write(manifest.ljust(padded(len(manifest)), b"\0"))
                for name_at, name_len, offset, length in ENTRY.iter_unpack(index.entries):
                    target.write(ENTRY.pack(name_at, name_len, offset + shift, length))
                target.write(index.pool)
                shutil.copyfileobj(source, target)
                if target.tell() != index.size + shift:
                    raise ValueError("prepared archive changed while copying")
                target.close()
                os.link(scratch, output)
            finally:
                target.close()
                scratch.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("preparation", type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--output", type=Path, help="create a separately bound archive")
    mode.add_argument("--check", action="store_true", help="verify an archive without modifying it")
    parser.add_argument("--guest-root", default=DEFAULT_ROOT)
    parser.add_argument("--archive", type=Path, help="check this archive instead of the preparation's default")
    parser.add_argument("--recipe", type=Path, help="check compatibility with this hosted source recipe")
    parser.add_argument("--sha256", help="check the archive against a previously verified copy")
    parser.add_argument("--payload-from", type=Path, help="check that rebinding preserved an original archive's contents")
    args = parser.parse_args()
    if not args.check and any((args.archive, args.recipe, args.sha256, args.payload_from)):
        parser.error("--archive, --recipe, --sha256 and --payload-from require --check")
    try:
        if args.check:
            result = preflight(
                args.preparation, args.archive or args.preparation / ARCHIVE,
                args.guest_root, args.recipe or DEFAULT_RECIPE, args.sha256, args.payload_from,
            )
            print(json.dumps(result, indent=2))
        else:
            rebind(args.preparation, args.output, args.guest_root)
            with args.output.open("rb") as archive:
                print(file_digest(archive, "sha256").hexdigest())
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"Could not {'verify' if args.check else 'rebind'} the private prepared archive. {error}\n")


if __name__ == "__main__":
    main()
