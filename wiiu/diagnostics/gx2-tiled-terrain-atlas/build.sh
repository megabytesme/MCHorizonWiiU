#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_RC61_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "$0")" && pwd -P)"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
readback="$fixture/../gx2-explicit-readback"
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

cp "$terrain/terrain.c" "$terrain/terrain.h" "$terrain/terrain_buffers.h" .
patch --batch --forward -p1 < "$fixture/terrain-scratch.patch"
"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o stock.gsh
"$compiler" -vs "$fixture/draw_clear.vert" -ps "$fixture/draw_clear.frag" -o test_color.gsh
for shader in stock test_color; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "$shader.o"
done
powerpc-eabi-gcc -std=gnu11 -O2 -Wall -Wextra -Werror -D__WIIU__ -mcpu=750 -meabi -mhard-float \
    -ffunction-sections -fdata-sections -I"$wut/include" -I"$PWD" -I"$fixture" -I"$readback" \
    -specs="$wut/share/wut.specs" -L"$wut/lib" -Wl,--gc-sections \
    "$fixture/main.c" stock.o test_color.o -lwut -o atlas-1d-tile-draw-clear.elf
elf2rpl atlas-1d-tile-draw-clear.elf atlas-1d-tile-draw-clear.rpx
readrpl -h -c -f atlas-1d-tile-draw-clear.rpx >readrpl.txt
sha256sum atlas-1d-tile-draw-clear.rpx stock.gsh test_color.gsh
