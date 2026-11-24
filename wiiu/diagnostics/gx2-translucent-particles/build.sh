#!/usr/bin/env bash
set -euo pipefail
if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
source_dir="$(realpath "$1")/wiiu"
mkdir -p "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"
"$compiler" -vs "$source_dir/gx2_terrain/terrain.vert" -ps "$source_dir/gx2_terrain/terrain.frag" -o terrain.gsh
"$compiler" -vs "$source_dir/loading_host/hand.vert" -ps "$source_dir/loading_host/hand.frag" -o hand.gsh
for shader in terrain hand; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "${shader}_gfd.o"
done
powerpc-eabi-gcc -O2 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$wut/include" -I"$source_dir/gx2_terrain" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture" "$source_dir/gx2_terrain/terrain.c" \
    "$source_dir/loading_host/hand.c" terrain_gfd.o hand_gfd.o -lwut -o particle_fixture.elf
elf2rpl particle_fixture.elf particle_fixture.rpx
sha256sum particle_fixture.rpx
