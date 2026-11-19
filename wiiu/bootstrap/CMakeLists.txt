cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_bootstrap C)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(CARGO_WIIU_EXECUTABLE cargo-wiiu REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(READRPL_EXECUTABLE readrpl REQUIRED)
find_program(POWERPC_AR_EXECUTABLE powerpc-eabi-ar REQUIRED)
find_program(POWERPC_NM_EXECUTABLE powerpc-eabi-nm REQUIRED)
find_program(CAFEGLSL_COMPILER_EXECUTABLE glslcompiler REQUIRED)
set(HORIZON_WIIU_CAPTURE_DIR "" CACHE PATH "Horizon render-capture directory; select worldgen or the diagnostic fixture when empty")
set(HORIZON_WIIU_CAPTURE_MAX_VERTICES 65536 CACHE STRING "Maximum captured solid vertices converted for GX2")
option(HORIZON_WIIU_WORLDGEN "Use Horizon's Cafe-std world generator at runtime" OFF)
option(HORIZON_WIIU_DIAGNOSTIC_FIXTURE "Use the non-vanilla renderer bring-up fixture" OFF)
set(HORIZON_WIIU_WORLDGEN_STATICLIB "" CACHE FILEPATH "Cafe-std Horizon worldgen static library")
set(HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT 1 CACHE STRING "Use Horizon's fast flat generator (1) or full Overworld (0)")
set(HORIZON_WIIU_WORLD_REGION_RADIUS 1 CACHE STRING "Generated chunk region radius (0 or 1)")

if(HORIZON_WIIU_WORLDGEN AND HORIZON_WIIU_CAPTURE_DIR)
   message(FATAL_ERROR "Select either HORIZON_WIIU_WORLDGEN or HORIZON_WIIU_CAPTURE_DIR")
endif()
if(HORIZON_WIIU_DIAGNOSTIC_FIXTURE AND (HORIZON_WIIU_WORLDGEN OR HORIZON_WIIU_CAPTURE_DIR))
   message(FATAL_ERROR "Select either HORIZON_WIIU_DIAGNOSTIC_FIXTURE, HORIZON_WIIU_WORLDGEN, or HORIZON_WIIU_CAPTURE_DIR")
endif()
if(NOT HORIZON_WIIU_WORLDGEN AND NOT HORIZON_WIIU_DIAGNOSTIC_FIXTURE AND NOT HORIZON_WIIU_CAPTURE_DIR)
   message(FATAL_ERROR "Select Horizon worldgen or explicitly enable HORIZON_WIIU_DIAGNOSTIC_FIXTURE")
endif()
if(HORIZON_WIIU_WORLDGEN AND NOT EXISTS "${HORIZON_WIIU_WORLDGEN_STATICLIB}")
   message(FATAL_ERROR "HORIZON_WIIU_WORLDGEN_STATICLIB must name the Cafe worldgen archive")
endif()
if(HORIZON_WIIU_WORLDGEN AND NOT HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT MATCHES "^[01]$")
   message(FATAL_ERROR "HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT must be 0 or 1")
endif()
if(HORIZON_WIIU_WORLDGEN AND NOT HORIZON_WIIU_WORLD_REGION_RADIUS MATCHES "^[01]$")
   message(FATAL_ERROR "HORIZON_WIIU_WORLD_REGION_RADIUS must be 0 or 1")
endif()

set(RUST_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust-target")
set(RUST_TARGET_NAME "powerpc-cafe-nintendo")
set(RUST_STATICLIB "${RUST_TARGET_DIR}/${RUST_TARGET_NAME}/release/libhorizon_wiiu_bootstrap.a")
set(RUST_FEATURE_ARGS)
if(HORIZON_WIIU_WORLDGEN)
   set(RUST_FEATURE_ARGS --no-default-features --features wiiu-worldgen-render)
elseif(HORIZON_WIIU_DIAGNOSTIC_FIXTURE)
   set(RUST_FEATURE_ARGS --no-default-features --features diagnostic-fixture)
endif()

add_custom_command(
   OUTPUT "${RUST_STATICLIB}"
   COMMAND "${CMAKE_COMMAND}" -E env
      "CARGO_TARGET_DIR=${RUST_TARGET_DIR}"
      "${CARGO_EXECUTABLE}" +nightly build --release --lib ${RUST_FEATURE_ARGS}
         --manifest-path "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
   DEPENDS
      Cargo.toml
      rust-toolchain.toml
      .cargo/config.toml
      .cafe/powerpc-cafe-nintendo.json
      .cafe/linker.ld
      ../../crates/horizon-core/Cargo.toml
      ../../crates/horizon-core/src/lib.rs
      ../../crates/horizon-core/src/position.rs
      ../../crates/horizon-core/src/version.rs
      src/lib.rs
   WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
   COMMENT "Building the Rust CafeOS application library"
   VERBATIM
)
add_custom_target(horizon_wiiu_rust DEPENDS "${RUST_STATICLIB}")

add_library(horizon_wiiu_rustlib STATIC IMPORTED GLOBAL)
set_target_properties(horizon_wiiu_rustlib PROPERTIES IMPORTED_LOCATION "${RUST_STATICLIB}")

set(GSH "${CMAKE_CURRENT_BINARY_DIR}/horizon_block.gsh")
set(GSH_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/horizon_block_gsh.o")
add_custom_command(
   OUTPUT "${GSH}"
   COMMAND "${CAFEGLSL_COMPILER_EXECUTABLE}"
      -vs "${CMAKE_CURRENT_SOURCE_DIR}/shaders/block.vert"
      -ps "${CMAKE_CURRENT_SOURCE_DIR}/shaders/block.frag"
      -o "${GSH}"
   DEPENDS shaders/block.vert shaders/block.frag
   COMMENT "Compiling the GX2 block shaders"
   VERBATIM
)
add_custom_command(
   OUTPUT "${GSH_OBJECT}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}"
      -I binary -O elf32-powerpc -B powerpc:common
      horizon_block.gsh horizon_block_gsh.o
   DEPENDS "${GSH}"
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
   COMMENT "Embedding the GFD shader file in the RPX"
   VERBATIM
)
set_source_files_properties("${GSH_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_custom_target(horizon_wiiu_shaders DEPENDS "${GSH_OBJECT}")

set(HORIZON_WIIU_CAPTURE_SOURCE "")
if(HORIZON_WIIU_CAPTURE_DIR)
   find_package(Python3 REQUIRED COMPONENTS Interpreter)
   set(HORIZON_WIIU_CAPTURE_SOURCE "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_capture.c")
   add_custom_command(
      OUTPUT "${HORIZON_WIIU_CAPTURE_SOURCE}"
      COMMAND "${Python3_EXECUTABLE}"
         "${CMAKE_CURRENT_SOURCE_DIR}/../../tools/horizon_wiiu_capture.py"
         "${HORIZON_WIIU_CAPTURE_DIR}"
         --output "${HORIZON_WIIU_CAPTURE_SOURCE}"
         --max-vertices "${HORIZON_WIIU_CAPTURE_MAX_VERTICES}"
      DEPENDS
         "${CMAKE_CURRENT_SOURCE_DIR}/../../tools/horizon_wiiu_capture.py"
         "${HORIZON_WIIU_CAPTURE_DIR}/scene.json"
         "${HORIZON_WIIU_CAPTURE_DIR}/sections.json"
         "${HORIZON_WIIU_CAPTURE_DIR}/sections.bin"
         "${HORIZON_WIIU_CAPTURE_DIR}/sprites.bin"
         "${HORIZON_WIIU_CAPTURE_DIR}/lightmap.bin"
   COMMENT "Converting captured Horizon sections for GX2"
      VERBATIM
   )
endif()

set(WUT_CRT0_OBJECT "")
if(HORIZON_WIIU_WORLDGEN)
   set(WUT_CRT0_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/wut_crt0_rpx.o")
   set(WORLDGEN_WUT_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libhorizon_wiiu_worldgen_wut.a")
   add_custom_command(
      OUTPUT "${WUT_CRT0_OBJECT}" "${WORLDGEN_WUT_STATICLIB}"
      COMMAND "${CMAKE_COMMAND}" -E env
         "POWERPC_AR=${POWERPC_AR_EXECUTABLE}"
         "POWERPC_NM=${POWERPC_NM_EXECUTABLE}"
         "POWERPC_OBJCOPY=${POWERPC_OBJCOPY_EXECUTABLE}"
         bash "${CMAKE_CURRENT_SOURCE_DIR}/prepare_wut_startup.sh"
         "${HORIZON_WIIU_WORLDGEN_STATICLIB}" "$ENV{DEVKITPRO}/wut/lib/libwut.a"
         "${WORLDGEN_WUT_STATICLIB}" "${WUT_CRT0_OBJECT}"
      DEPENDS "${HORIZON_WIIU_WORLDGEN_STATICLIB}" "$ENV{DEVKITPRO}/wut/lib/libwut.a" prepare_wut_startup.sh
      COMMENT "Selecting WUT startup for the Cafe std archive"
      VERBATIM)
   set_source_files_properties("${WUT_CRT0_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
   add_custom_target(horizon_wiiu_wut_startup DEPENDS "${WUT_CRT0_OBJECT}" "${WORLDGEN_WUT_STATICLIB}")
endif()
add_executable(horizon_wiiu_bootstrap "${WUT_CRT0_OBJECT}" src/main.c ../platform/src/vpad_bridge.c "${GSH_OBJECT}" "${HORIZON_WIIU_CAPTURE_SOURCE}")
target_include_directories(horizon_wiiu_bootstrap PRIVATE ../platform/src)
if(HORIZON_WIIU_CAPTURE_SOURCE)
   target_compile_definitions(horizon_wiiu_bootstrap PRIVATE HORIZON_WIIU_CAPTURE=1)
endif()
if(HORIZON_WIIU_WORLDGEN)
   add_library(horizon_wiiu_worldgen STATIC IMPORTED GLOBAL)
   set_target_properties(horizon_wiiu_worldgen PROPERTIES IMPORTED_LOCATION "${WORLDGEN_WUT_STATICLIB}")
   target_compile_definitions(horizon_wiiu_bootstrap PRIVATE
      HORIZON_WIIU_WORLDGEN=1
      HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT=${HORIZON_WIIU_WORLD_PROFILE_SUPERFLAT}
      HORIZON_WIIU_WORLD_REGION_RADIUS=${HORIZON_WIIU_WORLD_REGION_RADIUS})
   target_link_options(horizon_wiiu_bootstrap PRIVATE "LINKER:--defsym=__end__=ADDR(.bss)+SIZEOF(.bss)")
   add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_wut_startup)
endif()
add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_rust)
add_dependencies(horizon_wiiu_bootstrap horizon_wiiu_shaders)
target_link_libraries(horizon_wiiu_bootstrap PRIVATE horizon_wiiu_rustlib)
if(HORIZON_WIIU_WORLDGEN)
   target_link_libraries(horizon_wiiu_bootstrap PRIVATE horizon_wiiu_worldgen)
endif()

set(PACKAGE_ELF "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.package.elf")
set(RPX "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.rpx")
set(WUHB "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_bootstrap.wuhb")

# GCC 16 emits a zero-sized .note.GNU-stack section in its startup object.
# Both elf2rpl and cargo-wiiu 0.4.1 currently reject it, so remove this
# link-time-only marker from a copy before converting the executable to RPX.
# WUT's __rplwrap_exit names the coreinit export exit. Normalize that alias
# before cargo-wiiu conversion and reject any other unresolved WUT wrappers.
add_custom_command(
   OUTPUT "${RPX}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_wiiu_bootstrap>" "${PACKAGE_ELF}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack
      --redefine-sym __rplwrap_exit=exit "${PACKAGE_ELF}"
   COMMAND "${CMAKE_COMMAND}" "-DCARGO=${CARGO_EXECUTABLE}" "-DPACKAGE_ELF=${PACKAGE_ELF}"
      "-DREADRPL=${READRPL_EXECUTABLE}" "-DRPX=${RPX}"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
   DEPENDS horizon_wiiu_bootstrap "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
      "${CARGO_EXECUTABLE}" "${CARGO_WIIU_EXECUTABLE}" "${POWERPC_OBJCOPY_EXECUTABLE}" "${READRPL_EXECUTABLE}"
   COMMENT "Packaging CafeOS ELF as RPX"
   VERBATIM
)
add_custom_target(horizon_wiiu_rpx ALL DEPENDS "${RPX}")

add_custom_command(
   OUTPUT "${WUHB}"
   COMMAND "${CARGO_EXECUTABLE}" wiiu wuhb --name "Horizon Wii U Bootstrap" "${RPX}" "${WUHB}"
   DEPENDS "${RPX}"
   COMMENT "Packaging RPX as WUHB"
   VERBATIM
)
add_custom_target(horizon_wiiu_package ALL DEPENDS "${WUHB}")
