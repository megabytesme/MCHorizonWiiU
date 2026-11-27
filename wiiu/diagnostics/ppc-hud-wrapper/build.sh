#!/usr/bin/env bash
set -euo pipefail

if (( $# != 3 )); then
    printf 'Usage: %s ORIGINAL_SOURCE REPAIRED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
    exit 2
fi
fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
before="$(realpath "$1")/wiiu"
after="$(realpath "$2")/wiiu"
mkdir "$3"
cd "$3"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-$DEVKITPRO/devkitPPC}"
export PATH="$DEVKITPRO/tools/bin:$DEVKITPPC/bin:$PATH"
wut="$DEVKITPRO/wut"
cflags=(-std=gnu11 -O2 -Wall -Wextra -Werror -mcpu=750 -meabi -mhard-float)

for arm in original repaired; do
    source="$before"
    if [[ $arm == repaired ]]; then source="$after"; fi
    powerpc-eabi-gcc "${cflags[@]}" -DESPRESSO -D__WIIU__ -D__WUT__ -ffunction-sections -fdata-sections \
        -DHORIZON_GX2_HUD_VIEW_LANES=1 -I"$source/gx2_hud" -I"$source/gx2_terrain" \
        -I"$source/loading_host" -I"$wut/include" -S "$source/loading_host/loading_host.c" -o "$arm-full.S"
    python3 -B "$fixture/extract.py" "$arm-full.S" "$arm" >"$arm.S"
    powerpc-eabi-gcc -mcpu=750 -meabi -mhard-float -c "$arm.S" -o "$arm.o"
done
powerpc-eabi-gcc "${cflags[@]}" -I"$wut/include" -c "$fixture/main.c" -o main.o
powerpc-eabi-gcc "${cflags[@]}" -B"$wut/share" -L"$wut/lib" -specs="$wut/share/wut.specs" \
    main.o original.o repaired.o -lwut -Wl,-Map=hud-wrapper-cpu.map -o hud-wrapper-cpu.elf
powerpc-eabi-nm hud-wrapper-cpu.elf >symbols.txt
if grep -E ' GX2| WHBGfx' symbols.txt; then exit 1; fi
elf2rpl hud-wrapper-cpu.elf hud-wrapper-cpu.rpx
sha256sum hud-wrapper-cpu.rpx
