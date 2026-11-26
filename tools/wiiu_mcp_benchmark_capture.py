#!/usr/bin/env python3
"""Download and validate a completed Wii U benchmark capture through an existing MCP grant."""

import argparse
import base64
from hashlib import sha256
import json
import os
from pathlib import Path
import tempfile

from gx2_profile import HEADER, V3_ROW, read
from mcp_codex_cemu_test import paired_session
from wiiu_mcp_lifecycle_test import response_text


CHUNK_BYTES = 48 * 1024
MAX_CAPTURE_BYTES = HEADER.size + 4096 * V3_ROW.size


def download(call, instance, capture_id):
    data = bytearray()
    expected_size = None
    while True:
        offset = len(data)
        reply = call("wiiu_benchmark_capture", {"instance": instance, "capture_id": capture_id, "offset": offset})
        page = json.loads(response_text(reply))
        identity = (page["format"], page["encoding"], page["instance"], page["capture_id"], page["offset"])
        if identity != ("HZP3", "base64", instance, capture_id, offset):
            raise ValueError("Night returned a different capture or offset")
        size = page["total_bytes"]
        if (type(size) is not int or not HEADER.size <= size <= MAX_CAPTURE_BYTES
                or (expected_size is not None and size != expected_size)):
            raise ValueError("The capture size is invalid or changed during download")
        expected_size = size
        encoded = page["data"]
        if not isinstance(encoded, str) or len(encoded) > 4 * ((CHUNK_BYTES + 2) // 3):
            raise ValueError("The capture page exceeds the transport limit")
        block = base64.b64decode(encoded, validate=True)
        if not block or len(block) != min(CHUNK_BYTES, expected_size - offset):
            raise ValueError("The capture page has an invalid length")
        data.extend(block)
        if page["next_offset"] != (len(data) if len(data) < expected_size else None):
            raise ValueError("The next capture offset is invalid")
        if len(data) == expected_size:
            break
    metadata, _ = read(data)
    if metadata["version"] != 3:
        raise ValueError("The downloaded capture is not HZP3")
    return data, metadata


def publish(path, data):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as file:
            temporary = Path(file.name)
            file.write(data)
        os.link(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex-home", type=Path, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--server", required=True, help="existing authenticated MCP server name")
    parser.add_argument("--instance", required=True)
    parser.add_argument("--capture-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("choose a new output file")
    try:
        with paired_session(args.codex_home, args.ca, args.server) as session:
            data, metadata = download(session.call, args.instance, args.capture_id)
        publish(args.output, data)
    except (OSError, RuntimeError, ValueError, TypeError, KeyError) as error:
        parser.exit(1, f"Could not retrieve the benchmark capture. {error}\n")
    print(json.dumps({"bytes": len(data), "sha256": sha256(data).hexdigest(), **metadata}))


if __name__ == "__main__":
    main()
