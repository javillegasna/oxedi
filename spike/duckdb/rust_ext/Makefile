.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=oxedi835

# 1 pins the binary to TARGET_DUCKDB_VERSION (ABI C_STRUCT_UNSTABLE);
# 0 builds ABI C_STRUCT, loadable by TARGET_DUCKDB_VERSION and later.
USE_UNSTABLE_C_API ?= 0

TARGET_DUCKDB_VERSION ?= v1.5.6

all: configure release

include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_release
test_debug: test_extension_debug
test_release: test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean
