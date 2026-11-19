cmake_minimum_required(VERSION 3.13)
project(horizon_gx2_terrain C)
find_program(CAFEGLSL_COMPILER_EXECUTABLE glslcompiler REQUIRED)
add_library(horizon_gx2_terrain STATIC terrain.c)
target_include_directories(horizon_gx2_terrain PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(horizon_gx2_terrain PRIVATE -Wall -Wextra -Werror)
add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/terrain.gsh
   COMMAND ${CAFEGLSL_COMPILER_EXECUTABLE} -vs ${CMAKE_CURRENT_SOURCE_DIR}/terrain.vert
           -ps ${CMAKE_CURRENT_SOURCE_DIR}/terrain.frag -o ${CMAKE_CURRENT_BINARY_DIR}/terrain.gsh
   DEPENDS terrain.vert terrain.frag VERBATIM)
add_custom_target(horizon_gx2_terrain_shader ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/terrain.gsh)
