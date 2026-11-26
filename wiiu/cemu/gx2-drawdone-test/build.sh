#!/usr/bin/env bash
set -euo pipefail
if (( $# != 1 )); then
  printf 'Usage: %s OUTPUT_DIRECTORY\n' "$0" >&2
  exit 2
fi

fixture=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
mkdir -p "$1"
cd "$1"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

powerpc-eabi-gcc -std=c11 -mcpu=750 -meabi -mhard-float -O2 -Wall -Wextra -Werror \
  -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" \
  "$fixture/draw-done-probe.c" "$fixture/seeded-call.S" -lwut -o draw-done-probe.elf
powerpc-eabi-objcopy -R .note.GNU-stack draw-done-probe.elf draw-done-probe-stripped.elf
elf2rpl draw-done-probe-stripped.elf gx2-drawdone-abi.rpx
readrpl -i gx2-drawdone-abi.rpx | grep -E 'GX2(DrawDone|WaitTimeStamp|Get.*Submitted|Get.*Retired)'
sha256sum gx2-drawdone-abi.rpx
