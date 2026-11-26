#!/usr/bin/env python3
"""Rebind a private prepared archive to a new host fixture directory."""

import argparse
from hashlib import file_digest
from pathlib import Path
import shutil
import struct
import sys


sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "tools"))
from rebind_cafe_prepared_archive import ARCHIVE, MANIFEST, payload_digest, read_index, write_rebound_archive


def prepare(source, target):
    source = source.resolve(strict=True)
    target = target.resolve()
    manifest = (source / MANIFEST).read_bytes()
    with (source / ARCHIVE).open("rb") as archive:
        content = payload_digest(archive, read_index(archive, manifest))

    target.mkdir()
    shutil.copyfile(source / MANIFEST, target / MANIFEST)
    write_rebound_archive(source, target / ARCHIVE, target.as_posix())
    with (target / ARCHIVE).open("rb") as archive:
        index = read_index(archive, manifest)
        if index.root != target.as_posix() or payload_digest(archive, index) != content:
            raise ValueError("host archive changed the original preparation")
        archive.seek(0)
        return file_digest(archive, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("preparation", type=Path)
    parser.add_argument("new_host_root", type=Path)
    args = parser.parse_args()
    try:
        print(prepare(args.preparation, args.new_host_root))
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"Could not prepare the host fixture. {error}\n")


if __name__ == "__main__":
    main()
