#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
if ! grep -q HORIZON_TERRAIN_NEAR_FOG "$terrain/terrain.frag"; then
    printf 'Assembled source must include the matching GX2 near-fog trial.\n' >&2
    exit 2
fi
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

awk 'NR == 1 { print; print "#define HORIZON_TERRAIN_NEAR_FOG 1"; next } { print }' \
    "$terrain/terrain.frag" > terrain_near_fog.frag
"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o terrain.gsh -v > terrain.disasm 2>&1
"$compiler" -vs "$terrain/terrain.vert" -ps terrain_near_fog.frag -o terrain_near_fog.gsh -v > terrain_near_fog.disasm 2>&1
for shader in terrain terrain_near_fog; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "$shader.o"
done
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -DHORIZON_GX2_CLEAR_FOG=1 -O2 -Wall -Wextra -Werror -I"$terrain" -I"$wut/include" \
    -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture" "$terrain/terrain.c" \
    terrain.o terrain_near_fog.o -lwut -lm -o near-fog.elf
elf2rpl near-fog.elf near-fog.rpx
readrpl -a near-fog.rpx > near-fog-readrpl.log
sha256sum terrain.gsh terrain_near_fog.gsh near-fog.rpx
