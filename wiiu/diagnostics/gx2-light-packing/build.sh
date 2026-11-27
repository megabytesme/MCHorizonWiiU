#!/usr/bin/env bash
set -euo pipefail
if (($# != 3)); then
    printf 'Usage: %s ASSEMBLED_SOURCE PINNED_CAFEGLSL_EXECUTABLE NEW_BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
engine=$(realpath "$1")
compiler=$(realpath "$2")
fixture=$(cd "$(dirname "$0")" && pwd)
terrain="$engine/wiiu/gx2_terrain"
readback="$fixture/../gx2-explicit-readback"
clear="$fixture/../gx2-solid-display-list/solid_list_color"
mkdir "$3"
cd "$3"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
printf '8268622fbaade63707d649c212e990ef871a4d1657450d03dc2ff85f8fa4ec90  %s\n' "$compiler" | sha256sum -c
python3 -B "$fixture/generate.py" "$terrain"
"$compiler" -v -vs stock.vert -ps stock.frag -o stock.gsh > stock.disasm 2>&1
printf 'c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133  stock.gsh\n' | sha256sum -c
"$compiler" -v -vs centered.vert -ps stock.frag -o centered.gsh > centered.disasm 2>&1
"$compiler" -v -vs centered-color.vert -ps centered-color.frag -o combined.gsh > combined.disasm 2>&1
"$compiler" -v -vs bad-alpha.vert -ps centered-color.frag -o bad_alpha.gsh > bad_alpha.disasm 2>&1
"$compiler" -v -vs "$clear.vert" -ps "$clear.frag" -o clear.gsh > clear.disasm 2>&1
for kind in stock centered combined bad_alpha clear; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$kind.gsh" "$kind.o"
done
wut="$DEVKITPRO/wut"
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$terrain" -I"$readback" -I"$wut/include" \
    -L"$wut/lib" -specs="$wut/share/wut.specs" "$fixture/main.c" "$terrain/terrain.c" \
    stock.o centered.o combined.o bad_alpha.o clear.o -lwut -lm -o gx2-light-packing.elf
elf2rpl gx2-light-packing.elf gx2-light-packing.rpx
readrpl -a gx2-light-packing.rpx > gx2-light-packing.readrpl
wuhbtool gx2-light-packing.rpx gx2-light-packing.wuhb \
    --name="GX2 Terrain Light Packing Diagnostic" --short-name="GX2 Light Packing" --author="Horizon"
python3 -B "$fixture/verify.py"
cc -std=c11 -O2 -Wall -Wextra -Werror "$fixture/oracle.c" -lm -o oracle
./oracle
sha256sum stock.gsh centered.gsh combined.gsh gx2-light-packing.rpx gx2-light-packing.wuhb
