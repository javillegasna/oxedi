# Build and test entry points of the DuckDB extension `oxedi`, on DuckDB's
# extension-ci-tools. The repository root delegates its configure_ci, release,
# debug, test_release and test_debug targets here, so this directory is also
# the working directory of every recipe below.
.PHONY: clean clean_all test_oracle oracle_run

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=oxedi

# The extension version is the crate's own, as `v<version>`. Left to itself the
# ci-tools take a git tag at HEAD or the commit hash. configure/extension_version.txt
# is regenerated when the crate's Cargo.toml is newer.
EXTENSION_VERSION ?= v$(shell sed -n 's/^version = "\(.*\)"/\1/p' $(PROJ_DIR)Cargo.toml | head -n 1)

configure/extension_version.txt: $(PROJ_DIR)Cargo.toml

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

# release first, so a local run never tests a stale binary. The two steps run
# one after the other as recipe lines, so `make -j test` still builds once.
test: release
	$(MAKE) test_release
	$(MAKE) oracle_run
test_debug: test_extension_debug
# Must not build: DuckDB's CI runs it on a different host than the build.
# Our CI runs `make release` first.
test_release: test_extension_release

# The oracle test: read_835 of the release build against oxedi.parse_file
# on every sample and fixture. It needs a Python with duckdb, pyarrow, pytest
# and oxedi, which DuckDB's CI venv does not have, so test_release leaves
# it out; `test` and the root `duckdb-oracle` run it. oracle_run only runs the
# tests; test_oracle builds the release extension first, and the root target
# also rebuilds oxedi into the venv.
ORACLE_PYTHON ?= ../../.venv/bin/python
oracle_run:
	$(ORACLE_PYTHON) -m pytest -q -p no:cacheprovider test/python
test_oracle: release
	$(MAKE) oracle_run

# DuckDB's CI calls this when it builds against another DuckDB repository; a
# C API extension does not build DuckDB, so like set_duckdb_version it does
# nothing. ci-tools defines the other two but not this one.
set_duckdb_repository: nop

clean: clean_build clean_rust
clean_all: clean_configure clean
