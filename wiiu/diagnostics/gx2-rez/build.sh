#!/usr/bin/env bash
set -euo pipefail
if (($# != 3)); then
    echo "Usage: $0 ASSEMBLED_SOURCE CAFEGLSL BUILD_DIRECTORY" >&2
    exit 2
fi
here=$(cd "$(dirname "$0")" && pwd)
shader="$(realpath "$1")/wiiu/gx2_terrain"
compiler=$(realpath "$2")
patcher=$here/patch_descriptor.py
echo "8268622fbaade63707d649c212e990ef871a4d1657450d03dc2ff85f8fa4ec90  $compiler" | sha256sum -c
mkdir "$3"
cd "$3"
sed '1a #define HORIZON_TERRAIN_OPAQUE 1' "$shader/terrain.frag" > opaque.frag
compile_shader() {
    "$compiler" -vs "$shader/terrain.vert" -ps "$1" -o "$2.gsh" -v > "$2.disasm" 2>&1
}
compile_shader "$shader/terrain.frag" terrain
compile_shader opaque.frag opaque
compile_shader "$here/clear.frag" clear
# Patch the stock GFD only at checked DB_SHADER_CONTROL bits.
python3 -B "$patcher"
echo "c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133  terrain.gsh" | sha256sum -c
echo "55702fd7cc26dbb3b61d303093cfb68b9387e2ae551ee69a81a7d5f2bc5d3e53  terrain_rez.gsh" | sha256sum -c
echo "f1f3c06afd7790c23acdc62801e2212620a3488da2367631bde3c5ba0f0ad52f  opaque.gsh" | sha256sum -c
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"
for name in terrain terrain_rez opaque clear; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$name.gsh" "$name.o"
done
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -I"$shader" -I"$wut/include" -L"$wut/lib" -specs="$wut/share/wut.specs" \
    "$here/main.c" "$shader/terrain.c" terrain.o terrain_rez.o opaque.o clear.o -lwut -lm -o gx2-rez.elf
elf2rpl gx2-rez.elf gx2-rez.rpx
readrpl -a gx2-rez.rpx > gx2-rez-readrpl.log
wuhbtool gx2-rez.rpx gx2-rez.wuhb --name "GX2 Re-Z Diagnostic" --short-name "GX2 Re-Z" --author "HorizonWiiU"
sha256sum *.gsh gx2-rez.rpx gx2-rez.wuhb
