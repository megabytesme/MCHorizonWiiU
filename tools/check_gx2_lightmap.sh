#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
    echo "usage: $0 HOSTED_STAGED_SOURCE FRESH_OUTPUT_DIR" >&2
    exit 2
fi
terrain=$(realpath "$1/wiiu/gx2_terrain")
test_source=$(realpath "$(dirname "$0")/../wiiu/gx2_terrain/tests/lightmap.c")
mkdir "$2"
cd "$2"
cat > CMakeLists.txt <<'CMAKE'
cmake_minimum_required(VERSION 3.20)
project(horizon_gx2_lightmap_test C)
add_subdirectory("${TERRAIN}" terrain)
find_program(PPC_OBJCOPY powerpc-eabi-objcopy REQUIRED)
find_program(ELF2RPL elf2rpl REQUIRED)
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/terrain_gsh.o"
    COMMAND "${CMAKE_COMMAND}" -E copy terrain/terrain.gsh terrain.gsh
    COMMAND "${PPC_OBJCOPY}" -I binary -O elf32-powerpc -B powerpc:common terrain.gsh terrain_gsh.o
    DEPENDS horizon_gx2_terrain_shader terrain/terrain.gsh VERBATIM)
set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/terrain_gsh.o"
    PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_executable(lightmap "${TEST_SOURCE}" "${CMAKE_CURRENT_BINARY_DIR}/terrain_gsh.o")
target_link_libraries(lightmap PRIVATE horizon_gx2_terrain wut)
target_compile_options(lightmap PRIVATE -Wall -Wextra -Werror)
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/gx2-lightmap.rpx"
    COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:lightmap>" lightmap.package.elf
    COMMAND "${PPC_OBJCOPY}" --strip-debug --remove-section=.note.GNU-stack lightmap.package.elf
    COMMAND "${ELF2RPL}" lightmap.package.elf gx2-lightmap.rpx
    DEPENDS lightmap VERBATIM)
add_custom_target(lightmap_rpx ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/gx2-lightmap.rpx")
CMAKE
powerpc-eabi-cmake -G Ninja -S . -B build -DCMAKE_BUILD_TYPE=Release \
    -DTERRAIN="$terrain" -DTEST_SOURCE="$test_source"
cmake --build build --parallel "$(nproc)"
sha256sum build/gx2-lightmap.rpx
