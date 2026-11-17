#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
   echo "Usage: $0 OUTPUT_DIRECTORY" >&2
   exit 2
fi
: "${DEVKITPRO:?Set DEVKITPRO to your devkitPro installation}"
: "${DEVKITPPC:?Set DEVKITPPC to your devkitPPC installation}"
src=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$1"
out=$(cd "$1" && pwd)
if [ -e "$out/ppc_carry.elf" ] || [ -L "$out/ppc_carry.elf" ] ||
   [ -e "$out/ppc_carry.rpx" ] || [ -L "$out/ppc_carry.rpx" ]; then
   echo "Output already exists in $out; choose a fresh directory" >&2
   exit 1
fi
"$DEVKITPPC/bin/powerpc-eabi-gcc" -O2 -mcpu=750 -meabi -mhard-float \
   -I"$DEVKITPRO/wut/include" -specs="$DEVKITPRO/wut/share/wut.specs" \
   "$src/main.c" -L"$DEVKITPRO/wut/lib" -lwut -o "$out/ppc_carry.elf"
"$DEVKITPRO/tools/bin/elf2rpl" "$out/ppc_carry.elf" "$out/ppc_carry.rpx"
