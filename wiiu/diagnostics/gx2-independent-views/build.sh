#!/usr/bin/env bash
set -euo pipefail
if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture=$(cd "$(dirname "$0")" && pwd)
source_tree=$(cd "$1" && pwd)
mkdir -p "$2"
cd "$2"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPRO/portlibs/wiiu/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"

python3 "$fixture/projection.py" "$source_tree/wiiu/gx2_terrain/src/lib.rs" > projection.rs
"${CAFE_RUSTC:-rustc}" --target powerpc-nintendo-cafe --edition 2021 --crate-type lib \
    -C panic=abort -C opt-level=2 --emit obj=projection.o projection.rs
powerpc-eabi-objcopy -R .note.GNU-stack projection.o
powerpc-eabi-cmake -S "$source_tree/wiiu/loading_host" -B native -G Ninja \
    -DCMAKE_BUILD_TYPE=Release "-DCMAKE_C_FLAGS=-I$wut/include" \
    "-DCMAKE_CXX_FLAGS=-I$wut/include" \
    "-DCAFEGLSL_COMPILER_EXECUTABLE=${CAFEGLSL_COMPILER_EXECUTABLE:-$(command -v glslcompiler)}"
cmake --build native --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-$(nproc)}"
powerpc-eabi-g++ -x c -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$source_tree/wiiu/loading_host" -I"$source_tree/wiiu/gx2_terrain" \
    -I"$source_tree/wiiu/gx2_hud" -I"$wut/include" -L"$wut/lib" \
    -specs="$wut/share/wut.specs" "$fixture/main.c" -x none projection.o \
    native/libhorizon_wiiu_loading_host.a native/gx2_hud/libhorizon_gx2_hud.a \
    native/gx2_terrain/libhorizon_gx2_terrain.a -lwut -o independent-views.elf
elf2rpl independent-views.elf independent-views.rpx
wuhbtool independent-views.rpx independent-views.wuhb --name="GX2 Independent Views"
sha256sum projection.rs projection.o native/terrain.gsh native/terrain_opaque.gsh native/hud.gsh \
    independent-views.elf independent-views.rpx independent-views.wuhb > hashes.txt
cat hashes.txt
