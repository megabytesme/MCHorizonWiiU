#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE NEW_BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "$0")" && pwd -P)"
port="$(cd "$fixture/../../.." && pwd -P)"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
readback="$fixture/../gx2-explicit-readback"
clear="$fixture/../gx2-tiled-terrain-atlas"

# Share the exact input pins with the Windows build.
python3 -B - "$fixture" "$terrain" "$port" <<'PYTHON'
from hashlib import sha256
import json
from pathlib import Path
import sys

fixture, engine, port = map(Path, sys.argv[1:])
roots = dict(fixture=fixture, engine=engine, port=port)
recipe = json.loads((fixture / "windows-recipe.json").read_text(encoding="utf-8"))
for key, wanted in recipe["inputs"].items():
    root, name = key.split("/", 1)
    source = roots[root] / name
    if sha256(source.read_bytes()).hexdigest() != wanted:
        sys.exit(f"Unexpected fixture input: {source}")
PYTHON
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"
"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o stock.gsh -v > stock.disasm.txt 2>&1
"$compiler" -vs "$clear/draw_clear.vert" -ps "$clear/draw_clear.frag" -o test_color.gsh -v > test_color.disasm.txt 2>&1
sha256sum -c <<'SHADERS'
c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133  stock.gsh
5c29bc3ca5bd234eab66b6777731b47a184d2b1a36f8ca035c1a95b04d002470  test_color.gsh
SHADERS
for shader in stock test_color; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "$shader.o"
done
powerpc-eabi-gcc -std=gnu11 -O2 -Wall -Wextra -Werror -D__WIIU__ -mcpu=750 -meabi -mhard-float \
    -ffunction-sections -fdata-sections -I"$wut/include" -I"$terrain" -I"$readback" -I"$clear" \
    -specs="$wut/share/wut.specs" -L"$wut/lib" -Wl,--gc-sections \
    "$fixture/main.c" stock.o test_color.o -lwut -o auto-quads.elf
elf2rpl auto-quads.elf auto-quads.rpx
readrpl -a auto-quads.rpx > readrpl.txt
wuhbtool auto-quads.rpx auto-quads.wuhb --name='GX2 Automatic Terrain Quads' --short-name='GX2 Quads' --author=Horizon
sha256sum auto-quads.rpx auto-quads.wuhb
