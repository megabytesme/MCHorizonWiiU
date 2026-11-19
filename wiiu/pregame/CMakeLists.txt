cmake_minimum_required(VERSION 3.20)
project(horizon_wiiu_pregame C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

set(CAFE_RUSTC "$ENV{HORIZON_WIIU_STD_RUSTC}" CACHE FILEPATH "Cafe stage1 Rust compiler")
if(NOT EXISTS "${CAFE_RUSTC}")
   message(FATAL_ERROR "Set HORIZON_WIIU_STD_RUSTC to a stage1 compiler with powerpc-nintendo-cafe core")
endif()
find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(CARGO_WIIU_EXECUTABLE cargo-wiiu REQUIRED)
find_program(OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(READRPL_EXECUTABLE readrpl REQUIRED)
find_program(WUHBTOOL_EXECUTABLE wuhbtool REQUIRED)
add_subdirectory(../gx2_hud gx2_hud)

set(package_name HorizonLauncher-Night)
set(app_title "Horizon Wii U")
set(theme_dir "${CMAKE_CURRENT_SOURCE_DIR}/themes/night")
file(GLOB_RECURSE theme_inputs CONFIGURE_DEPENDS "${theme_dir}/*")
file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" launcher_version REGEX "^version = ")
string(REGEX REPLACE ".*\"([0-9.]+)\"" "\\1" launcher_version "${launcher_version}")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS Cargo.toml)

set(shell_object "${CMAKE_CURRENT_BINARY_DIR}/shell.o")
add_custom_command(OUTPUT "${shell_object}"
   COMMAND "${CMAKE_COMMAND}" -E env  "CARGO_PKG_VERSION=${launcher_version}" "${CAFE_RUSTC}" --target powerpc-nintendo-cafe --edition 2021
      --crate-name horizon_wiiu_pregame --crate-type lib --emit=obj --cfg horizon_wiiu_cafe
      -C opt-level=s -C panic=abort -C codegen-units=1 -D warnings
      -o "${shell_object}" "${CMAKE_CURRENT_SOURCE_DIR}/src/lib.rs"
   DEPENDS Cargo.toml src/lib.rs src/accounts.rs src/config.rs src/session.rs src/launcher.rs src/cafe.rs src/canvas.rs src/mock.rs "${CAFE_RUSTC}" ${theme_inputs}
   VERBATIM)

set(shader_object "${CMAKE_CURRENT_BINARY_DIR}/pregame_gsh.o")
add_custom_command(OUTPUT "${shader_object}"
   COMMAND "${CMAKE_COMMAND}" -E copy gx2_hud/hud.gsh pregame.gsh
   COMMAND "${OBJCOPY_EXECUTABLE}" -I binary -O elf32-powerpc -B powerpc:common
      pregame.gsh pregame_gsh.o
   DEPENDS horizon_gx2_hud_shader "${CMAKE_CURRENT_BINARY_DIR}/gx2_hud/hud.gsh"
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}" VERBATIM)
set_source_files_properties("${shell_object}" "${shader_object}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)

add_executable(horizon_wiiu_pregame ../host/cafe_host.c "${shell_object}" "${shader_object}")
target_compile_options(horizon_wiiu_pregame PRIVATE -Wall -Wextra -Werror)
target_link_libraries(horizon_wiiu_pregame PRIVATE horizon_gx2_hud)

set(package_elf "${CMAKE_CURRENT_BINARY_DIR}/pregame.package.elf")
set(rpx "${CMAKE_CURRENT_BINARY_DIR}/${package_name}.rpx")
set(wuhb "${CMAKE_CURRENT_BINARY_DIR}/${package_name}.wuhb")
add_custom_command(OUTPUT "${rpx}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_wiiu_pregame>" "${package_elf}"
   COMMAND "${OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack
      --redefine-sym __rplwrap_exit=exit "${package_elf}"
   COMMAND "${CMAKE_COMMAND}" "-DCARGO=${CARGO_EXECUTABLE}" "-DPACKAGE_ELF=${package_elf}"
      "-DREADRPL=${READRPL_EXECUTABLE}" "-DRPX=${rpx}" -P "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
   DEPENDS horizon_wiiu_pregame ../package_rpx.cmake "${OBJCOPY_EXECUTABLE}"
      "${CARGO_EXECUTABLE}" "${CARGO_WIIU_EXECUTABLE}" "${READRPL_EXECUTABLE}"
   VERBATIM)
add_custom_command(OUTPUT "${wuhb}"
   COMMAND "${WUHBTOOL_EXECUTABLE}" "${rpx}" "${wuhb}" "--name=${app_title}" "--short-name=Horizon"
   DEPENDS "${rpx}" "${WUHBTOOL_EXECUTABLE}" VERBATIM)
add_custom_target(horizon_wiiu_pregame_package ALL DEPENDS "${wuhb}")
