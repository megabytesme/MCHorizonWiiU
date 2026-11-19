cmake_minimum_required(VERSION 3.13)
project(horizon_gx2_hud_diagnostic C)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)
find_program(ELF2RPL_EXECUTABLE elf2rpl REQUIRED)
add_subdirectory(.. hud_lib)

add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/hud_diag_gsh.o
   COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_BINARY_DIR}/hud_lib/hud.gsh hud_diag.gsh
   COMMAND ${POWERPC_OBJCOPY_EXECUTABLE} -I binary -O elf32-powerpc -B powerpc:common hud_diag.gsh hud_diag_gsh.o
   DEPENDS horizon_gx2_hud_shader ${CMAKE_CURRENT_BINARY_DIR}/hud_lib/hud.gsh
   WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR} VERBATIM)
set_source_files_properties(${CMAKE_CURRENT_BINARY_DIR}/hud_diag_gsh.o PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
add_executable(horizon_gx2_hud_diag main.c ${CMAKE_CURRENT_BINARY_DIR}/hud_diag_gsh.o)
target_link_libraries(horizon_gx2_hud_diag PRIVATE horizon_gx2_hud)
target_compile_options(horizon_gx2_hud_diag PRIVATE -Wall -Wextra -Werror)

add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/horizon_gx2_hud_diag.rpx
   COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:horizon_gx2_hud_diag> ${CMAKE_CURRENT_BINARY_DIR}/hud_diag.package.elf
   COMMAND ${POWERPC_OBJCOPY_EXECUTABLE} --strip-debug --remove-section=.note.GNU-stack ${CMAKE_CURRENT_BINARY_DIR}/hud_diag.package.elf
   COMMAND ${ELF2RPL_EXECUTABLE} ${CMAKE_CURRENT_BINARY_DIR}/hud_diag.package.elf ${CMAKE_CURRENT_BINARY_DIR}/horizon_gx2_hud_diag.rpx
   DEPENDS horizon_gx2_hud_diag VERBATIM)
add_custom_target(horizon_gx2_hud_diag_rpx ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/horizon_gx2_hud_diag.rpx)
