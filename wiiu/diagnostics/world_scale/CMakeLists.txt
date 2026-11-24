cmake_minimum_required(VERSION 3.13)
project(horizon_gx2_world_scale_diagnostic C)

if(NOT EXISTS "${HORIZON_WIIU_ASSEMBLED}/wiiu/loading_host/world_scale.c")
   message(FATAL_ERROR "Set HORIZON_WIIU_ASSEMBLED to the staged Horizon source")
endif()

add_subdirectory("${HORIZON_WIIU_ASSEMBLED}/wiiu/loading_host" loading_host)
add_executable(gx2_world_scale_diagnostic main.c)
target_compile_options(gx2_world_scale_diagnostic PRIVATE -Wall -Wextra -Werror)
target_link_libraries(gx2_world_scale_diagnostic PRIVATE horizon_wiiu_loading_host)
wut_create_rpx(gx2_world_scale_diagnostic)
