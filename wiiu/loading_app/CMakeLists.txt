cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_loading_app C CXX)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

option(HORIZON_WIIU_TRACE_VPAD_INPUT "Log Cafe VPAD input transitions to the serial log" OFF)
option(HORIZON_WIIU_TRACE_SWKBD "Log CafeOS keyboard stage transitions to the serial log" OFF)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(CARGO_WIIU_EXECUTABLE cargo-wiiu REQUIRED)
find_program(PYTHON3_EXECUTABLE python3 REQUIRED)
find_program(POWERPC_AR_EXECUTABLE powerpc-eabi-ar REQUIRED)
find_program(POWERPC_NM_EXECUTABLE powerpc-eabi-nm REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(READRPL_EXECUTABLE readrpl REQUIRED)

set(DEVKITPRO "$ENV{DEVKITPRO}" CACHE PATH "devkitPro installation")
if(NOT DEVKITPRO OR NOT IS_DIRECTORY "${DEVKITPRO}/wut")
   message(FATAL_ERROR "Set DEVKITPRO to an installed devkitPro tree containing wut/")
endif()
set(WUT_ROOT "${DEVKITPRO}/wut")
set(WUT_SPECS "${WUT_ROOT}/share/wut.specs")
set(WUT_LINKER_SCRIPT "${WUT_ROOT}/share/wut.ld")
foreach(required IN ITEMS "${WUT_SPECS}" "${WUT_LINKER_SCRIPT}" "${WUT_ROOT}/lib/libwut.a")
   if(NOT EXISTS "${required}")
      message(FATAL_ERROR "Required WUT file is missing: ${required}")
   endif()
endforeach()

# The stage1 rustc supplies the built-in powerpc-nintendo-cafe target and its
# sysroot. Do not use the stale per-project target JSON for the app build.
set(HORIZON_WIIU_PREBUILT_STATICLIB "$ENV{HORIZON_WIIU_PREBUILT_STATICLIB}" CACHE FILEPATH
   "Optional verified Cafe loading-app static archive for native-only packaging")
set(HORIZON_WIIU_STD_RUSTC "$ENV{HORIZON_WIIU_STD_RUSTC}" CACHE FILEPATH
   "Cafe stage1 rustc with the built-in powerpc-nintendo-cafe target")
if(NOT HORIZON_WIIU_PREBUILT_STATICLIB AND
   (NOT HORIZON_WIIU_STD_RUSTC OR NOT EXISTS "${HORIZON_WIIU_STD_RUSTC}"))
   message(FATAL_ERROR "Set HORIZON_WIIU_STD_RUSTC to the Cafe stage1 rustc executable")
endif()

# Normal builds compile from the selected manifest (which can be a patched
# scratch copy); a verified archive can be supplied to exercise native linking
# and RPX packaging without invoking Cargo.
set(HORIZON_WIIU_CARGO_MANIFEST "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml" CACHE FILEPATH
   "Loading-app Cargo.toml (use a patched scratch manifest when needed)")
set(HORIZON_WIIU_CARGO_TARGET_DIR "$ENV{CARGO_TARGET_DIR}" CACHE PATH
   "Reusable Cargo target directory; defaults to the CARGO_TARGET_DIR environment")
if(NOT HORIZON_WIIU_CARGO_TARGET_DIR)
   set(HORIZON_WIIU_CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust-target")
endif()
set(HORIZON_WIIU_CARGO_HOME "$ENV{CARGO_HOME}" CACHE PATH "Optional reusable Cargo home/cache")
set(HORIZON_WIIU_CARGO_JOBS "1" CACHE STRING "Cargo build jobs (conservative Cafe default)")
set(HORIZON_WIIU_CARGO_RUSTFLAGS "$ENV{RUSTFLAGS}" CACHE STRING
   "Additional Rust flags; Cafe's custom getrandom backend is added automatically")

set(HORIZON_WIIU_WINIT_TV_SIZE_SOURCE "$ENV{HORIZON_WIIU_WINIT_TV_SIZE_SOURCE}" CACHE FILEPATH
   "tv_size.c from the configured patched Winit scratch tree")
if(NOT HORIZON_WIIU_WINIT_TV_SIZE_SOURCE OR NOT EXISTS "${HORIZON_WIIU_WINIT_TV_SIZE_SOURCE}")
   message(FATAL_ERROR
      "Set HORIZON_WIIU_WINIT_TV_SIZE_SOURCE to the scratch Winit tv_size.c adapter; "
      "the current Winit Cafe backend needs cafe_winit_tv_size at native link time")
endif()

# Use the installed WUT platform's linker paths/libraries, but derive a local
# linker script/specs pair that places crtend text beside .init. This prevents
# the Cafe far-branch overflow seen when crtend is left at the end of .text.
add_compile_options(-mcpu=750 -meabi -mhard-float -DESPRESSO -D__WIIU__ -D__WUT__)
include_directories(SYSTEM "${WUT_ROOT}/include")
set(LOCAL_WUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/wut-link")
set(LOCAL_WUT_LINKER_SCRIPT "${LOCAL_WUT_DIR}/wut-startup-near.ld")
set(LOCAL_WUT_SPECS "${LOCAL_WUT_DIR}/wut-startup-near.specs")
file(MAKE_DIRECTORY "${LOCAL_WUT_DIR}")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
   "${WUT_LINKER_SCRIPT}" "${WUT_SPECS}" "${CMAKE_CURRENT_SOURCE_DIR}/cmake/prepare_wut_linker.cmake")
execute_process(
   COMMAND "${CMAKE_COMMAND}"
      "-DWUT_LINKER_SCRIPT=${WUT_LINKER_SCRIPT}"
      "-DWUT_SPECS=${WUT_SPECS}"
      "-DOUTPUT_LINKER_SCRIPT=${LOCAL_WUT_LINKER_SCRIPT}"
      "-DOUTPUT_SPECS=${LOCAL_WUT_SPECS}"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/cmake/prepare_wut_linker.cmake"
   RESULT_VARIABLE prepare_wut_result
   OUTPUT_VARIABLE prepare_wut_output
   ERROR_VARIABLE prepare_wut_error)
if(NOT prepare_wut_result STREQUAL "0")
   message(FATAL_ERROR "Could not derive the local WUT startup linker script:\n${prepare_wut_output}${prepare_wut_error}")
endif()

# powerpc-eabi-cmake normally initializes these with WUT's link paths and
# wut.specs. Replace only that specs path, preserving any other toolchain flags.
set(_wut_link_flags
   "-L${WUT_ROOT}/lib -L${DEVKITPRO}/portlibs/wiiu/lib -L${DEVKITPRO}/portlibs/ppc/lib ${CMAKE_EXE_LINKER_FLAGS}")
string(REGEX REPLACE "-specs=[^ \t]*wut\\.specs" "" _wut_link_flags "${_wut_link_flags}")
string(REPLACE "-specs=${LOCAL_WUT_SPECS}" "" _wut_link_flags "${_wut_link_flags}")
string(APPEND _wut_link_flags " -specs=${LOCAL_WUT_SPECS}")
set(CMAKE_EXE_LINKER_FLAGS "${_wut_link_flags}" CACHE STRING "Cafe WUT linker flags" FORCE)
set(CMAKE_EXE_LINKER_FLAGS "${_wut_link_flags}")

set(RUST_TARGET "powerpc-nintendo-cafe")
if(HORIZON_WIIU_PREBUILT_STATICLIB)
   if(NOT EXISTS "${HORIZON_WIIU_PREBUILT_STATICLIB}")
      message(FATAL_ERROR "HORIZON_WIIU_PREBUILT_STATICLIB does not exist: ${HORIZON_WIIU_PREBUILT_STATICLIB}")
   endif()
   set(RUST_STATICLIB "${HORIZON_WIIU_PREBUILT_STATICLIB}")
else()
   if(NOT EXISTS "${HORIZON_WIIU_CARGO_MANIFEST}")
      message(FATAL_ERROR "HORIZON_WIIU_CARGO_MANIFEST does not exist: ${HORIZON_WIIU_CARGO_MANIFEST}")
   endif()
   get_filename_component(CARGO_WORKING_DIRECTORY "${HORIZON_WIIU_CARGO_MANIFEST}" DIRECTORY)
   set(RUST_STATICLIB
      "${HORIZON_WIIU_CARGO_TARGET_DIR}/${RUST_TARGET}/release/libhorizon_wiiu_loading_app.a")
   set(_cargo_environment
      "CARGO_TARGET_DIR=${HORIZON_WIIU_CARGO_TARGET_DIR}"
      "RUSTC=${HORIZON_WIIU_STD_RUSTC}"
      "CC_powerpc-nintendo-cafe=${CMAKE_C_COMPILER}"
      "RUSTFLAGS=${HORIZON_WIIU_CARGO_RUSTFLAGS} --cfg getrandom_backend=\"custom\"")
   if(HORIZON_WIIU_CARGO_HOME)
      list(APPEND _cargo_environment "CARGO_HOME=${HORIZON_WIIU_CARGO_HOME}")
   endif()
   # A target without OUTPUT is intentionally always run: Cargo reuses its
   # incremental cache, while CMake cannot track all transitive patched crates.
   add_custom_target(horizon_wiiu_loading_rust
      COMMAND "${CMAKE_COMMAND}" -E env ${_cargo_environment}
         "${CARGO_EXECUTABLE}" +nightly build --release --lib
         --manifest-path "${HORIZON_WIIU_CARGO_MANIFEST}"
         --target "${RUST_TARGET}" --jobs "${HORIZON_WIIU_CARGO_JOBS}"
      BYPRODUCTS "${RUST_STATICLIB}"
      WORKING_DIRECTORY "${CARGO_WORKING_DIRECTORY}"
      COMMENT "Building the Cafe Loading static library with stage1 rustc"
      VERBATIM)
endif()

set(WUT_CRT0_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/wut_crt0_rpx.o")
set(WUT_RUST_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libhorizon_wiiu_loading_wut.a")
add_custom_command(
   OUTPUT "${WUT_CRT0_OBJECT}" "${WUT_RUST_STATICLIB}"
   COMMAND "${CMAKE_COMMAND}" -E env
      "POWERPC_AR=${POWERPC_AR_EXECUTABLE}"
      "POWERPC_NM=${POWERPC_NM_EXECUTABLE}"
      "POWERPC_OBJCOPY=${POWERPC_OBJCOPY_EXECUTABLE}"
      bash "${CMAKE_CURRENT_SOURCE_DIR}/../bootstrap/prepare_wut_startup.sh"
      "${RUST_STATICLIB}" "${WUT_ROOT}/lib/libwut.a"
      "${WUT_RUST_STATICLIB}" "${WUT_CRT0_OBJECT}"
   DEPENDS "${RUST_STATICLIB}" "${WUT_ROOT}/lib/libwut.a"
      "${CMAKE_CURRENT_SOURCE_DIR}/../bootstrap/prepare_wut_startup.sh"
   COMMENT "Selecting the WUT RPX startup from the Cafe archive"
   VERBATIM)
set_source_files_properties("${WUT_CRT0_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_custom_target(horizon_wiiu_loading_wut DEPENDS "${WUT_CRT0_OBJECT}" "${WUT_RUST_STATICLIB}")

add_library(horizon_wiiu_loading_rustlib STATIC IMPORTED GLOBAL)
set_target_properties(horizon_wiiu_loading_rustlib PROPERTIES IMPORTED_LOCATION "${WUT_RUST_STATICLIB}")

add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../loading_host"
   "${CMAKE_CURRENT_BINARY_DIR}/loading_host")

add_executable(horizon_wiiu_loading_app
   "${WUT_CRT0_OBJECT}"
   main.c
   ../audio/audio.c
   ../platform/src/keyboard_bridge.c
   ../platform/src/mouse_bridge.c
   ../platform/src/vpad_bridge.c
   "${HORIZON_WIIU_WINIT_TV_SIZE_SOURCE}")
target_include_directories(horizon_wiiu_loading_app PRIVATE ../audio ../platform/src)
if(HORIZON_WIIU_TRACE_VPAD_INPUT)
   target_compile_definitions(horizon_wiiu_loading_app PRIVATE HORIZON_WIIU_TRACE_VPAD_INPUT=1)
endif()
if(HORIZON_WIIU_TRACE_SWKBD)
   target_compile_definitions(horizon_wiiu_loading_app PRIVATE HORIZON_WIIU_TRACE_SWKBD=1)
   target_compile_definitions(horizon_wiiu_loading_host PRIVATE HORIZON_WIIU_TRACE_SWKBD=1)
endif()
add_dependencies(horizon_wiiu_loading_app horizon_wiiu_loading_wut)
if(NOT HORIZON_WIIU_PREBUILT_STATICLIB)
   add_dependencies(horizon_wiiu_loading_app horizon_wiiu_loading_rust)
endif()
target_link_options(horizon_wiiu_loading_app PRIVATE
   -mcpu=750 -meabi -mhard-float
   "-Wl,--defsym=__end__=ADDR(.bss)+SIZEOF(.bss)")
target_link_libraries(horizon_wiiu_loading_app PRIVATE
   horizon_wiiu_loading_rustlib
   horizon_wiiu_loading_host)
target_compile_options(horizon_wiiu_loading_app PRIVATE -Wall -Wextra -Werror)

set(PACKAGE_ELF "${CMAKE_CURRENT_BINARY_DIR}/loading_app.package.elf")
set(RELOCATION_NORMALIZER "${CMAKE_CURRENT_SOURCE_DIR}/../../tools/wiiu_normalize_rpl_relocations.py")
set(RPX "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_loading_app.rpx")
add_custom_command(
   OUTPUT "${RPX}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_wiiu_loading_app>" "${PACKAGE_ELF}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack
      --redefine-sym __rplwrap_exit=exit "${PACKAGE_ELF}"
   COMMAND "${PYTHON3_EXECUTABLE}" "${RELOCATION_NORMALIZER}" "${PACKAGE_ELF}"
   COMMAND "${CMAKE_COMMAND}" "-DCARGO=${CARGO_EXECUTABLE}" "-DPACKAGE_ELF=${PACKAGE_ELF}"
      "-DREADRPL=${READRPL_EXECUTABLE}" "-DRPX=${RPX}"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
   DEPENDS horizon_wiiu_loading_app "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
      "${CARGO_EXECUTABLE}" "${CARGO_WIIU_EXECUTABLE}" "${POWERPC_OBJCOPY_EXECUTABLE}"
      "${READRPL_EXECUTABLE}" "${RELOCATION_NORMALIZER}"
   VERBATIM)
add_custom_target(horizon_wiiu_loading_rpx ALL DEPENDS "${RPX}")
