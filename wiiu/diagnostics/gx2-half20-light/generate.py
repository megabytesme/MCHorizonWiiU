"""Derive the half-position vertex program from the pinned stock input."""

from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text()
for old, new in (
    ("layout(location = 0) in vec4 aPosition;", "layout(location = 0) in vec3 aPosition;"),
    ("layout(location = 2) in vec2 aUv;",
     "layout(location = 2) in vec2 aUv;\nlayout(location = 3) in float aLight;"),
    ("float packedLight = aPosition.w;", "float packedLight = aLight;"),
):
    if source.count(old) != 1:
        raise ValueError(f"stock shader contract changed: {old!r}")
    source = source.replace(old, new)
Path(sys.argv[2]).write_text(source)
