#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
    printf 'Usage: %s BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi

source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$1"
cd "$1"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

for shader in pattern sample; do
    "${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" -vs "$source_dir/reference.vert" \
        -ps "$source_dir/$shader.frag" -o "$shader.gsh"
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$shader.gsh" "${shader}_gfd.o"
done
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$wut/include" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$source_dir/main.c" pattern_gfd.o sample_gfd.o -lwut -o reference.elf
elf2rpl reference.elf HorizonGX2Reference.rpx
wuhbtool HorizonGX2Reference.rpx HorizonGX2Reference.wuhb \
    --name='Horizon GX2 Offscreen Reference' --short-name='GX2 Reference' --author='MCHorizonWiiU'
sha256sum HorizonGX2Reference.rpx HorizonGX2Reference.wuhb
