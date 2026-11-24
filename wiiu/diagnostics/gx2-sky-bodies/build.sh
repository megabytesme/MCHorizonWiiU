#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi

fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/main.c"
source_dir="$(realpath "$1")/wiiu"
mkdir -p "$2"
cd "$2"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

powerpc-eabi-cmake -S "$source_dir/loading_host" -B native -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_FLAGS="-I$wut/include" -DCMAKE_CXX_FLAGS="-I$wut/include"
cmake --build native --parallel "$(nproc)" --target horizon_wiiu_loading_host
powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -Wall -Wextra -Werror -I"$source_dir/gx2_hud" -I"$source_dir/loading_host" \
    -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture" \
    native/libhorizon_wiiu_loading_host.a native/gx2_hud/libhorizon_gx2_hud.a \
    -lwut -o celestial_fixture.elf
elf2rpl celestial_fixture.elf celestial_fixture.rpx
sha256sum celestial_fixture.rpx
