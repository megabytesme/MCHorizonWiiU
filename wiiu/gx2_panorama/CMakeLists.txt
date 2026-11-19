cmake_minimum_required(VERSION 3.13)
project(horizon_gx2_panorama C)
find_program(CAFEGLSL_COMPILER_EXECUTABLE glslcompiler REQUIRED)

add_library(horizon_gx2_panorama STATIC panorama.c)
target_include_directories(horizon_gx2_panorama PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(horizon_gx2_panorama PRIVATE -Wall -Wextra -Werror)

add_custom_command(OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/panorama.gsh
   COMMAND ${CAFEGLSL_COMPILER_EXECUTABLE} -vs ${CMAKE_CURRENT_SOURCE_DIR}/panorama.vert
           -ps ${CMAKE_CURRENT_SOURCE_DIR}/panorama.frag -o ${CMAKE_CURRENT_BINARY_DIR}/panorama.gsh
   DEPENDS panorama.vert panorama.frag VERBATIM)
add_custom_target(horizon_gx2_panorama_shader ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/panorama.gsh)
