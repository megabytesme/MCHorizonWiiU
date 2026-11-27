#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_RC68_SOURCE NEW_BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "$0")" && pwd -P)"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
readback="$fixture/../gx2-explicit-readback"
clear="$fixture/../gx2-tiled-terrain-atlas"
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

python3 -B "$fixture/shader_sources.py" "$terrain/terrain.frag" .
for shader in stock clamped gutter; do
    "$compiler" -vs "$terrain/terrain.vert" -ps "$shader.frag" -o "$shader.gsh"
done
sha256sum -c <<'SHADERS'
c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133  stock.gsh
e45ce7109160c329ea2492deb8af5fa08d3c0745ac1601ffe8f5dac6274bdad1  clamped.gsh
f941aa1a713dcbfe1e9a28d8bfed6ac30f345c5937cb280304b3e49195064473  gutter.gsh
SHADERS
"$compiler" -vs "$clear/draw_clear.vert" -ps "$clear/draw_clear.frag" -o test_color.gsh
for shader in stock clamped gutter test_color; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "$shader.o"
done
powerpc-eabi-gcc -std=gnu11 -O2 -Wall -Wextra -Werror -D__WIIU__ -mcpu=750 -meabi -mhard-float \
    -ffunction-sections -fdata-sections -I"$wut/include" -I"$terrain" -I"$readback" \
    -specs="$wut/share/wut.specs" -L"$wut/lib" -Wl,--gc-sections \
    "$fixture/main.c" stock.o clamped.o gutter.o test_color.o -lwut -o atlas-parity.elf
elf2rpl atlas-parity.elf atlas-parity.rpx
readrpl -h -c -f atlas-parity.rpx >readrpl.txt
sha256sum atlas-parity.rpx stock.gsh clamped.gsh gutter.gsh test_color.gsh
