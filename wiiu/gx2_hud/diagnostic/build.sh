#!/usr/bin/env bash
set -euo pipefail
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
build_dir="${HORIZON_WIIU_HUD_DIAG_BUILD_DIR:-${TMPDIR:-/tmp}/horizonwiiu-hud-diagnostic}"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-${DEVKITPRO}/devkitPPC}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/portlibs/wiiu/bin:${DEVKITPPC}/bin:${PATH}"
if [[ -n "${CAFEGLSL_ROOT:-}" ]]; then export PATH="${CAFEGLSL_ROOT}/bin:${PATH}"; fi
powerpc-eabi-cmake -S "${source_dir}" -B "${build_dir}" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "${build_dir}" --parallel "$(nproc)"
