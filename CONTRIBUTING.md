# Contributing

Thank you for helping. This page says how to start, how to test fast, the rules every change
keeps, and how to send it. [ARCHITECTURE.md](ARCHITECTURE.md) explains how the code is laid out.

## Start here

1. Read [ARCHITECTURE.md](ARCHITECTURE.md): the pipelines, where to change what, and the
   glossary of internal names.
2. Install the tools (see [Setup](#setup)), then run `make venv && make gates`. When it passes,
   your machine builds the Rust workspace and passes the commit gates. The Python, SQL and
   viewer checks are separate targets (`make py-test`, `make test_release`, `make viewer-test`).
3. While you work, run only the tests you need: see [the fast loop](#the-fast-loop).

## The fast loop

`make gates` runs every check. While you work, run only the part you touch:

| What | Command | Warm time |
|---|---|---|
| One core module | `cargo test -p oxedi_core --lib engine::` | under 1 s |
| One integration suite | `cargo test -p oxedi_core --test project_golden` | 2 s |
| One test by name | `cargo test -p oxedi_core --test write_findings a_value_holding_a_delimiter` | 1 s |
| The DuckDB crate (Rust) | `cargo test -p oxedi_duckdb` | 5 s |
| One Python test | `make py-dev && .venv/bin/pytest crates/oxedi_py/tests/test_spec.py -k version` | 5 s |
| The SQL tests | `make release && make test_release` | 40 s |
| The viewer's unit checks | `make viewer-units` | under 1 s |
| Before pushing | `make gates`, plus `make py-test` when `crates/oxedi_py` changed | a few minutes |

Times are for a machine that has built once. Some notes:

- `--lib <path>` filters the core's unit tests by module path (`engine::`, `project::`,
  `write::`); `--test <name>` runs one file of `crates/oxedi_core/tests/`. The suites are listed
  in [crates/oxedi_core/tests/README.md](crates/oxedi_core/tests/README.md).
- The first `cargo test -p …` after a workspace build compiles the crate again, once (about a
  minute), because a single package builds with fewer features than the workspace.
- `crates/oxedi_py` has no Rust tests (`test = false`): its tests are pytest, and `make py-dev`
  builds the extension first. `make py-dev` installs into the active virtual environment when
  one is active; deactivate any other one so it lands in `.venv`.
- The SQL tests need the DuckDB tools once: `git submodule update --init && make configure_ci`.
  See [DuckDB extension](#duckdb-extension).

## Code conventions

Each rule comes with an example from the code.

**Lossless.** Every input byte lands in exactly one segment's `raw`. Input the code does not
understand is data, an event or a diagnostic, never dropped. A segment no loop holds stays in the
document, as `Event::Unmatched` and a `Rule::UnknownSegment` diagnostic. From
`crates/oxedi_core/src/tokenizer/tests.rs`:

```rust
assert_eq!(segments[1].raw, b"\r\nSE*2*1~"); // the line break before SE is kept
assert_eq!(concat_raw(&segments), input);    // every raw, joined, is the input
```

**No panics on input.** No `unwrap`, `expect`, `panic!` or indexing that can fail on input
data in `src/`. Return an error, report a diagnostic, or skip with a reason. Indexing with ids
the code itself created (`LoopId`, `NodeId`) is fine. The only exception is the built-in specs
(`Spec::builtin_835`, `Spec::builtin_835_4010`), which `expect` because a test proves they
load. From `crates/oxedi_core/src/write/writer.rs`:

```rust
let Some(table) = data.get(reason.table) else { continue; };
```

**Errors explain themselves.** Every error and diagnostic names the rule that failed, where it
failed and the offending value. "Where" is the loop and key as written for a spec, and the
segment index, byte range, element and component position and loop path for a file. Each
variant has one test of its full `Display` text. `Display` reads well on its own and prints any
inner error in full. `source()` returns the inner error only when `Display` does not print it,
and one test per error type pins which (for example `no_spec_error_chains_its_source` in
`crates/oxedi_core/src/spec/tests/display.rs`). From
`crates/oxedi_core/src/diagnostic/tests.rs`:

```text
SNIP 1 · segment "XX" is not part of the structure: no open loop holds it and it opens no loop · segment #7 · at interchange#1/group#1/transaction#1/1000A#1 · datum "XX"
```

**The structure is data.** No code in the core is specific to the 835: the spec JSON holds
the structure. The Rust bindings (`crates/oxedi_py/src`, `crates/oxedi_duckdb/src`,
`crates/oxedi_wasm/src`) and the viewer never name an 835 segment; they ask the core. From
`crates/oxedi_wasm/src/view/mod.rs`:

```rust
Spec::select(&[builtin(), builtin_4010()], builtin(), document.segments())
```

**The core stays sans-IO.** `oxedi_core`'s `[dependencies]` are `serde`, `serde_json`, `hmac`
and `sha2`. Nothing that does I/O, uses threads or needs a runtime. Open an issue before adding
any dependency to the core. The Python binding holds the only `pyo3` and `arrow-*`
dependencies, and every binding delegates the work to the core.

**Comments describe the code, not its history.** Comments and doc comments say what the code
does and why. No ticket or planning references, no "added in", no internal codes. From
`crates/oxedi_core/src/process/mod.rs`:

```rust
// One segment buffer for the whole pass: each segment is parsed into the
// previous one's element and composite vectors.
```

**One folder per module.** A module of a crate is a folder `x/mod.rs`; its parts are files in
that folder, and its tests live in `x/tests.rs` or `x/tests/<topic>.rs`. Clippy's
`self_named_module_files` lint rejects `x.rs` next to `x/`. A folder's `mod.rs` re-exports and
lists what each file holds (see `crates/oxedi_core/src/write/mod.rs`). The Python binding's
`crates/oxedi_py/src/` is the exception: its modules are leaves tested from pytest, so each is
one file.

**Files stay small.** About 400 lines of code per file, tests excluded. A longer file needs a
reason it is one coherent piece.

**Positions say their base.** Element, component and loop ordinal are 1-based; a segment index
and a table row are 0-based. A new field says which in its doc comment. From
`crates/oxedi_core/src/diagnostic/mod.rs`:

```rust
/// 1-based element position inside the segment, when the finding is.
pub element: Option<usize>,
```

**One name per concept.** A binding type that mirrors a core type is `Py<Core>` in Python
(`PyDiagnostic`) and `<Core>View` in the viewer (`DiagnosticView`, `DelimitersView`). Do not
reuse a core type's name for a different shape. In prose, a "diagnostic" is a reading result and
a "finding" is a writing or external-validator result.

**A rename moves the map too.** A change that renames a public core item or moves a module
updates [ARCHITECTURE.md](ARCHITECTURE.md) and `crates/oxedi_core/tests/public_paths.rs` (which
names every public path) in the same commit.

**Every commit passes the gates.** CI runs the same commands:

```bash
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo bench --workspace --no-run --locked
```

`make gates` runs these four plus rustdoc with warnings as errors and `make readme-py-check`.

## Tests and test data

Test data comes in three kinds, all under `crates/oxedi_core/tests/`:

- **Fixtures** (`fixtures/`): small synthetic files. Never edited.
- **Samples** (`samples/`): real payer files, anonymized before they entered the repository.
  Never edited. The originals and the key that re-identifies them live outside the repository
  and are never committed.
- **Goldens** (`golden/`): generated expected output. Regenerate them, then read the diff:

  ```bash
  UPDATE_GOLDEN=1 cargo test -p oxedi_core --test engine_golden    # events per file
  UPDATE_GOLDEN=1 cargo test -p oxedi_core --test project_golden   # tables and diagnostics
  ```

A new fixture or sample goes in the lists of `crates/oxedi_core/tests/common/mod.rs`
(`FIXTURES`, `SAMPLES`). `tests/file_lists.rs` and the Python `test_file_lists.py` fail when a
file is missing from them. [crates/oxedi_core/tests/README.md](crates/oxedi_core/tests/README.md)
maps every suite to what it pins.

**Never attach a real 835 to an issue or a pull request.** It holds patient data (names, member
ids, claim numbers, service dates). Instead:

- share masked output: parse with `oxedi.Mask` (see
  [Sharing data without patient information](docs/python/without-patient-data.md)) and share
  the tables or the diagnostics. The mask hides only those: never share `result.document`,
  whose every `Segment` keeps the original bytes. In the viewer, the mask switch masks the
  tables, the diagnostics and the CSV export; its Text and Loops views still show the original
  file;
- or cut the file down to the few segments that show the problem and replace every name, id
  and date by hand.

## Pull requests

- Branch from `master`; keep one change per pull request.
- The body has two sections. **Intent**: what the change is for and why. **Verification**: how
  you checked it (the commands you ran, a golden diff you read). Do not list the diff; reviewers
  read it.
- Run `make gates` before pushing, and `make py-test` when `crates/oxedi_py` changed. CI runs
  only the areas your change touches; see [CI](#ci).
- A change users can see adds a line under `[Unreleased]` in `CHANGELOG.md` (or
  `crates/oxedi_duckdb/CHANGELOG.md` for the extension).
- The maintainer merges.

## Good first issues

Issues labelled `good first issue` have a **Pointers** section: where the code lives, which test
pins it and how to check it. Comment on the issue before starting, so two people do not take
the same one.

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
make stubs      # regenerate the native module's type stub (CI fails on drift)
make stubtest   # check the stub against the built module and the public API
```

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

## Browser viewer

The viewer (`viewer/`) is a static page that runs the core as WebAssembly (`crates/oxedi_wasm`,
an internal binding with no public API). It has no JavaScript build step: `make wasm` builds the
module and generates `viewer/pkg/` (git-ignored).

```bash
make wasm              # build the module into viewer/pkg
make wasm-size-check   # fail if the gzipped module is over 250 KB
make viewer-serve      # build, then serve at http://127.0.0.1:8001
make viewer-test       # unit checks (node), the oracle against oxedi.parse, the browser tests
```

Tools: the `wasm32-unknown-unknown` Rust target (`make wasm` adds it), the `wasm-bindgen` CLI at
exactly the version `make wasm-bindgen-version` prints (from `Cargo.lock`), `wasm-opt` from
binaryen (CI uses release 133), Node 24 for the oracle and unit checks, and Chromium for the
browser tests. `crates/oxedi_wasm/scripts/install_tools.sh` installs the two prebuilt tools with
pinned checksums; CI and the deploy use it. Bumping the crate's `wasm-bindgen` pin means bumping
the version and checksum in that script. The browser tests run in the docs environment
(`make docs-venv`); install the browser once with `.venv-docs/bin/playwright install chromium`.

The page's Content-Security-Policy forbids network connections, so it loads the module from
generated ES modules instead of fetching it. Do not add a request, a font or a script from
another origin.

## Documentation

`README.md` is for GitHub and links to the guides under `docs/` with relative links.
`crates/oxedi_py/README.md` is the PyPI page and has the same content with absolute
`https://github.com/javillegasna/oxedi/blob/master/...` links, because relative links break on
PyPI. Change the root `README.md` and regenerate the other with `make readme-py`; `make gates` and CI
run `make readme-py-check`, which fails when the two differ.

## Documentation site

The site is built with MkDocs Material from `mkdocs.yml` and `docs/`. Its tools live in
`docs/requirements.txt`, in their own environment (`.venv-docs`), never in the package.

```bash
make docs         # strict build into site/: a broken link or a page outside the navigation fails
make docs-serve   # preview at http://127.0.0.1:8000
make release      # build the DuckDB extension once; the SQL examples load it
make docs-test    # run every python and sql block of docs/**/*.md and README.md
```

The API reference under `docs/reference/api/` is generated from the installed package and its type stub;
write a symbol's documentation in its docstring, not in the page.

`make docs-test` runs the blocks of a page in order, in one namespace (Python) or one DuckDB
connection (SQL), inside a temporary directory that holds `remittance.835`, `remittance-4010.835`, `big.835`, `a.835`,
`b.835`, `remits/*.835` and `remittances/*.835` (copies of the files under
`crates/oxedi_core/tests/fixtures/`). A block that cannot run carries a comment on the line
right before its fence, with a reason; the run lists every skipped block:

    <!-- docs-test: skip needs the reader's own tables -->

Use it for blocks that need something the runner cannot provide (a network, an orchestrator,
dbt templating), and when the equivalent runs elsewhere on the page, say so in the reason. A
`python` or `sql` block without the marker must run.

To add a page, put it in `docs/` and in the `nav` of `mkdocs.yml`; the strict build rejects a
page outside the navigation.

### Deploy

`.github/workflows/docs.yml` publishes with `mike` to the `gh-pages` branch. A push to `master`
that changes what the site or the viewer is built from (the `paths` of the workflow) publishes
`dev` (titled "dev (unreleased)"). A final tag `vX.Y.Z` publishes version `X.Y` and moves the `latest` alias, which
the site root redirects to. Pre-release tags and `duckdb-v*` tags publish nothing. Pull requests
only build the site (the Docs job in `ci.yml`). To publish a release that was tagged before the workflow existed, run "Docs deploy" manually
from `master` with the `version` input (`X.Y`, for example `0.5`); it also moves `latest`.
The same workflow publishes the viewer next to the versions: a push to `master` puts it in
`viewer/dev/`, and a final tag (or the manual run) puts it in `viewer/`, which keeps `viewer/dev/`.
It then checks that `versions.json`, every version directory and the viewer are still on
`gh-pages`. Never commit to `gh-pages` by hand.

## CI

`ci.yml` runs six areas, each a reusable workflow (`_rust.yml`, `_python.yml`, `_duckdb.yml`,
`_docs.yml`, `_viewer.yml`, and `_lint.yml`, which lints the workflows and their scripts), only
when files that area depends on changed: a pull request is compared with its base, a push to
`master` with the previous commit. The `Changes` job holds the
path filters; a change under `.github/` runs Rust, and a change to the composite actions in
`.github/actions/` runs every area. A draft pull request runs Rust only; the other areas start
when it is marked ready for review. A new push to a pull request cancels its running checks.

`nightly.yml` runs every area on `master` each night, whatever changed, plus the DuckDB
pre-release leg (`next`), which reports and does not block. Run it by hand from the Actions tab
when a change needs the full set before merging.

`CI result` is the one check to require: it fails when an area failed or was cancelled and
passes when each one passed or was not needed. Caches are written only from `master`;
pull requests read them. The checks of a push to `master` are never cancelled or replaced by
a later push.

## Releases

`make release-check TAG=v<version>` checks the tag, a clean tree and the changelog entry before
a release. The Python package and the DuckDB extension are versioned and tagged separately
(`v*` and `duckdb-v*`); see `make help` for the publishing targets.

## Complexity

**Targets.** Per function: cyclomatic complexity (CCN) of 6–7, nesting no deeper than 3, at
most 8 branches. Flat per-variant dispatch (a big `Display` or kind `match`) is the expected
exception. No pinned tool counts branches apart from CCN, so the 8-branch limit is checked in
review.

**The ratchet.** Today's code is the baseline: nothing may get worse, and a function over a
target may only shrink. Two checks hold that line:

- Clippy (`make clippy`) caps cognitive complexity, nesting, function length and argument count
  at today's largest values, set in `clippy.toml`. Lower a ceiling when a refactor brings the
  largest function down. A Rust toolchain bump may need them measured again.
- `make complexity` (needs uv) measures CCN and nesting of every Rust, Python and JS function
  outside tests, benches, examples and tooling, with a pinned lizard (Python nesting with the
  `ast` module). It compares the functions over a target (CCN above 7, nesting above 3) with
  `.github/complexity-baseline.txt`. CI runs it in the Workflows area (see [CI](#ci)).

**What it reports.** Every line says how to resolve it:

- `FAIL`: a function is over a target and not in the baseline, or worse than its entry.
  Simplify it.
- `STALE`: an entry is better than recorded, or its function is within the targets or gone.
  Run `make complexity-update` and commit the baseline with the refactor, so the baseline
  shrinks.
- `MOVED`: a file was moved or renamed, and a function in it matches its old entry, no worse.
  Run `make complexity-update` to record the new path.

**Reviewed exceptions.** Adding a variant to a flat dispatch, such as one more arm in an
error's `Display`, raises its CCN by one and is acceptable. Record it with
`make complexity-update ACCEPT='path::function'` (several separated by spaces) and say why in
the pull request's **Intent**. Nothing else is accepted this way: split the function instead.
On a pull request, CI lists every added or raised entry in the job summary, and changes to the
baseline or `clippy.toml` need the maintainer's review (`.github/CODEOWNERS`).
