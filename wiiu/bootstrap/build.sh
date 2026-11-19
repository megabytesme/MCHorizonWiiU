#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BUILD_DIR="${HORIZON_WIIU_BUILD_DIR:-${TMPDIR:-/tmp}/horizonwiiu-bootstrap}"
WORLDGEN_ENABLED="${HORIZON_WIIU_WORLDGEN:-1}"
DIAGNOSTIC_FIXTURE_ENABLED="${HORIZON_WIIU_DIAGNOSTIC_FIXTURE:-0}"
if [[ -n "${HORIZON_WIIU_CAPTURE_DIR:-}" && -z "${HORIZON_WIIU_WORLDGEN+x}" ]]; then
   WORLDGEN_ENABLED=0
fi
if [[ "${WORLDGEN_ENABLED}" != "0" && "${WORLDGEN_ENABLED}" != "1" ]]; then
   printf 'HORIZON_WIIU_WORLDGEN must be 0 or 1.\n' >&2
   exit 1
fi
if [[ "${DIAGNOSTIC_FIXTURE_ENABLED}" != "0" && "${DIAGNOSTIC_FIXTURE_ENABLED}" != "1" ]]; then
   printf 'HORIZON_WIIU_DIAGNOSTIC_FIXTURE must be 0 or 1.\n' >&2
   exit 1
fi
if [[ "${WORLDGEN_ENABLED}" == "1" && "${DIAGNOSTIC_FIXTURE_ENABLED}" == "1" ]]; then
   printf 'Select either Horizon worldgen or the diagnostic fixture, not both.\n' >&2
   exit 1
fi
if [[ -n "${HORIZON_WIIU_CAPTURE_DIR:-}" && "${WORLDGEN_ENABLED}" == "1" ]]; then
   printf 'Select either Horizon worldgen or a captured Horizon scene, not both.\n' >&2
   exit 1
fi
if [[ -z "${HORIZON_WIIU_CAPTURE_DIR:-}" && "${WORLDGEN_ENABLED}" == "0" && "${DIAGNOSTIC_FIXTURE_ENABLED}" == "0" ]]; then
   printf 'Select Horizon worldgen (default) or explicitly set HORIZON_WIIU_DIAGNOSTIC_FIXTURE=1.\n' >&2
   exit 1
fi

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-${DEVKITPRO}/devkitPPC}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/portlibs/wiiu/bin:${DEVKITPPC}/bin:${PATH}"

if [[ ! -f "${SCRIPT_DIR}/.cafe/powerpc-cafe-nintendo.json" ]]; then
   git -C "${REPO_ROOT}" submodule update --init --recursive -- wiiu/bootstrap/.cafe
fi

mkdir -p "${BUILD_DIR}"

WORLDGEN_STATICLIB=""
if [[ "${WORLDGEN_ENABLED}" == "1" ]]; then
   if [[ -z "${HORIZON_WIIU_STD_RUSTC:-}" || ! -x "${HORIZON_WIIU_STD_RUSTC}" ]]; then
      printf 'Set HORIZON_WIIU_STD_RUSTC to the Cafe stage-1 rustc built from rust-wiiu/rust.\n' >&2
      exit 1
   fi
   WORLDGEN_CARGO_TARGET_DIR="${HORIZON_WIIU_WORLDGEN_TARGET_DIR:-${BUILD_DIR}/worldgen-rust-target}"
   (
      cd "${SCRIPT_DIR}/../worldgen"
      CARGO_TARGET_DIR="${WORLDGEN_CARGO_TARGET_DIR}" \
         RUSTC="${HORIZON_WIIU_STD_RUSTC}" \
         cargo +nightly build --release --lib --features cafe --jobs "$(nproc)" \
            --manifest-path "${SCRIPT_DIR}/../worldgen/Cargo.toml" \
            --target powerpc-nintendo-cafe
   )
   WORLDGEN_STATICLIB="${WORLDGEN_CARGO_TARGET_DIR}/powerpc-nintendo-cafe/release/libhorizon_wiiu_worldgen.a"
   if [[ ! -f "${WORLDGEN_STATICLIB}" ]]; then
      printf 'Cafe worldgen static library was not produced: %s\n' "${WORLDGEN_STATICLIB}" >&2
      exit 1
   fi
fi

if [[ -n "${CAFEGLSL_ROOT:-}" ]]; then
   if [[ ! -x "${CAFEGLSL_ROOT}/bin/glslcompiler" ]]; then
      printf 'CAFEGLSL_ROOT is set but bin/glslcompiler is missing: %s\n' "${CAFEGLSL_ROOT}" >&2
      exit 1
   fi
else
   CAFEGLSL_VERSION="1.0.0-rc1"
   CAFEGLSL_ROOT="${BUILD_DIR}/deps/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64"
   if [[ ! -x "${CAFEGLSL_ROOT}/bin/glslcompiler" ]]; then
      CAFEGLSL_ARCHIVE="${BUILD_DIR}/deps/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64.tar.xz"
      mkdir -p "${BUILD_DIR}/deps"
      curl -fsSL -o "${CAFEGLSL_ARCHIVE}" \
         "https://github.com/Exzap/CafeGLSL/releases/download/v${CAFEGLSL_VERSION}/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64.tar.xz"
      printf '%s  %s\n' \
         'fdbd561539080f1db74ec3f248edee4e1988d329a4d20f2273c72adf55015f97' \
         "${CAFEGLSL_ARCHIVE}" | sha256sum --check
      tar -xJf "${CAFEGLSL_ARCHIVE}" -C "${BUILD_DIR}/deps"
   fi
fi

export PATH="${CAFEGLSL_ROOT}/bin:${PATH}"
LINKER_FLAGS_ARGS=()
if [[ -n "${HORIZON_WIIU_LINKER_FLAGS:-}" ]]; then
   LINKER_FLAGS_ARGS+=("-DCMAKE_EXE_LINKER_FLAGS=${HORIZON_WIIU_LINKER_FLAGS}")
fi
powerpc-eabi-cmake -S "${SCRIPT_DIR}" -B "${BUILD_DIR}" -G Ninja \
   -DCMAKE_BUILD_TYPE=Release \
   -DHORIZON_WIIU_CAPTURE_DIR="${HORIZON_WIIU_CAPTURE_DIR:-}" \
   -DHORIZON_WIIU_CAPTURE_MAX_VERTICES="${HORIZON_WIIU_CAPTURE_MAX_VERTICES:-65536}" \
   -DHORIZON_WIIU_WORLDGEN="$([[ "${WORLDGEN_ENABLED}" == "1" ]] && echo ON || echo OFF)" \
   -DHORIZON_WIIU_DIAGNOSTIC_FIXTURE="$([[ "${DIAGNOSTIC_FIXTURE_ENABLED}" == "1" ]] && echo ON || echo OFF)" \
   -DHORIZON_WIIU_WORLDGEN_STATICLIB="${WORLDGEN_STATICLIB}" \
   -DHORIZON_WIIU_WORLD_PROFILE_SUPERFLAT="${HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT:-1}" \
   -DHORIZON_WIIU_WORLD_REGION_RADIUS="${HORIZON_WIIU_WORLD_REGION_RADIUS:-1}" \
   -DCAFEGLSL_COMPILER_EXECUTABLE="${CAFEGLSL_ROOT}/bin/glslcompiler" \
   "${LINKER_FLAGS_ARGS[@]}"
cmake --build "${BUILD_DIR}" --parallel "$(nproc)"

printf 'RPX: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.rpx"
printf 'WUHB: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.wuhb"
