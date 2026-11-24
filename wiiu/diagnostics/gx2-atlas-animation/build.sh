#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
source_dir="$(realpath "$1")/wiiu/gx2_terrain"
mkdir -p "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" \
    -vs "$source_dir/terrain.vert" -ps "$source_dir/terrain.frag" -o terrain.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common terrain.gsh terrain_gfd.o
powerpc-eabi-gcc -O2 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$wut/include" -I"$source_dir" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture" "$source_dir/terrain.c" terrain_gfd.o \
    -lwut -o animation_fixture.elf
elf2rpl animation_fixture.elf animation_fixture.rpx
sha256sum animation_fixture.rpx
