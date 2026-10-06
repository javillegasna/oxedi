# Build and test entry points of the DuckDB extension `oxedi`, on DuckDB's
# extension-ci-tools. The repository root delegates its configure_ci, release,
# debug, test_release and test_debug targets here, so this directory is also
# the working directory of every recipe below.
.PHONY: clean clean_all test_oracle

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=oxedi

# 0 builds ABI C_STRUCT: the binary loads on TARGET_DUCKDB_VERSION and every
# later DuckDB. 1 would pin it to TARGET_DUCKDB_VERSION exactly.
USE_UNSTABLE_C_API=0

# The oldest DuckDB whose stable C API has everything the extension calls
# (client context, file system).
TARGET_DUCKDB_VERSION=v1.5.6

# The crate is a workspace member, so cargo would build into the workspace's
# target directory; the ci-tools recipes look for the library under ./target.
# The release profile is set here so that it applies to this library only.
export CARGO_TARGET_DIR := target
export CARGO_PROFILE_RELEASE_LTO := true
export CARGO_PROFILE_RELEASE_STRIP := true

# The ci-tools recipes splice this variable into `cargo build`; --locked keeps
# the duckdb-rs version of Cargo.lock.
CARGO_OVERRIDE_DUCKDB_RS_FLAG ?= --locked

all: configure release

include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_release
test_debug: test_extension_debug
test_release: test_extension_release

# The oracle test: read_835 of the release build against oxedi835.parse_file
# on every sample and fixture. It needs a Python with duckdb, pyarrow, pytest
# and oxedi835, which DuckDB's CI venv does not have, so test_release leaves
# it out; the repository's own CI runs it through the root `duckdb-oracle`.
ORACLE_PYTHON ?= ../../.venv/bin/python
test_oracle:
	$(ORACLE_PYTHON) -m pytest -q -p no:cacheprovider test/python

# DuckDB's CI calls this when it builds against another DuckDB repository; a
# C API extension does not build DuckDB, so like set_duckdb_version it does
# nothing. ci-tools defines the other two but not this one.
set_duckdb_repository: nop

clean: clean_build clean_rust
clean_all: clean_configure clean
