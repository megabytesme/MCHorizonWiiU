#!/usr/bin/env python3
"""Requires the exact pinned RC64 stock shader and modifies only DB.Z_ORDER."""
from hashlib import sha256
from pathlib import Path
from struct import unpack_from

stock = Path("terrain.gsh").read_bytes()
assert sha256(stock).hexdigest() == "c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133"
pos = 32
changed = None
while pos < len(stock):
    magic, header, _, _, kind, n, _, _ = unpack_from(">8I", stock, pos)
    assert magic == 0x424c4b7b and header == 32 and pos+header+n <= len(stock)
    pos += header
    if kind == 6:
        assert changed is None and unpack_from(">I", stock, pos+0x9c)[0] == 0x50
        changed = pos+0x9f
    pos += n
assert changed == 0x697 and pos == len(stock)
patched = stock[:changed] + b"\x70" + stock[changed+1:]
assert sha256(patched).hexdigest() == "55702fd7cc26dbb3b61d303093cfb68b9387e2ae551ee69a81a7d5f2bc5d3e53"
Path("terrain_rez.gsh").write_bytes(patched)
