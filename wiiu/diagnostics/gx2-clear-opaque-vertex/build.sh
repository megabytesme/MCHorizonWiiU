#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
terrain="$(realpath "$1")/wiiu/gx2_terrain"
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

awk 'NR == 1 { print; print "#define HORIZON_TERRAIN_OPAQUE 1"; next } { print }' \
    "$terrain/terrain.frag" >opaque.frag
awk 'NR == 1 { print; print "#define HORIZON_TERRAIN_CLEAR_FOG 1"; next } { print }' \
    opaque.frag >clear.frag
awk 'NR == 1 { print; print "#define HORIZON_TERRAIN_CLEAR_VERTEX 1"; next } { print }' \
    "$terrain/terrain.vert" >clear.vert
sed '/^out vec2 vFogDistance;$/d; /^[[:space:]]*vFogDistance = vec2(0.0);$/d' \
    clear.vert >missing-export.vert

"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o terrain.gsh
"$compiler" -vs "$terrain/terrain.vert" -ps opaque.frag -o opaque.gsh
"$compiler" -vs "$terrain/terrain.vert" -ps clear.frag -o clear_pixel.gsh
"$compiler" -vs clear.vert -ps clear.frag -o clear_vertex.gsh
"$compiler" -vs missing-export.vert -ps clear.frag -o missing_export.gsh
objects=()
for arm in terrain opaque clear_pixel clear_vertex missing_export; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$arm.gsh" "${arm}_gfd.o"
    objects+=("${arm}_gfd.o")
done
powerpc-eabi-gcc -O2 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -DHORIZON_GX2_CLEAR_FOG=1 -Wall -Wextra -Werror -I"$wut/include" -I"$terrain" \
    -B"$wut/share" -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture" "$terrain/terrain.c" \
    "${objects[@]}" -lwut -o clear_opaque_vertex.elf
elf2rpl clear_opaque_vertex.elf clear_opaque_vertex.rpx
sha256sum clear_pixel.gsh clear_vertex.gsh clear_opaque_vertex.rpx
