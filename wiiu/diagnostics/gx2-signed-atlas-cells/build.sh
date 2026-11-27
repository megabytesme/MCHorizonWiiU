#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
retirement="${HORIZON_EXPLICIT_READBACK_INCLUDE:-$fixture/../gx2-explicit-readback}"
if ! grep -q HORIZON_TERRAIN_ATLAS_CELLS "$terrain/terrain.vert"; then
    printf 'Assembled source must include the stock-quantized atlas cell trial.\n' >&2
    exit 2
fi
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

awk 'NR == 1 { print; print "#define HORIZON_TERRAIN_ATLAS_CELLS 1"; next } { print }' \
    "$terrain/terrain.vert" > atlas_cell.vert
"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o stock.gsh -v > stock.disasm 2>&1
"$compiler" -vs atlas_cell.vert -ps "$terrain/terrain.frag" -o atlas_cell.gsh -v > atlas_cell.disasm 2>&1
for shader in stock atlas_cell; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "$shader.o"
done
powerpc-eabi-gcc -std=gnu11 -O2 -Wall -Wextra -Werror -D__WIIU__ -mcpu=750 -meabi -mhard-float \
    -ffunction-sections -fdata-sections -I"$wut/include" -I"$terrain" -I"$retirement" \
    -specs="$wut/share/wut.specs" -L"$wut/lib" -Wl,--gc-sections \
    "$fixture/main.c" "$terrain/terrain.c" stock.o atlas_cell.o -lwut -o atlas-quantized.elf
elf2rpl atlas-quantized.elf atlas-quantized.rpx
readrpl -a atlas-quantized.rpx > atlas-quantized-readrpl.log
sha256sum stock.gsh atlas_cell.gsh atlas-quantized.rpx
