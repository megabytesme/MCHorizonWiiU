#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
    printf 'Usage: %s ASSEMBLED_SOURCE BUILD_DIRECTORY\n' "$0" >&2
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

# Build the comparison against the actual implementation in the supplied source.
python3 - "$source_tree" <<'PY'
from pathlib import Path
import sys

source = (Path(sys.argv[1]) / "wiiu/loading_host/loading_host.c").read_text()
start = source.index("/* Match the verified diagnostic copy shape")
end = source.index("\n}\n", start) + len("\n}\n")
Path("baseline.inc").write_text(source[start:end] + "\n", encoding="utf-8")
PY
"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" -vs "$fixture/reference.vert" \
    -ps "$fixture/pattern.frag" -o pattern.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common pattern.gsh pattern_gfd.o
for arm in stage-only cemu-ab; do
    flags=()
    if [[ "$arm" == cemu-ab ]]; then flags+=(-DCEMU_BASELINE); fi
    powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
        -O2 -Wall -Wextra -Werror -I. -I"$wut/include" -L"$wut/lib" \
        -specs="$wut/share/wut.specs" "${flags[@]}" "$fixture/main.c" pattern_gfd.o -lwut -o "$arm.elf"
    elf2rpl "$arm.elf" "$arm.rpx"
done
sha256sum baseline.inc pattern.gsh stage-only.rpx cemu-ab.rpx | tee hashes.txt
