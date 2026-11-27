"""Prepare two controlled pixel-source changes from the assembled stock shader."""

import argparse
from pathlib import Path


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f"expected a single stock atlas expression {old!r}")
    return source.replace(old, new)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stock", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    stock = args.stock.read_bytes()
    clamped = replace_once(stock, b"max(uMaterial.y, 1.0)", b"uMaterial.y")
    clamped = replace_once(clamped, b"mix(vUv.xy, atlas_uv, step(1.0, uMaterial.y))", b"atlas_uv")
    gutter = replace_once(stock, b"clamp(local * side, vec2(0.5), vec2(side - 0.5))", b"local * side")
    for name, source in (("stock", stock), ("clamped", clamped), ("gutter", gutter)):
        with (args.output / f"{name}.frag").open("xb") as target:
            target.write(source)


if __name__ == "__main__":
    main()
