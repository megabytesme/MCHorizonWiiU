cmake_minimum_required(VERSION 3.13)
project(horizon_gx2_hud C)
find_program(CAFEGLSL_COMPILER_EXECUTABLE glslcompiler REQUIRED)

add_library(horizon_gx2_hud STATIC hud.c hud_data.c cit_glint.c)
target_include_directories(horizon_gx2_hud PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(horizon_gx2_hud PRIVATE -Wall -Wextra -Werror)

add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/hud.gsh
   COMMAND ${CAFEGLSL_COMPILER_EXECUTABLE} -vs ${CMAKE_CURRENT_SOURCE_DIR}/hud.vert
           -ps ${CMAKE_CURRENT_SOURCE_DIR}/hud.frag -o ${CMAKE_CURRENT_BINARY_DIR}/hud.gsh
   DEPENDS hud.vert hud.frag VERBATIM)
add_custom_target(horizon_gx2_hud_shader ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/hud.gsh)
