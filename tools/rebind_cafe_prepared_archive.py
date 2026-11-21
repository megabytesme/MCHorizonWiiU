#!/usr/bin/env python3
"""Copy a private prepared archive with the root path seen by CafeOS."""

import argparse
from hashlib import file_digest
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


def padded(length):
    return (length + 7) & ~7


def rebind(preparation, output, guest_root):
    root = PurePosixPath(guest_root)
    if (root.as_posix() != guest_root or root.parts[:2] != ("/", "vol")
            or len(root.parts) < 4 or ".." in root.parts or "\0" in guest_root):
        raise ValueError("provide a normalized absolute Cafe resource path")
    guest = guest_root.encode("utf-8")
    manifest = (preparation / MANIFEST).read_bytes()
    archive = preparation / ARCHIVE
    if output.exists() or output.is_symlink():
        raise ValueError("choose a new output file for the private archive")
    with archive.open("rb") as source:
        size = os.fstat(source.fileno()).st_size
        magic, version, count, old_root, source_size, pool_size = HEADER.unpack(source.read(HEADER.size))
        if (magic, version) != (MAGIC, 2) or count == 0 or source_size != len(manifest):
            raise ValueError("expected a version 2 archive with the preparation's manifest")
        index_start = HEADER.size + padded(old_root) + padded(source_size)
        pool_start = index_start + count * ENTRY.size
        payload_start = pool_start + padded(pool_size)
        if payload_start > size:
            raise ValueError("prepared archive index extends beyond the input")
        source.seek(HEADER.size + padded(old_root))
        if source.read(padded(source_size)) != manifest.ljust(padded(source_size), b"\0"):
            raise ValueError("prepared archive belongs to a different manifest")
        entries = source.read(count * ENTRY.size)
        pool = source.read(padded(pool_size))
        if source.tell() != payload_start:
            raise ValueError("truncated prepared archive index")

        shift = padded(len(guest)) - padded(old_root)
        with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as target:
            scratch = Path(target.name)
            try:
                target.write(HEADER.pack(MAGIC, version, count, len(guest), source_size, pool_size))
                target.write(guest.ljust(padded(len(guest)), b"\0"))
                target.write(manifest.ljust(padded(source_size), b"\0"))
                for name_at, name_len, offset, length in ENTRY.iter_unpack(entries):
                    if (not name_len or name_at + name_len > pool_size
                            or not payload_start <= offset <= offset + length <= size):
                        raise ValueError("prepared archive contains an out-of-range entry")
                    target.write(ENTRY.pack(name_at, name_len, offset + shift, length))
                target.write(pool)
                shutil.copyfileobj(source, target)
                if target.tell() != size + shift:
                    raise ValueError("prepared archive changed while copying")
                target.close()
                os.link(scratch, output)
            finally:
                target.close()
                scratch.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("preparation", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--guest-root", default=DEFAULT_ROOT)
    args = parser.parse_args()
    try:
        rebind(args.preparation, args.output, args.guest_root)
        with args.output.open("rb") as archive:
            print(file_digest(archive, "sha256").hexdigest())
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"Could not rebind the private prepared archive. {error}\n")


if __name__ == "__main__":
    main()
