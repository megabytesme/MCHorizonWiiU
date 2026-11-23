cmake_minimum_required(VERSION 3.13)
project(horizon_cafe_fs_probe C)

find_program(POWERPC_OBJCOPY powerpc-eabi-objcopy REQUIRED)
find_program(ELF2RPL elf2rpl REQUIRED)
find_program(WUHBTOOL wuhbtool REQUIRED)
add_executable(horizon_cafe_fs_probe main.c)
target_compile_options(horizon_cafe_fs_probe PRIVATE -Wall -Wextra -Werror)

set(package "${CMAKE_CURRENT_BINARY_DIR}/horizon_cafe_fs_probe.package.elf")
set(rpx "${CMAKE_CURRENT_BINARY_DIR}/HorizonFSProbe.rpx")
set(wuhb "${CMAKE_CURRENT_BINARY_DIR}/HorizonFSProbe.wuhb")
add_custom_command(OUTPUT "${rpx}" "${wuhb}"
   COMMAND "${CMAKE_COMMAND}" -E copy "$<TARGET_FILE:horizon_cafe_fs_probe>" "${package}"
   COMMAND "${POWERPC_OBJCOPY}" --strip-debug --remove-section=.note.GNU-stack "${package}"
   COMMAND "${ELF2RPL}" "${package}" "${rpx}"
   COMMAND "${WUHBTOOL}" "${rpx}" "${wuhb}" "--name=Horizon Cafe FS Probe"
   DEPENDS horizon_cafe_fs_probe VERBATIM)
add_custom_target(horizon_cafe_fs_probe_package ALL DEPENDS "${rpx}" "${wuhb}")
