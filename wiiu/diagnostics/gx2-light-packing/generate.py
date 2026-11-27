#!/usr/bin/env python3
"""Generate the independently selectable fixture programs from the stock pair."""

from pathlib import Path
import sys


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f"stock shader contract changed: {old!r}")
    return source.replace(old, new)


terrain = Path(sys.argv[1])
vertex = (terrain / "terrain.vert").read_text()
fragment = (terrain / "terrain.frag").read_text()
Path("stock.vert").write_text(vertex)
Path("stock.frag").write_text(fragment)
old_light = (
    "    float packedLight = aPosition.w;\n"
    "    vec2 levels = min(vec2(floor(packedLight / 256.0), "
    "mod(packedLight, 256.0)), vec2(240.0));\n"
    "    vec2 lightUv = levels / 256.0 + vec2(0.5 / 16.0);\n"
)
new_light = (
    "    float light = aPosition.w;\n"
    "    vec2 lightUv = vec2(floor(light) * (1.0 / 256.0), fract(light));\n"
)
vertex = replace_once(vertex, old_light, new_light)
Path("centered.vert").write_text(vertex)

for path, text, qualifier in (
    ("centered-color.vert", vertex, "out"),
    ("centered-color.frag", fragment, "in"),
):
    text = replace_once(text, f"{qualifier} vec3 vColor;", f"{qualifier} vec4 vColorCutout;")
    text = replace_once(text, f"{qualifier} float vCutout;\n", "")
    if qualifier == "out":
        text = replace_once(text, "vColor =", "vColorCutout.rgb =")
        text = text.replace("vCutout =", "vColorCutout.a =")
        wrong_alpha = replace_once(text, "vColorCutout.a = aColor.a;", "vColorCutout.a = 1.0;")
        Path("bad-alpha.vert").write_text(wrong_alpha)
    else:
        text = text.replace("vColor,", "vColorCutout.rgb,").replace("vCutout", "vColorCutout.a")
    Path(path).write_text(text)
