# Stage 5c · Module layout — Implementation Plan

> Lean plan: a refactor with no behaviour change. The design is the partition below; the
> moves are mechanical. Four batches, one review per batch, an Opus triage at the end.

**Goal:** `spec.rs` and `project.rs` become folders of short files by responsibility, every core
module keeps its unit tests in a sibling file, and nothing a caller can see changes except the
two issues the stage closes (#64, #71).

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 5c · Estructura de módulos" (T37–T45; T45 approved with this plan).

## Global Constraints
- No behaviour change. Every commit passes `make gates`; batches touching Python also pass
  `make py-test`. No golden file changes; never run `UPDATE_GOLDEN`.
- Public API frozen: no item becomes public that was not, none disappears, no path changes.
  Cross-file visibility inside a folder uses `pub(super)` or `pub(crate)`, never `pub`.
- Code moves verbatim. Allowed edits while moving: `use` lines, visibility qualifiers,
  `//!` module headers, test-module wrappers and their indentation. Anything else is a defect.
- Comments describe implementation only: no stage numbers, principle codes, issue numbers or
  history.
- Size guide (T40): about 400 lines of code per file, tests excluded. A file over it needs a
  sentence in the report saying why it is one coherent piece.
- Fixtures and samples are never edited. `git add` named files.
- Test count only grows: record the baseline in Task 1 and compare after every batch.

## Review Focus
1. **Leaked visibility:** a helper made `pub` instead of `pub(super)` widens the API silently.
   The rustdoc item list (Task 1) must stay identical.
2. **Lost tests:** a test block dropped while moving still compiles. The per-binary counts
   (Task 1) must match exactly after each move.
3. **Changed logic hidden in a move:** a reviewer must be able to diff moved code against
   master. Each move commit contains moves only.
4. **Bench baselines:** criterion keys baselines by group name, not file. Renaming a group
   breaks comparison with every recorded baseline.
5. **`__version__` drift:** a hard-coded string would drift from the single version source.

---

## Batch A (Haiku → Sonnet review): guard rails and sibling test files

### Task 1: Baselines and the public-path test
- Record the baselines in the report and nowhere in the repo:
  - Per test binary, the counts from `cargo test --workspace --locked 2>&1 | grep "test result"`.
  - The pytest count from `make py-test`.
  - The sorted list of rustdoc files from
    `cargo doc -p edi835_core --no-deps && find target/doc/edi835_core -name '*.html' | sort`.
- Create `crates/edi835_core/tests/public_paths.rs`. It holds one `use` per public item, by its
  full module path: every `pub` item of every `pub mod` listed in `lib.rs`, plus every
  root re-export. Then add one `#[test] fn every_public_path_resolves() {}` that compiles only if
  every path exists. To build the list, take the item names from the rustdoc file list
  (`struct.X.html` and similar) rather than reading by eye.
- Commit: `test: name every public path of the core`.

### Task 2: Unit tests to sibling files, unsplit modules
For each of `check`, `column`, `delimiters`, `diagnostic`, `document`, `element`, `engine`,
`frame`, `process`, `segment`, `tokenizer`, `tree`:
- Replace the inline `#[cfg(test)] mod tests { … }` with `#[cfg(test)] mod tests;`.
- Move the body to `src/<module>/tests.rs`, de-indented one level.
- `column.rs` has three `#[cfg(test)]` items (lines 199, 206, 936). Move only the `mod tests`
  block. Leave test-only helpers that live in the main code where they are, and name them in
  the report.
- After each file, the test counts of that binary equal the baseline.
- One commit for all twelve files: `refactor: unit tests of the core in sibling files`.

## Batch B (Sonnet → Opus review): split `spec.rs`

### Task 3: `spec/` by responsibility
Line ranges are against `master` at 948a94b. Each new file opens with a one-line `//!`.

| File | Contents (current lines) |
|---|---|
| `spec/mod.rs` | module header (from `spec.rs`), `mod` declarations, `pub use` of every item the old file exported, `Spec` struct, and accessor `impl Spec` from `builtin_835` through `best_match` (1061–1246) |
| `spec/loops.rs` | `LoopId`, `Trigger`, `LoopDef`, `ControlCount`, `Control`, `ControlError` and its `Display` (30–141) |
| `spec/segments.rs` | `ElementType`, `ElementDef`, `SegmentDef`, `ElementDefError` and `Display` (142–328), `parse_position`, `compile_elements` (1878–1954) |
| `spec/tables.rs` | `Repeat`, `ColumnSource`, `TableDef`, `AnchorChains`, `TableDefError` and `Display` (329–607) |
| `spec/compile.rs` | `compile_tables`, `compile_table`, `compile_column`, `check_held`, `link_tables` (1450–1832) |
| `spec/build.rs` | `impl Spec { from_value, check_ambiguity }` (1247–1430), `compile_control`, `check_cycles` (1833–1862, 2294–2314) |
| `spec/error.rs` | `SpecError`, its `Display` and `Error` (608–963) |
| `spec/raw.rs` | the `Raw*` deserialization structs (964–1060) |
| `spec/shape.rs` | `section`, `kind_of`, `object_at`, `Leaf`, `check_leaf`, `check_member*`, `check_keys`, `check_shape`, `check_elements_shape` (1440–1449, 1955–2293, minus the renderers) |
| `spec/render.rs` | `render_chain`, `render_trigger`, `render_value`, `render_key`, `child` |
| `spec/patch.rs` | `merge_patch` (2315–2334) |

- Tests: the 2,647 test lines go to `spec/tests/`, one file per topic, with `spec/tests/mod.rs`
  holding the shared helpers (`segs`, `json_error` and any others). Topics follow the test
  names: loading and accessors, triggers and ambiguity, controls, segments and elements,
  tables, shape and paths, patch, and `Display` texts. Ruling: T38 says `tests.rs`. A folder is
  used here because a single 2,600-line test file would repeat the problem T38 solves.
- Where a range above disagrees with the file, follow the item names and note the difference.
  If an item fits two files, put it next to its only caller.
- Verify:
  - per-binary test counts equal the baseline;
  - the rustdoc file list is identical to Task 1's;
  - the public-path test passes;
  - no file under `spec/` exceeds the size guide without a sentence in the report.
- Commit: `refactor: split the spec module by responsibility`.

## Batch C (Sonnet → Opus review): split `project.rs`, bench files

### Task 4a: One module, one folder (T41, approved 2026-10-04)
- For each of the twelve modules with a sibling `tests.rs`, move `src/<m>.rs` to
  `src/<m>/mod.rs` with `git mv`. Contents stay unchanged.
- Enable `self_named_module_files = "deny"` under `[lints.clippy]` in
  `crates/edi835_core/Cargo.toml`, and check that clippy now fails if a `x.rs` is placed next
  to a `x/` folder.
- Fix the `expect` message in `spec/mod.rs` so it names the test at its current path,
  `spec::tests::loading::builtin_835_loads` (a parked minor from the batch B review).
- Commit: `refactor: one folder per core module`.

### Task 4: `project/` by responsibility
Line ranges are against 948a94b. `impl Projector` blocks may live in several child files,
because a child module sees its parent's private fields.

| File | Contents |
|---|---|
| `project/mod.rs` | header, `Projector` struct, `new`, `on`, `finish`, `take_tables`, the event handlers `opened`, `closed`, `captured`, `open_row` (196–415), `TableState`, `Row`, `Slot` |
| `project/plan.rs` | `ElementPlan`, `SegmentPlan`, `Plans`, `column_type` (102–180, 753–773) |
| `project/check.rs` | `impl Projector { check, check_value }` (416–556), `Parsed`, `Checked`, `parse` (55–77, 821–832) |
| `project/fill.rs` | `impl Projector { fill, segment_rows, segment_row, report_dropped, report, push_diagnostic }` (557–752), `matches`, `has_content`, `leaf_text`, `Place`, `read`, `Dropped`, `append` (774–end of code) |

- Tests go to `project/tests.rs`, or to `project/tests/` by topic if they pass about 800 lines.
- Verify as in Task 3. Commit: `refactor: split the projector by responsibility`.

### Task 5 (T45): one bench file per layer
`benches/tokenize.rs` holds all eight criterion groups. Split it, keeping every group name and
`BenchmarkId` byte for byte so recorded baselines still compare:
- `tokenize.rs`: `tokenize`, `index`
- `engine.rs`: `engine`, `engine_events`, `engine_fragment`
- `check.rs`: `check`
- `process.rs`: `process`, `process_rows`

Shared loaders go to `benches/common/mod.rs`, with one `[[bench]]` entry per file in
`crates/edi835_core/Cargo.toml`. Verify with `cargo bench --workspace --no-run --locked`. Run
`cargo bench -p edi835_core -- --list` and confirm the same set of ids as before.
- Commit: `bench: one bench file per layer`.

## Batch D (Sonnet → Opus review): issues and guides

### Task 6: #64 and #71
- **#64 `frame.rs`:** use one indexing style. `next_frame`'s indices are provably in range, so
  keep its slicing with a short comment saying why each index holds. Rewrite `first_frame` the
  same way and drop the unreachable fallback branch and the `after.get(after.len()..)` detour.
  Behaviour is identical, and the frame tests and goldens prove it.
- **#64 compat date error:** give it the `{file_path}: segment N …` prefix the other compat
  errors use, with no `#`. Update its full-text test.
- **#64 `IsaError::Truncated`:** when leading trivia was skipped (a byte order mark or
  whitespace), the message says so the way `NotIsa` already does. Add a full-text `Display`
  test for each case: with and without skipped bytes.
