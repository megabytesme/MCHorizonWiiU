#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
source_dir="$(realpath "$1")/wiiu/loading_host"
mkdir -p "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" \
    -vs "$source_dir/outline.vert" -ps "$source_dir/outline.frag" -o outline.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common outline.gsh outline_gfd.o
powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" \
    "$fixture" "$source_dir/outline.c" outline_gfd.o -lwut -o outline_fixture.elf
elf2rpl outline_fixture.elf outline_fixture.rpx
sha256sum outline_fixture.rpx