#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
    echo "Usage: $0 BUILD_DIRECTORY" >&2
    exit 2
fi
fixture=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
mkdir -p "$1"
cd "$1"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPPC/bin:$DEVKITPRO/tools/bin:$PATH"
wut="$DEVKITPRO/wut"

powerpc-eabi-gcc -std=gnu11 -O2 -mcpu=750 -meabi -mhard-float \
    -DESPRESSO -D__WIIU__ -D__WUT__ -Wall -Wextra -Werror -I"$wut/include" \
    -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture/main.c" -lwut -o netconf.elf
powerpc-eabi-objcopy --remove-section .note.GNU-stack netconf.elf netconf-rpl.elf
elf2rpl netconf-rpl.elf HorizonNetConf.rpx
wuhbtool HorizonNetConf.rpx HorizonNetConf.wuhb \
    --name='Horizon Network Resolver Probe' --short-name='Horizon DNS Probe' --author=MCHorizonWiiU
sha256sum HorizonNetConf.rpx HorizonNetConf.wuhb
