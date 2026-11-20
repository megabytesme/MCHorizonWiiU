# The standalone shell emits a Rust object. The hosted variant gets its
# runtime from the game archive instead.
function(horizon_link_standalone_rust)
   set(source "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/standalone_panic.rs")
   set(object "${CMAKE_CURRENT_BINARY_DIR}/standalone_panic.o")
   add_custom_command(OUTPUT "${object}"
      COMMAND "${CAFE_RUSTC}" --edition 2021 --target powerpc-nintendo-cafe
         --crate-type lib --emit=obj -C panic=abort -C opt-level=s
         "--remap-path-prefix=${CMAKE_CURRENT_FUNCTION_LIST_DIR}=/horizon-port/wiiu/builder"
         -o "${object}" "${source}"
      DEPENDS "${source}" "${CAFE_RUSTC}" VERBATIM)
   set_source_files_properties("${object}" PROPERTIES GENERATED TRUE EXTERNAL_OBJECT TRUE)
   target_sources(horizon_wiiu_pregame PRIVATE "${object}")
   target_link_libraries(horizon_wiiu_pregame PRIVATE "${HORIZON_CAFE_LIBCORE}" "${HORIZON_CAFE_BUILTINS}")
endfunction()

cmake_language(DEFER CALL horizon_link_standalone_rust)
