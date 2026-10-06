# Stage 7b · DuckDB write (`COPY … TO … (FORMAT edi835)`) — Implementation Plan

> Lean plan: design-heavy tasks to Opus with contracts, not transcribed code. Batches of two
> tasks, one review per batch, Opus triage at the end.

**Goal:** the `oxedi` DuckDB extension registers a `COPY` format `edi835` that takes the spec's
tables (one `LIST<STRUCT>` column per table) and writes an 835 with the core writer, byte for
byte equal to `oxedi.write` with the same envelope.

**Architecture:** a new module in `crates/oxedi_duckdb` registers a copy function through the
stable C API (`duckdb_create_copy_function`, `_set_bind`, `_set_global_init`, `_set_sink`,
`_set_finalize`, `duckdb_register_copy_function`). Bind reads the options (envelope, version) and
checks the input schema against the spec; sink converts each chunk's lists of structs into the
core's `Table` rows; finalize calls `oxedi_core::write` and writes the bytes through DuckDB's file
system (`duckdb_file_system_open`, `duckdb_file_handle_write`). The core and the Python package do
not change.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 7b" (T88–T93), approved 2026-10-06.
Spike: `.doc/spikes/duckdb-extension.md` §4 (the `oxedi835_probe` copy format on branch
`spike-duckdb-prototype`, readable with `git show spike-duckdb-prototype:<path>`; a starting point
only). Branch `stage-7b-duckdb-write`. Ledger `.superpowers/sdd/stage-7b/progress.md`.

## Global Constraints
- `CLAUDE.md` non-negotiables apply: no `unwrap`/`expect`/`panic!` or fallible indexing on input;
  `unsafe` only for raw C API calls, each block with a `// SAFETY:` comment, every callback
  catching panics; comments implementation-only; one folder per module (`x/mod.rs`, tests in
  `x/tests.rs`), ~400 code lines per file; P10 errors (rule, place, datum) with one full-text test
  per message (Rust `Display` test and one SQLLogicTest per user-visible message).
- No global state: everything lives in the copy function's extra info, bind data and global state.
  No `std::fs`; all output through DuckDB's file system.
- The core gets no new dependency. A core change is allowed only if it removes a duplication with
  `crates/oxedi_py/src/import.rs` (for example the table/column name checks and their messages)
  and leaves every Python message and test unchanged; record the ruling in the ledger.
- No new crate dependency in `oxedi_duckdb` (in particular no `arrow-*`).
- Conversion rules from DuckDB values to the spec's column types are those of
  `crates/oxedi_py/src/import.rs` (text and blobs to bytes; integers to integers; decimals rescaled
  exactly, floats refused for decimals; dates; times; a value that would change is refused with
  table, column and row). Messages for unknown tables or columns read exactly as Python's.
- Option names and defaults exactly as `oxedi.Envelope` (T90); `date` and `time` required;
  `version` values as `read_835` (T92). Strict only (T91): any finding fails the `COPY` with the
  full findings listing, the same text `oxedi.WriteError` shows, and nothing is written.
- Gates on every commit: `make gates`; the extension's tests (`make test_release` after
  `make release`) and `make duckdb-oracle`.

## Review Focus
1. **Byte oracle.** For every sample and fixture that `oxedi.write` accepts, both versions,
   `COPY` from `read_835`'s tables equals `oxedi.write` byte for byte with the same envelope.
2. **Nothing written on failure.** Strict refusals, conversion errors and option errors leave no
   file (and do not truncate an existing one) — check when the file handle is opened.
3. **Input shape edge cases:** a table column absent from the query, an empty list, a NULL list,
   NULL struct fields, several rows concatenated, a column that is not `LIST<STRUCT>`, a struct
   field not in the spec, `DECIMAL` of a different scale, `INTEGER`/`HUGEINT` for `BIGINT`,
   `VARCHAR` vs `BLOB`.
4. **No panic crosses FFI**, no state shared between two databases in one process.
5. **Caller context:** `TEMP` tables, a view, and the caller's open transaction are visible.

---

## Batch A (Opus → Opus review): the format

### Task 1: registration, options and the input schema (bind)
- Register copy function `edi835` beside `read_835` (update the crate docs in `lib.rs` and the
  `mod.rs` file lists).
