cmake_minimum_required(VERSION 3.14)
project(cassiopeia_runtime LANGUAGES CXX)
get_filename_component(CASSIOPEIA_PLUGIN_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}" REALPATH)
get_filename_component(CASSIOPEIA_WORKSPACE "${CASSIOPEIA_PLUGIN_SOURCE}/../.." ABSOLUTE)
find_program(CASSIOPEIA_CARGO_EXECUTABLE cargo REQUIRED)
set(CASSIOPEIA_CARGO_TARGET "${CMAKE_CURRENT_BINARY_DIR}/rust")
set(CASSIOPEIA_RUNTIME_LIBRARY "${CASSIOPEIA_CARGO_TARGET}/release/cassiopeia_native.dll")
file(GLOB_RECURSE CASSIOPEIA_RUST_SOURCES CONFIGURE_DEPENDS "${CASSIOPEIA_WORKSPACE}/crates/*.rs")
add_custom_command(OUTPUT "${CASSIOPEIA_RUNTIME_LIBRARY}"
  COMMAND "${CASSIOPEIA_CARGO_EXECUTABLE}" build --release --locked -p haneoka-cassiopeia-native
    --manifest-path "${CASSIOPEIA_WORKSPACE}/Cargo.toml" --target-dir "${CASSIOPEIA_CARGO_TARGET}"
  DEPENDS ${CASSIOPEIA_RUST_SOURCES} "${CASSIOPEIA_WORKSPACE}/Cargo.toml"
    "${CASSIOPEIA_WORKSPACE}/Cargo.lock"
    "${CASSIOPEIA_WORKSPACE}/crates/cassiopeia-core/Cargo.toml"
    "${CASSIOPEIA_WORKSPACE}/crates/cassiopeia-native/Cargo.toml"
  VERBATIM)
add_custom_target(cassiopeia_runtime_kernel ALL DEPENDS "${CASSIOPEIA_RUNTIME_LIBRARY}")
set(cassiopeia_runtime_bundled_libraries "${CASSIOPEIA_RUNTIME_LIBRARY}" PARENT_SCOPE)
