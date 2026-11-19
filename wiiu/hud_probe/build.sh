#!/usr/bin/env bash
set -euo pipefail
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
build_dir="${HORIZON_WIIU_HUD_PROBE_BUILD_DIR:-${TMPDIR:-/tmp}/horizonwiiu-hud-probe}"
if [[ -z "${HORIZON_WIIU_STD_RUSTC:-}" || ! -x "${HORIZON_WIIU_STD_RUSTC}" ]]; then
   printf 'Set HORIZON_WIIU_STD_RUSTC to the Cafe stage-1 rustc.\n' >&2
   exit 1
fi
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-${DEVKITPRO}/devkitPPC}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/portlibs/wiiu/bin:${DEVKITPPC}/bin:${PATH}"
if [[ -n "${CAFEGLSL_ROOT:-}" ]]; then export PATH="${CAFEGLSL_ROOT}/bin:${PATH}"; fi
mkdir -p "${build_dir}"
RUSTFLAGS='--cfg getrandom_backend="custom"' \
CARGO_TARGET_DIR="${build_dir}/rust-target" RUSTC="${HORIZON_WIIU_STD_RUSTC}" \
   cargo +nightly build --release --lib --manifest-path "${source_dir}/Cargo.toml" \
      --target powerpc-nintendo-cafe --jobs "${HORIZON_WIIU_JOBS:-$(nproc)}"
powerpc-eabi-cmake -S "${source_dir}" -B "${build_dir}/cmake" -G Ninja \
   -DCMAKE_BUILD_TYPE=Release \
   -DHORIZON_HUD_STATICLIB="${build_dir}/rust-target/powerpc-nintendo-cafe/release/libhorizon_wiiu_hud_probe.a"
cmake --build "${build_dir}/cmake" --parallel "${HORIZON_WIIU_JOBS:-$(nproc)}"
printf 'RPX: %s\n' "${build_dir}/cmake/horizon_wiiu_hud_probe.rpx"
