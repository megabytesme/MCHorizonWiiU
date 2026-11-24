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
    -vs "$source_dir/stock_sky.vert" -ps "$source_dir/stock_sky.frag" -o stock_sky.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common stock_sky.gsh stock_sky_gfd.o
powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$wut/include" -I"$source_dir" -L"$wut/lib" -specs="$wut/share/wut.specs" \
    "$fixture" "$source_dir/stock_sky.c" stock_sky_gfd.o -lwut -o stock_sky_fixture.elf
elf2rpl stock_sky_fixture.elf stock_sky_fixture.rpx
sha256sum stock_sky_fixture.rpx
