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

"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" -vs "$fixture/commands.vert" \
    -ps "$fixture/commands.frag" -o commands.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common commands.gsh commands_gfd.o
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$wut/include" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture/main.c" commands_gfd.o -lwut -o commands.elf
elf2rpl commands.elf HorizonGX2DisplayLists.rpx
wuhbtool HorizonGX2DisplayLists.rpx HorizonGX2DisplayLists.wuhb \
    --name='Horizon GX2 Display Lists' --short-name='GX2 Display Lists' --author='MCHorizonWiiU'
sha256sum HorizonGX2DisplayLists.rpx HorizonGX2DisplayLists.wuhb
