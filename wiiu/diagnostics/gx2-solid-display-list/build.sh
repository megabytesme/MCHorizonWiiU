#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
assembled="$(realpath "$1")/wiiu"
terrain="$assembled/gx2_terrain"
host="$assembled/loading_host"
mkdir "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
compiler="${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}"
wut="$DEVKITPRO/wut"

"$compiler" -vs "$terrain/terrain.vert" -ps "$terrain/terrain.frag" -o terrain.gsh -v > terrain.disasm 2>&1
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common terrain.gsh terrain.o
"$compiler" -vs "$fixture/solid_list_color.vert" -ps "$fixture/solid_list_color.frag" \
    -o solid_list_color.gsh -v > solid-list-color.disasm 2>&1
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common solid_list_color.gsh solid_list_color.o
"$compiler" -vs "$host/world_scale.vert" -ps "$host/world_scale.frag" \
    -o world_scale.gsh -v > world-scale.disasm 2>&1
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common world_scale.gsh world_scale.o
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$terrain" -I"$host" -I"$wut/include" \
    -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture/main.c" "$terrain/terrain.c" "$host/world_scale.c" \
    terrain.o solid_list_color.o world_scale.o -lwut -lm -o solid-list.elf
elf2rpl solid-list.elf solid-list.rpx
readrpl -a solid-list.rpx > solid-list-readrpl.log
sha256sum terrain.gsh solid_list_color.gsh world_scale.gsh solid-list.rpx
