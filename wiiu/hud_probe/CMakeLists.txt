cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_hud_probe C)
find_program(CARGO_EXECUTABLE cargo REQUIRED)
find_program(CARGO_WIIU_EXECUTABLE cargo-wiiu REQUIRED)
find_program(POWERPC_AR_EXECUTABLE powerpc-eabi-ar REQUIRED)
find_program(POWERPC_NM_EXECUTABLE powerpc-eabi-nm REQUIRED)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(READRPL_EXECUTABLE readrpl REQUIRED)
set(HORIZON_HUD_STATICLIB "" CACHE FILEPATH "Cafe Rust HUD diagnostic archive")
if(NOT EXISTS "${HORIZON_HUD_STATICLIB}")
   message(FATAL_ERROR "HORIZON_HUD_STATICLIB must name the diagnostic archive")
endif()
add_subdirectory(../gx2_hud hud_lib)
set(WUT_CRT0_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/wut_crt0_rpx.o")
set(WUT_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libhud_probe_wut.a")
add_custom_command(OUTPUT "${WUT_CRT0_OBJECT}" "${WUT_STATICLIB}"
   COMMAND "${CMAKE_COMMAND}" -E env
      "POWERPC_AR=${POWERPC_AR_EXECUTABLE}" "POWERPC_NM=${POWERPC_NM_EXECUTABLE}"
      "POWERPC_OBJCOPY=${POWERPC_OBJCOPY_EXECUTABLE}"
      bash "${CMAKE_CURRENT_SOURCE_DIR}/../bootstrap/prepare_wut_startup.sh"
      "${HORIZON_HUD_STATICLIB}" "$ENV{DEVKITPRO}/wut/lib/libwut.a"
      "${WUT_STATICLIB}" "${WUT_CRT0_OBJECT}"
   DEPENDS "${HORIZON_HUD_STATICLIB}" "$ENV{DEVKITPRO}/wut/lib/libwut.a"
      "${CMAKE_CURRENT_SOURCE_DIR}/../bootstrap/prepare_wut_startup.sh" VERBATIM)
set_source_files_properties("${WUT_CRT0_OBJECT}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_custom_target(hud_probe_wut DEPENDS "${WUT_CRT0_OBJECT}" "${WUT_STATICLIB}")
add_library(hud_rust STATIC IMPORTED GLOBAL)
set_target_properties(hud_rust PROPERTIES IMPORTED_LOCATION "${WUT_STATICLIB}")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/hud_probe_gsh.o"
   COMMAND "${CMAKE_COMMAND}" -E copy "${CMAKE_CURRENT_BINARY_DIR}/hud_lib/hud.gsh" hud_probe.gsh
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" -I binary -O elf32-powerpc -B powerpc:common
      --set-section-alignment .data=256 hud_probe.gsh hud_probe_gsh.o
   DEPENDS horizon_gx2_hud_shader "${CMAKE_CURRENT_BINARY_DIR}/hud_lib/hud.gsh"
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}" VERBATIM)
set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/hud_probe_gsh.o" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_executable(horizon_wiiu_hud_probe "${WUT_CRT0_OBJECT}" main.c "${CMAKE_CURRENT_BINARY_DIR}/hud_probe_gsh.o")
add_dependencies(horizon_wiiu_hud_probe hud_probe_wut)
target_link_options(horizon_wiiu_hud_probe PRIVATE "LINKER:--defsym=__end__=ADDR(.bss)+SIZEOF(.bss)")
target_link_libraries(horizon_wiiu_hud_probe PRIVATE hud_rust horizon_gx2_hud)
target_compile_options(horizon_wiiu_hud_probe PRIVATE -Wall -Wextra -Werror)
set(PACKAGE_ELF "${CMAKE_CURRENT_BINARY_DIR}/hud_probe.package.elf")
set(RPX "${CMAKE_CURRENT_BINARY_DIR}/horizon_wiiu_hud_probe.rpx")
add_custom_command(OUTPUT "${RPX}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_wiiu_hud_probe>" "${PACKAGE_ELF}"
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}" --strip-debug --remove-section=.note.GNU-stack
      --redefine-sym __rplwrap_exit=exit "${PACKAGE_ELF}"
   COMMAND "${CMAKE_COMMAND}" "-DCARGO=${CARGO_EXECUTABLE}" "-DPACKAGE_ELF=${PACKAGE_ELF}"
      "-DREADRPL=${READRPL_EXECUTABLE}" "-DRPX=${RPX}"
      -P "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
   DEPENDS horizon_wiiu_hud_probe "${CMAKE_CURRENT_SOURCE_DIR}/../package_rpx.cmake"
      "${CARGO_EXECUTABLE}" "${CARGO_WIIU_EXECUTABLE}" "${POWERPC_OBJCOPY_EXECUTABLE}" "${READRPL_EXECUTABLE}" VERBATIM)
add_custom_target(hud_probe_rpx ALL DEPENDS "${RPX}")
