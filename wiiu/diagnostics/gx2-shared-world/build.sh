#!/usr/bin/env bash
set -euo pipefail

if (( $# < 2 || $# > 3 )) || { (( $# == 3 )) && [[ "$3" != --prehand ]]; }; then
    printf 'Usage: %s PATCHED_ASSEMBLED_SOURCE BUILD_DIRECTORY [--prehand]\n' "$0" >&2
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

test_source=main.c
output=shared_world_fixture
define=()
link_options=()
terrain_library=()
if (( $# == 3 )); then
    test_source=prehand.c
    output=shared_world_prehand
    define=(-DHORIZON_GX2_PREHAND_DIAGNOSTIC)
    link_options=(-Wl,--wrap=GX2DrawDone)
    terrain_library=(native/gx2_terrain/libhorizon_gx2_terrain.a)
fi

powerpc-eabi-cmake -S "$source_tree/wiiu/loading_host" -B native -G Ninja \
    "-DCMAKE_C_FLAGS=-I$wut/include ${define[*]}" "-DCMAKE_CXX_FLAGS=-I$wut/include"
cmake --build native --parallel "$(nproc)"
powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    "${define[@]}" -Wall -Wextra -Werror \
    -I"$source_tree/wiiu/loading_host" -I"$source_tree/wiiu/gx2_hud" -I"$source_tree/wiiu/gx2_terrain" \
    -I"$wut/include" -L"$wut/lib" "${link_options[@]}" \
    -specs="$wut/share/wut.specs" "$fixture/$test_source" \
    native/libhorizon_wiiu_loading_host.a native/gx2_hud/libhorizon_gx2_hud.a \
    "${terrain_library[@]}" -lwut -o "$output.elf"
elf2rpl "$output.elf" "$output.rpx"
sha256sum "$output.rpx"