- Bind: parse the options from `duckdb_copy_function_bind_get_options` into an `Envelope` and a
  spec choice (T90, T92): required `sender_id`, `receiver_id`, `date` (DATE or ISO text), `time`
  (TIME or `HH:MM`/`HHMM` text, as `oxedi.Envelope` accepts); optional fields with Python's
  defaults; `delimiters` in the same form Python takes; `line_break` boolean. P10 errors: unknown
  option (with the valid list), missing required option, wrong type or value (option, value,
  expected), unknown version (valid values), and the file options that do not apply
  (`PARTITION_BY`, `PER_THREAD_OUTPUT`, `COMPRESSION`, others DuckDB passes: find out exactly
  which reach the bind and which DuckDB rejects itself; test what reaches us).
- Input schema (T89): every input column must be named after a spec table (message as Python's),
  of type `LIST(STRUCT(...))`; every struct field must be a column of that table (message as
  Python's) with a DuckDB type convertible to the column's type (P10 error naming table, field,
  received type and expected type). Unconvertible types fail at bind, before any row is read.
- Rust unit tests for option parsing and schema checks (full-text messages); one SQLLogicTest per
  message.
- Commit: `feat(duckdb): edi835 copy format options and input schema`.

### Task 2: rows, write and output (sink, finalize)
- Sink: read each chunk's list vectors (offsets/lengths, child struct vectors, validity) and
  append one core row per struct to that table, concatenating across input rows and chunks (T89).
  Value conversion as in Global Constraints; a value refused at conversion is a P10 error naming
  table, column, row (row within that table) and value. NULL list or NULL struct contributes no
  row; NULL field is a null cell.
- Finalize: build `Tables`, call the core `write` with the chosen spec and envelope; on
  `WriteError` fail with its full text (T91); on success open the target through DuckDB's file
  system only now (so a failure never creates or truncates the file), write all bytes, close,
  and report I/O errors with path and cause (T93).
- SQLLogicTests: round trip of one 5010 and one 4010 fixture through `read_835` → `COPY` →
  `read_835` (tables equal apart from `segment`); a `TEMP` table source; a view; several rows via
  `UNION ALL`; a strict refusal (a fixture `oxedi.write` refuses) with its full listing and no file
  written; an existing file left untouched on failure.
- Commit: `feat(duckdb): edi835 copy format writes through the core writer`.

## Batch B (Sonnet → Opus review): oracle, docs, version

### Task 3: byte oracle and refusal parity
- Extend the extension's Python oracle (`crates/oxedi_duckdb/test/python/`, run by
  `make duckdb-oracle` and CI): for every sample and fixture, both versions, write with
  `oxedi.write(result.tables, envelope)` and with `COPY` from `read_835`'s tables using the same
  envelope options; files `oxedi.write` accepts must be byte-identical; files it refuses must fail
  in `COPY` with an error containing the same findings text.
- Commit: `test(duckdb): edi835 copy equals oxedi.write on every sample`.

### Task 4: docs, changelog, version
- Extension README and root README: a short "Writing" section with the query pattern of T89, the
  options table, strict behaviour, `version`. Crate docs in `lib.rs`.
- `crates/oxedi_duckdb/CHANGELOG.md` `## [0.2.0]` (Added: the `edi835` format); crate version and
  `description.yml` version 0.2.0 (`make duckdb-version-check` green); `description.yml` docs
  `hello_world` gains a `COPY … (FORMAT edi835 …)` line if it stays short.
- `.doc/state.md` and `.doc/roadmap.md`: 7b in PR with gate results.
- Commit: `docs(duckdb): edi835 copy format; extension 0.2.0`.

## Exit gate (§7 Stage 7b)
- Byte oracle green on all accepted samples and fixtures, 5010 and 4010; refusals identical.
- One test per message; `TEMP`, view and `UNION ALL` sources tested.
- Loads on DuckDB stable (≥ 1.5.6) and `next` (CI matrix).
- `make gates`, `make test_release`, `make duckdb-oracle` green; core and Python unchanged (or a
  ledgered de-duplication with Python messages unchanged).
- After merge: issue on Project #8 for `allow_findings` from DuckDB (T91).
