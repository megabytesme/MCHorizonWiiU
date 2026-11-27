#!/usr/bin/env bash
set -euo pipefail
if (($# != 3)); then
    printf 'Usage: %s ASSEMBLED_ENGINE PINNED_CAFEGLSL NEW_BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
engine=$(realpath "$1")
compiler=$(realpath "$2")
fixture=$(cd "$(dirname "$0")" && pwd)
mkdir "$3"
cd "$3"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
terrain="$engine/wiiu/gx2_terrain"
readback="$fixture/../gx2-explicit-readback"
clear="$fixture/../gx2-solid-display-list/solid_list_color"
cp "$terrain/terrain.vert" stock.vert
cp "$terrain/terrain.frag" stock.frag
python3 -B "$fixture/generate.py" stock.vert half.vert
sha256sum -c <<< "8268622fbaade63707d649c212e990ef871a4d1657450d03dc2ff85f8fa4ec90  $compiler"
"$compiler" -v -vs stock.vert -ps stock.frag -o stock.gsh > stock.disasm 2>&1
"$compiler" -v -vs half.vert -ps stock.frag -o half.gsh > half.disasm 2>&1
"$compiler" -v -vs "$clear.vert" -ps "$clear.frag" -o clear.gsh > clear.disasm 2>&1
sha256sum -c <<'SUMS'
c07d3b7557e55341a3bfcd3e36cf2dbd048f8473514a8ac26d400878503f7133  stock.gsh
32019e4e5410477107ef824cc0df045e20e5a27f803d6c355f42764860701ad2  half.gsh
SUMS
for kind in stock half clear; do
    powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common "$kind.gsh" "$kind.o"
done
wut="$DEVKITPRO/wut"
for arm in pair scalar; do
    flags=()
    if [[ "$arm" == scalar ]]; then flags=(-DDRAW_SCALAR20); fi
    output="gx2-half20-$arm"
    powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
        -O2 -Wall -Wextra -Werror -I"$terrain" -I"$readback" -I"$wut/include" \
        -L"$wut/lib" -specs="$wut/share/wut.specs" "${flags[@]}" "$fixture/main.c" "$terrain/terrain.c" \
        stock.o half.o clear.o -lwut -lm -o "$output.elf"
    elf2rpl "$output.elf" "$output.rpx"
    readrpl -h -S -f -i "$output.rpx" > "$output.readrpl"
    sha256sum "$output.rpx"
done
cc -std=c11 -O2 -Wall -Wextra -Werror "$fixture/oracle.c" -o oracle
./oracle
