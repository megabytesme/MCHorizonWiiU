cmake_minimum_required(VERSION 3.13)
project(horizon_wiiu_loading_host C)
find_program(POWERPC_OBJCOPY_EXECUTABLE powerpc-eabi-objcopy REQUIRED)

# Keep all generated files under the caller's binary tree. These are the
# existing HUD and panorama implementations; this target adds only the WHB
# target-switching callback needed to drive both displays from one batch.
add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../gx2_hud"
                 "${CMAKE_CURRENT_BINARY_DIR}/gx2_hud")
add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../gx2_panorama"
                 "${CMAKE_CURRENT_BINARY_DIR}/gx2_panorama")

set(HUD_GFD_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/hud_gfd.o")
set(PANORAMA_GFD_OBJECT "${CMAKE_CURRENT_BINARY_DIR}/panorama_gfd.o")
add_custom_command(
   OUTPUT "${HUD_GFD_OBJECT}"
   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
      "${CMAKE_CURRENT_BINARY_DIR}/gx2_hud/hud.gsh" hud.gsh
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}"
      -I binary -O elf32-powerpc -B powerpc:common hud.gsh hud_gfd.o
   DEPENDS horizon_gx2_hud_shader
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
   VERBATIM)
add_custom_command(
   OUTPUT "${PANORAMA_GFD_OBJECT}"
   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
      "${CMAKE_CURRENT_BINARY_DIR}/gx2_panorama/panorama.gsh" panorama.gsh
   COMMAND "${POWERPC_OBJCOPY_EXECUTABLE}"
      -I binary -O elf32-powerpc -B powerpc:common panorama.gsh panorama_gfd.o
   DEPENDS horizon_gx2_panorama_shader
   WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
   VERBATIM)
set_source_files_properties("${HUD_GFD_OBJECT}" "${PANORAMA_GFD_OBJECT}"
   PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)

add_library(horizon_wiiu_loading_host STATIC
   loading_host.c "${HUD_GFD_OBJECT}" "${PANORAMA_GFD_OBJECT}")
target_include_directories(horizon_wiiu_loading_host PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(horizon_wiiu_loading_host PRIVATE -Wall -Wextra -Werror)
target_link_libraries(horizon_wiiu_loading_host
   PUBLIC horizon_gx2_hud horizon_gx2_panorama)
