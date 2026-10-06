# Stage 5f · DuckDB extension (read) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Lean plan:
> design-heavy tasks go to Opus with contracts, not transcribed code; the CI/docs task to Sonnet.
> Batches of two tasks, one review each, Opus triage at the end.

**Goal:** a DuckDB community extension `oxedi` whose `read_835(...)` returns the spec's tables and
the diagnostics, row-for-row equal to `oxedi835.parse_file`, loadable on DuckDB ≥ 1.5.6.

**Architecture:** a new workspace crate `crates/oxedi_duckdb` (cdylib, `libduckdb-sys` with the
`loadable-extension` feature, stable C API, ABI `C_STRUCT`) that depends on `edi835_core` and
does all I/O through DuckDB's file system. The core does not change.

**Tech stack:** Rust 2024, `duckdb-rs` (pinned in `Cargo.lock`), DuckDB's `extension-ci-tools`,
SQLLogicTest, Python `duckdb` for the oracle comparison.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 5f" (T62–T69), approved 2026-10-05.
Spike: `.doc/spikes/duckdb-extension.md`; prototype on branch `spike-duckdb-prototype` under
`spike/duckdb/rust_ext/` (`src/lib.rs`, `Makefile`, `test/sql/read_835.test`) and
`spike/duckdb/scripts/compare.py` — read with `git show spike-duckdb-prototype:<path>`; it is a
starting point, not code to copy blindly (it uses `std::fs` and a process-global connection, both
ruled out by T68/T69). Ledger: `.superpowers/sdd/stage-5f/progress.md`.

## Global Constraints
- `CLAUDE.md` non-negotiables apply to the new crate: no `unwrap`/`expect`/`panic!` or fallible
  indexing on input; `unsafe` for the raw C API calls (owner-ratified), each block with a `// SAFETY:`
  comment and every callback catching panics; comments describe implementation only; one folder per module
  (`x/mod.rs`, tests in `x/tests.rs`), ~400 code lines per file; P10 errors (rule, place, datum)
  with one full-text test per message.
- The core (`crates/edi835_core`) gets no new dependency and no behaviour change; the Python
  binding is untouched.
- Names: extension `oxedi`, crate `oxedi_duckdb`, function `read_835` with exactly the
  parameters of T63: `table_name` (default `'claims'`), `filename` (default `false`), `version`
  (default NULL), `binary` (default `false`), `ignore_errors` (default `false`).
- Type mapping from the core's Arrow schema: `int64` → BIGINT; `binary` → VARCHAR (BLOB with
  `binary := true`); `date32` → DATE; `decimal128(38, s)` → DECIMAL(38, s); any other type the
  core emits is a compile-time-visible match arm, never a silent fallback.
- Gates on every commit: `make gates` (the workspace now includes the crate) plus the extension's
  own test target once it exists.

## Review Focus
1. **Oracle equality.** Every table and `diagnostics`, every sample and fixture, equals
   `oxedi835.parse_file` row for row (md5 per table), including NULLs, decimals and dates.
2. **No global state, no `std::fs`.** Two databases in one process do not share a connection;
   remote paths go through DuckDB's file system.
3. **Error paths are P10 and never panic across the FFI boundary**: invalid UTF-8 cell, unparsable
   file with and without `ignore_errors`, unknown `table`, unknown `version`, empty glob.
4. **Root Makefile compatibility with the community CI**, which runs `make configure_ci`,
   `make release` and `make test_release` from the repository root.
5. **ABI pin.** The built binary loads on DuckDB stable (≥ 1.5.6) and on `next`; `duckdb-rs`
   upgrades are gated on its header having no unstable tail that we use.

---

## Batch A (Opus → Opus review): scaffold and tables

### Task 1: crate, build and root-Makefile wiring
- Create `crates/oxedi_duckdb` as a workspace member from the Rust extension template as the spike
  did (`USE_UNSTABLE_C_API=0`, ABI `C_STRUCT`, target DuckDB v1.5.6), with `extension-ci-tools`
  as a git submodule at the path the template expects.
