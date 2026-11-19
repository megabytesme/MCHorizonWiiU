#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 8 ]; then
   echo "Usage: $0 NEW_BUILD_DIR GAME_ARCHIVE TV_SIZE_C ARGS_OBJECT SESSION_LOCK_OBJECT PROCESS_ID_SYMBOL GLSL_COMPILER DISABLE_AX_AUDIO_ON_OR_OFF" >&2
   exit 2
fi
: "${DEVKITPRO:?Set DEVKITPRO to your devkitPro installation}"
: "${DEVKITPPC:?Set DEVKITPPC to your devkitPPC installation}"
src=$(cd "$(dirname "$0")" && pwd)
wiiu=$(cd "$src/../.." && pwd)
archive=$(realpath -e "$2")
tv=$(realpath -e "$3")
args=$(realpath -e "$4")
session=$(realpath -e "$5")
symbol=$6
glsl=$(realpath -e "$7")
audio=$8
[[ "$symbol" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]] || { echo 'Invalid process ID symbol' >&2; exit 2; }
case "$audio" in ON|OFF) ;; *) echo 'DISABLE_AX_AUDIO must be ON or OFF' >&2; exit 2;; esac
build=$(realpath -m "$1")
for path in "$src" "$build" "$archive" "$tv" "$args" "$session" "$glsl" "$DEVKITPRO" "$DEVKITPPC"; do
   case "$path" in *[[:space:],]*) echo "Paths containing spaces or commas are unsupported: $path" >&2; exit 2;; esac
done
mkdir "$build" # Refuse an existing directory and never replace earlier outputs.
"$DEVKITPPC/bin/powerpc-eabi-gcc" -O2 -mcpu=750 -meabi -mhard-float \
   -I"$DEVKITPRO/wut/include" -c "$src/pool.c" -o "$build/pool.o"
flags="-L$DEVKITPRO/wut/lib -L$DEVKITPRO/portlibs/wiiu/lib -L$DEVKITPRO/portlibs/ppc/lib"
flags="$flags -Wl,--wrap=OSGetArgcArgv -Wl,$args -Wl,--wrap=$symbol -Wl,$session"
flags="$flags -Wl,--wrap=NSSInit -Wl,--wrap=NSSGetRandom -Wl,$build/pool.o"
"$DEVKITPRO/portlibs/wiiu/bin/powerpc-eabi-cmake" -S "$wiiu/loading_app" -B "$build" -G Ninja \
   -DCMAKE_BUILD_TYPE=Release -DHORIZON_WIIU_PREBUILT_STATICLIB="$archive" \
   -DHORIZON_WIIU_WINIT_TV_SIZE_SOURCE="$tv" -DHORIZON_WIIU_CEMU_GLOBAL_SESSION_LOCK=ON \
   -DHORIZON_WIIU_DISABLE_AX_AUDIO="$audio" -DCAFEGLSL_COMPILER_EXECUTABLE="$glsl" \
   -DCMAKE_EXE_LINKER_FLAGS="$flags"
cmake --build "$build" --parallel "$(nproc)"
