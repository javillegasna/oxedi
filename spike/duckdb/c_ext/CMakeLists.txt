cmake_minimum_required(VERSION 3.5...3.29)

if(NOT DEFINED EXTENSION_NAME)
    message(FATAL_ERROR "DuckDB extension name is required")
endif()
if (DEFINED TARGET_DUCKDB_VERSION_MAJOR)
    add_definitions(-DDUCKDB_EXTENSION_API_VERSION_MAJOR=${TARGET_DUCKDB_VERSION_MAJOR})
endif()
if (DEFINED TARGET_DUCKDB_VERSION_MINOR)
    add_definitions(-DDUCKDB_EXTENSION_API_VERSION_MINOR=${TARGET_DUCKDB_VERSION_MINOR})
endif()
if (DEFINED TARGET_DUCKDB_VERSION_PATCH)
    add_definitions(-DDUCKDB_EXTENSION_API_VERSION_PATCH=${TARGET_DUCKDB_VERSION_PATCH})
endif()
add_definitions(-DDUCKDB_EXTENSION_NAME=${EXTENSION_NAME})
if (DEFINED DUCKDB_EXTENSION_API_VERSION_UNSTABLE)
    add_definitions(-DDUCKDB_EXTENSION_API_VERSION_UNSTABLE=${DUCKDB_EXTENSION_API_VERSION_UNSTABLE})
endif()

project(${EXTENSION_NAME} LANGUAGES C)

add_library(${EXTENSION_NAME} SHARED src/oxedi835_ext.c)

set(CMAKE_C_VISIBILITY_PRESET hidden)
set(CMAKE_C_FLAGS_RELEASE "${CMAKE_C_FLAGS_RELEASE} -s")

target_include_directories(${EXTENSION_NAME} PRIVATE src/include duckdb_capi)

# The Rust core, built beforehand by `make ffi`.
target_link_libraries(${EXTENSION_NAME} PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/ffi/target/release/liboxedi835_ffi.a pthread dl m)
