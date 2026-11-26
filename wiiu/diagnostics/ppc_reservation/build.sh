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
    "$fixture/main.c" "$fixture/patterns.S" -lwut -o ppc_reservation.elf
powerpc-eabi-objcopy -R .note.GNU-stack ppc_reservation.elf ppc_reservation-stripped.elf
elf2rpl ppc_reservation-stripped.elf ppc_reservation.rpx
sha256sum ppc_reservation.rpx
