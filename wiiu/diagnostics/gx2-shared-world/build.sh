#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s PATCHED_ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source_tree=$(cd "$1" && pwd)
mkdir -p "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

powerpc-eabi-cmake -S "$source_tree/wiiu/loading_host" -B native -G Ninja \
    "-DCMAKE_C_FLAGS=-I$wut/include" "-DCMAKE_CXX_FLAGS=-I$wut/include"
cmake --build native --parallel "$(nproc)"
powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$source_tree/wiiu/loading_host" -I"$source_tree/wiiu/gx2_hud" \
    -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture/main.c" \
    native/libhorizon_wiiu_loading_host.a native/gx2_hud/libhorizon_gx2_hud.a \
    -lwut -o shared_world_fixture.elf
elf2rpl shared_world_fixture.elf shared_world_fixture.rpx
sha256sum shared_world_fixture.rpx