- **#71:** in `oxedi835/__init__.py`, set
  `__version__ = importlib.metadata.version("oxedi835")`. Add a pytest that checks it equals the
  metadata version and parses as PEP 440 with `packaging.version.Version`, or with a regex if
  `packaging` is absent.
- Commits: `fix: one indexing style in framing; truncation names skipped bytes` (Rust),
  `fix(py): compat date error prefix; expose __version__` (Python). The PR closes #64 and #71.

### Task 7: Guides and wrap-up
- `CLAUDE.md`, "Non-negotiables in code": add the size guide (about 400 lines of code per
  file, tests excluded, sibling `tests.rs` or `tests/` by topic) and the rule that a folder's
  `mod.rs` re-exports and states what each file holds.
- Check that `lib.rs`'s index still reads correctly, with no paths changed.
- Run `cargo bench --workspace` and compare against the last recorded baseline for
  `tokenize`, `engine`, `process`. Put the numbers in the commit message; a change beyond about
  ±5% gets a sentence in the report.
- Commit: `docs: module size guide; bench check after the split`.

## Exit gate (§7 Stage 5c)
- `make gates` and `make py-test` are green. Test counts equal the baseline plus the new
  public-path and #64 and #71 tests.
- The rustdoc file list is identical to Task 1's, and no golden changed.
- Benchmarks show no regression beyond noise.
- No code file under `spec/` or `project/` is over the guide without a written reason.
