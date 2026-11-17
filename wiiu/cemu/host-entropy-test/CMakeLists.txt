cmake_minimum_required(VERSION 3.13)
project(horizon_cemu_host_entropy_smoke C)
option(HORIZON_TEST_PREINIT "Run the real entropy probe before WHBProcInit" OFF)
find_program(POWERPC_AR_EXECUTABLE powerpc-eabi-ar REQUIRED)
find_program(POWERPC_NM_EXECUTABLE powerpc-eabi-nm REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(READRPL_EXECUTABLE readrpl REQUIRED)
find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(HORIZON_ENTROPY_STATICLIB "" CACHE FILEPATH "Real Cafe entropy smoke archive")
if(NOT EXISTS "${HORIZON_ENTROPY_STATICLIB}")
   message(FATAL_ERROR "Set HORIZON_ENTROPY_STATICLIB to the real entropy smoke archive")
endif()
get_filename_component(WIIU_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(WUT_CRT0_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/wut_crt0_rpx.o")
set(WUT_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libentropy_wut.a")
add_custom_command(OUTPUT "${WUT_CRT0_OBJECT}" "${WUT_STATICLIB}"
   COMMAND "${CMAKE_COMMAND}" -E env "POWERPC_AR=${POWERPC_AR_EXECUTABLE}"
      "POWERPC_NM=${POWERPC_NM_EXECUTABLE}" "POWERPC_OBJCOPY=${POWERPC_OBJCOPY_EXECUTABLE}"
      bash "${WIIU_ROOT}/bootstrap/prepare_wut_startup.sh" "${HORIZON_ENTROPY_STATICLIB}"
      "$ENV{DEVKITPRO}/wut/lib/libwut.a" "${WUT_STATICLIB}" "${WUT_CRT0_OBJECT}"
   DEPENDS "${HORIZON_ENTROPY_STATICLIB}" "${WIIU_ROOT}/bootstrap/prepare_wut_startup.sh" VERBATIM)
set_source_files_properties("${WUT_CRT0_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_custom_target(entropy_wut_startup DEPENDS "${WUT_CRT0_OBJECT}" "${WUT_STATICLIB}")
add_library(entropy_rust STATIC IMPORTED GLOBAL)
set_target_properties(entropy_rust PROPERTIES IMPORTED_LOCATION "${WUT_STATICLIB}")
add_executable(host_entropy_smoke "${WUT_CRT0_OBJECT}" smoke.c pool.c)
add_dependencies(host_entropy_smoke entropy_wut_startup)
if(HORIZON_TEST_PREINIT)
   target_compile_definitions(host_entropy_smoke PRIVATE HORIZON_TEST_PREINIT)
endif()
target_link_options(host_entropy_smoke PRIVATE "LINKER:--defsym=__end__=ADDR(.bss)+SIZEOF(.bss)"
   "LINKER:--wrap=NSSInit" "LINKER:--wrap=NSSGetRandom")
target_link_libraries(host_entropy_smoke PRIVATE entropy_rust)
set(PACKAGE_ELF "${CMAKE_CURRENT_BINARY_DIR}/smoke.package.elf")
set(RPX "${CMAKE_CURRENT_BINARY_DIR}/host_entropy_smoke.rpx")
add_custom_command(OUTPUT "${RPX}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:host_entropy_smoke>" "${PACKAGE_ELF}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack
      --redefine-sym __rplwrap_exit=exit "${PACKAGE_ELF}"
   COMMAND "${CMAKE_COMMAND}" "-DCARGO=${CARGO_EXECUTABLE}" "-DPACKAGE_ELF=${PACKAGE_ELF}"
      "-DREADRPL=${READRPL_EXECUTABLE}" "-DRPX=${RPX}" -P "${WIIU_ROOT}/package_rpx.cmake"
   DEPENDS host_entropy_smoke "${WIIU_ROOT}/package_rpx.cmake" VERBATIM)
add_custom_target(host_entropy_rpx ALL DEPENDS "${RPX}")
