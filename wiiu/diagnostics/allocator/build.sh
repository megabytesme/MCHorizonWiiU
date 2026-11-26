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

powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$wut/include" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture/main.c" -lwut -o allocator.elf
elf2rpl allocator.elf HorizonAllocator.rpx
wuhbtool HorizonAllocator.rpx HorizonAllocator.wuhb \
    --name='Horizon Allocator Probe' --short-name='Allocator' --author='MCHorizonWiiU'
sha256sum HorizonAllocator.rpx HorizonAllocator.wuhb
