#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
    printf 'Usage: %s BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi

fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$1"
cd "$1"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" -vs "$fixture/timing.vert" \
    -ps "$fixture/timing.frag" -o timing.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common timing.gsh timing_gfd.o
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$wut/include" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture/main.c" timing_gfd.o -lwut -o timing.elf
elf2rpl timing.elf HorizonGX2Timing.rpx
wuhbtool HorizonGX2Timing.rpx HorizonGX2Timing.wuhb \
    --name='Horizon GX2 Timing' --short-name='GX2 Timing' --author='MCHorizonWiiU'
sha256sum HorizonGX2Timing.rpx HorizonGX2Timing.wuhb
