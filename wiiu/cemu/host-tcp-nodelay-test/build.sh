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
  -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture/probe.c" -lwut -o probe.elf
powerpc-eabi-objcopy -R .note.GNU-stack probe.elf probe-stripped.elf
elf2rpl probe-stripped.elf tcp-nodelay.rpx
sha256sum tcp-nodelay.rpx
