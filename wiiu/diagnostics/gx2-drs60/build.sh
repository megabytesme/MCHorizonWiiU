#!/usr/bin/env bash
set -euo pipefail
if (( $# != 3 )); then
    echo "Usage: $0 ASSEMBLED_SOURCE GPU_TIMING_FIXTURE BUILD_DIRECTORY" >&2
    exit 2
fi
probe=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source_tree=$(cd "$1" && pwd)
timing=$(cd "$2" && pwd)
mkdir -p "$3"
cd "$3"

wut="${DEVKITPRO:-/opt/devkitpro}/wut"
cafe_rustc="${HORIZON_WIIU_STD_RUSTC:?Set HORIZON_WIIU_STD_RUSTC to the pinned Cafe compiler}"
cp "$probe/drs_policy.rs" .
cp "$source_tree/crates/horizon-client-render/src/dynamic_resolution.rs" .
python3 - "$source_tree" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1])
performance = (source / "crates/horizon-client/src/platform/cafe_performance.rs").read_text()
graphics = (source / "crates/horizon-client-render/src/graphics.rs").read_text()

def block(text, start, marker):
    first = text.index(start)
    return text[first:text.index(marker, first)].rstrip()

frequency = block(performance, "const GX2_GPU_HZ:", "\n")
sample = block(performance, "#[repr(C, align(8))]", "\nstruct GpuTimingObservation")
work = block(performance, "#[derive(Clone, Copy, Default)]\npub struct CafeFrameWork", "\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
resolution = block(graphics, "pub struct WorldResolution {", "\n}")
Path("cafe_drs_exact.rs").write_text(
    f"{frequency}\n{sample}\n#[derive(Clone, Copy)]\n{resolution}\n}}\n{work}\n"
)
PY
"${HOST_RUSTC:-rustc}" --edition 2024 --test drs_policy.rs -o policy-tests
./policy-tests | tee host-tests.txt
"$cafe_rustc" --edition 2024 --target powerpc-nintendo-cafe --crate-type staticlib \
    -C panic=abort -C opt-level=3 drs_policy.rs -o libdrs_policy.a
# WUT provides the C entry point. Drop the Cafe Rust executable entry section.
powerpc-eabi-objcopy --remove-section .crt0 libdrs_policy.a libdrs_policy_wut.a

"${CAFEGLSL_COMPILER_EXECUTABLE:-glslcompiler}" -vs "$timing/timing.vert" \
    -ps "$timing/timing.frag" -o timing.gsh
powerpc-eabi-objcopy -I binary -O elf32-powerpc -B powerpc:common timing.gsh timing_gfd.o
powerpc-eabi-gcc -std=gnu11 -mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__ \
    -O2 -Wall -Wextra -Werror -I"$wut/include" -I"$timing" \
    -I"$source_tree/wiiu/loading_host" -L"$wut/lib" -specs="$wut/share/wut.specs" -Wl,-Map,drs-probe.map \
    "$probe/drs_probe.c" timing_gfd.o libdrs_policy_wut.a -lwut -o drs-probe.elf
# Current devkitPPC includes this ELF metadata, which elf2rpl cannot represent.
powerpc-eabi-objcopy --strip-debug --remove-section .note.GNU-stack --redefine-sym __rplwrap_exit=exit drs-probe.elf drs-probe-rpl.elf
cargo wiiu rpx drs-probe-rpl.elf HorizonGX2DRS60LinkOrder.rpx
python3 -B "$probe/import-markers.py" HorizonGX2DRS60LinkOrder.rpx
wuhbtool HorizonGX2DRS60LinkOrder.rpx HorizonGX2DRS60LinkOrder.wuhb \
    --name='Horizon GX2 DRS 60 Link Check' --short-name='GX2 DRS Link' --author=MCHorizonWiiU
sha256sum HorizonGX2DRS60LinkOrder.rpx HorizonGX2DRS60LinkOrder.wuhb | tee hashes.txt
