#!/usr/bin/env python3
"""Check the pinned shader pair and resource linkage of the standalone fixture."""

import hashlib
from pathlib import Path
import re
from struct import unpack_from


def words(data, at, n=1):
    return unpack_from(f">{n}I", data, at)


def block_map(path):
    raw = path.read_bytes()
    assert raw[:4] == b"Gfx2"
    pos, blocks = 32, {}
    while pos < len(raw):
        magic, header, _, _, kind, size, _, _ = words(raw, pos, 8)
        assert magic == 0x424C4B7B and header == 32 and kind not in blocks
        pos += header
        assert pos + size <= len(raw)
        blocks[kind] = raw[pos:pos + size]
        pos += size
    return blocks


def reflection(stage, at, stride):
    count, pointer = words(stage, at, 2)
    assert pointer & 0xFFF00000 == 0xD0600000
    values = {}
    for idx in range(count):
        name, *fields = words(stage, (pointer & 0xFFFFF) + idx * 4 * stride, stride)
        key = stage[name & 0xFFFFF:].split(b"\0", 1)[0].decode()
        values[key] = tuple(fields)
    return values


def interpolate(vertex, pixel, expected):
    controls = words(pixel, 0x10)[0]
    assert controls == expected
    assert (words(pixel, 8)[0] & 31) == controls
    assert (words(vertex, 8)[0] & 0x3E) // 2 + 1 == controls
    semantic_out = list(words(vertex, 0x10)[0].to_bytes(4, "little"))[:controls]
    semantic_in = list(words(pixel, 0x14, controls))
    # The exact programs use matching semantic names and default smooth control.
    assert semantic_in == semantic_out == list(range(0x8A, 0x8A + controls))


def inspect(label, stock, metadata):
    here = block_map(Path(label + ".gsh"))
    vs, ps = here[3], here[6]
    a, b = stock[3], stock[6]
    assert reflection(vs, 0x104, 4) == reflection(a, 0x104, 4)
    assert reflection(vs, 0xE4, 5) == reflection(a, 0xE4, 5)
    assert reflection(vs, 0xFC, 3) == reflection(a, 0xFC, 3) == {"uLightmap": (1, 0)}
    assert reflection(ps, 0xB8, 5) == reflection(b, 0xB8, 5)
    assert reflection(ps, 0xD0, 3) == reflection(b, 0xD0, 3) == {"uTerrain": (1, 0)}
    assert words(ps, 0x94, 4) == words(b, 0x94, 4)
    assert words(ps, 0x9C) == (0x50,)
    if label == "centered":
        assert ps == b and here[7] == stock[7]
        assert (vs[4:0xD0], vs[0xD4:]) == (a[4:0xD0], a[0xD4:])

    size_vs, regs_vs, groups_vs, size_ps, regs_ps, groups_ps, exports = metadata
    assert len(here[5]) // 4 == size_vs and len(here[7]) // 4 == size_ps
    assert words(vs, 0)[0] & 127 == regs_vs and words(ps, 0)[0] & 127 == regs_ps
    interpolate(vs, ps, exports)
    vertex, pixel = Path(label + ".disasm").read_text().split("--------------------------------------")[:2]
    assert len(re.findall(r"\s\d+ [xyzwts]:", vertex)) == groups_vs
    assert len(re.findall(r"\s\d+ [xyzwts]:", pixel)) == groups_ps
    assert vertex.count("SAMPLE_LZ") == 1 and pixel.count("SAMPLE ") == 1
    assert pixel.count("KILL") == 2 and pixel.count("RECIP_IEEE") == 2
    assert len(re.findall(r"EXPORT(?:_DONE)?\s+PARAM ", vertex)) == exports
    print(f"{label:8} VS {size_vs:3} dw {regs_vs} GPR {groups_vs} groups | "
          f"PS {size_ps:3} dw {regs_ps} GPR {groups_ps} groups | {exports} smooth PARAM")


assert hashlib.sha256(Path("stock.gsh").read_bytes()).hexdigest() == \
    "c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133"
stock = block_map(Path("stock.gsh"))
for name, numbers in {
    "stock": (178, 6, 18, 142, 5, 24, 4),
    "centered": (162, 5, 18, 142, 5, 24, 4),
    "combined": (158, 5, 17, 142, 4, 24, 3),
}.items():
    inspect(name, stock, numbers)
