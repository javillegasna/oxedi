# Contributing

## Setup

Prerequisites: the Rust toolchain pinned in `rust-toolchain.toml` (installed by `rustup`),
Python 3.11 or later, and [uv](https://docs.astral.sh/uv/). `make venv` creates `.venv` with
maturin and the test tools.

```bash
make help       # list every target
make venv       # create .venv with the dev tools
make py-dev     # build the extension into .venv (debug)
make gates      # format, clippy, Rust tests, benches compile, docs
make py-test    # build and run the Python test suite
make dist       # build the sdist and the release wheel into target/wheels
make smoke      # install the built wheel in a clean venv and run the suite
```

## Test files

The files under `crates/oxedi_core/tests/fixtures/` and `crates/oxedi_core/tests/samples/` are
never edited.

## DuckDB extension

From the repository root (Rust, Python 3 and the `extension-ci-tools` submodule are needed):

```bash
git submodule update --init
make configure_ci          # test venv, platform and version (the crate's, as v<version>)
make release               # build/release/extension/oxedi/oxedi.duckdb_extension
make test_release          # SQLLogicTest files in crates/oxedi_duckdb/test/sql
make duckdb-oracle         # read_835 against oxedi.parse_file, row by row
```

Load a local build with `duckdb -unsigned` and
`LOAD 'build/release/extension/oxedi/oxedi.duckdb_extension';`. `description.yml` in
`crates/oxedi_duckdb` is the descriptor for the community repository.

## Documentation

`README.md` is for GitHub and links to the guides under `docs/` with relative links.
`crates/oxedi_py/README.md` is the PyPI page and has the same content with absolute
`https://github.com/javillegasna/oxedi/blob/master/...` links, because relative links break on
PyPI. Change the root `README.md` and regenerate the other with `make readme-py`.

## Releases

`make release-check TAG=v<version>` checks the tag, a clean tree and the changelog entry before
a release. The Python package and the DuckDB extension are versioned and tagged separately
(`v*` and `duckdb-v*`); see `make help` for the publishing targets.