- **Settle Review Focus 4 first**: find out exactly how `duckdb/community-extensions` CI invokes
  the build for a repo (read its workflow and a Rust extension such as `rusty_sheet`), and make
  our root `Makefile` expose `configure_ci`, `release`, `debug`, `test_release`, `test_debug`
  (whatever the CI calls) by delegating to the crate, without breaking `make help` or the existing
  targets. If the CI cannot work from a subdirectory at all, STOP and report BLOCKED with the
  evidence; do not restructure the repository.
- The extension registers `read_835` with the T63 parameters, state in the function's
  `extra_info` (no `OnceLock`/globals).
- `make gates` still green (clippy over the new crate); `make release` builds
  `oxedi.duckdb_extension`; a smoke SQLLogicTest loads it and reads one fixture.
- Commit: `feat(duckdb): oxedi extension crate and build wiring`.

### Task 2: `read_835` tables through DuckDB's file system
- Bind: resolve `path` (string, list or glob) through DuckDB's file system (`duckdb_file_system`
  / glob API in the stable C API — confirm the exact functions in the header), one file at a time;
  read bytes; select the spec by declared version (core `Spec::select` over the built-ins) unless
  `version` is given; parse with the core; produce the requested `table` with the type mapping in
  Global Constraints; `filename := true` adds a VARCHAR `filename` column last.
- Scan: emit rows in DuckDB vector-sized chunks; memory bounded by one file's tables.
- Tests: SQLLogicTest for each table on one 5010 and one 4010 sample (row counts and a few exact
  values), list and glob inputs, `filename`.
- Commit: `feat(duckdb): read_835 tables`.

## Batch B (Opus → Opus review): diagnostics, errors, oracle

### Task 3: diagnostics and error paths
- `table := 'diagnostics'` with the T67 columns (`level`, `kind`, `rule`, `segment`, `element`,
  `component`, `path`, `datum`, `origin`, `code`, plus `filename`).
- T64: a non-UTF-8 cell in a VARCHAR column fails the query with a P10 message naming file,
  table, column, row and the bytes, and suggesting `binary := true`; with it, those columns are
  BLOB. T65: unparsable file → P10 error naming the file, rule and datum; with
  `ignore_errors := true` it becomes one `diagnostics` row and the scan continues. Unknown `table`
  or `version`, empty glob: P10 errors listing the valid values.
- No panic crosses the FFI boundary (catch at the entry points; test with a deliberately failing
  input).
- One SQLLogicTest per message, full text.
- Commit: `feat(duckdb): diagnostics and error paths`.

### Task 4: oracle equality test
- A Python test (in the crate, run by its test target and by CI) that loads the built extension
  in the `duckdb` Python client (`allow_unsigned_extensions`) and compares, for every sample and
  fixture and for every table plus `diagnostics`, `read_835(...)` against `oxedi835.parse_file`
  (md5 per row in a canonical rendering; decimals and dates compared exactly). Reuse the spike's
  `compare.py` idea.
- Commit: `test(duckdb): read_835 equals parse_file on every sample`.

## Batch C (Sonnet → Opus review): CI, descriptor, docs

### Task 5: CI, community descriptor, README, state
- CI job (Linux) that builds the extension and runs its tests against DuckDB stable and `next`;
  the existing jobs unchanged.
- `description.yml` (in the crate or repo root, as the community repo expects) with name `oxedi`,
  version from the workspace, `language: Rust`, `build: cargo`, license, maintainer, repo/ref
  placeholder, `excluded_platforms` for musl and WASM, docs `hello_world` using `read_835` and
  `COPY ... TO 'x.parquet'`.
- README (root and the crate) short DuckDB section: `INSTALL oxedi FROM community; LOAD oxedi;`,
  examples, parameters, the VARCHAR/`binary` rule and `ignore_errors`.
- `.doc/state.md`, `.doc/roadmap.md`: 5f in PR with results.
- Commit: `ci: build and test the DuckDB extension; community descriptor; docs`.

## Exit gate (§7 Stage 5f)
- Oracle equality on all samples, fixtures, tables and diagnostics.
- One test per error message.
- Loads on DuckDB stable ≥ 1.5.6 and `next`.
- `make gates` green; core without new dependencies.
