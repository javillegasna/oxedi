# Stage 5d · Pipeline performance — Implementation Plan

> Lean plan: the tasks are design work on hot paths, so they are specified by contract,
> constraint and measurement, not by transcribed code. Opus implements and Opus reviews.
> Each candidate is measured on its own and kept only if it pays (T50).

**Goal:** `process` reaches at least 50 MiB/s on `edi835_test_united.rmt` and
`edi835_test_versant.RMT` (it measures about 33 today), and a `Document` index costs 8 bytes
per segment. No output changes.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 5d · Rendimiento del pipeline"
(T46–T51). Issues #39 and #45 carry the profile and the measurements.

## Global Constraints
- No output changes:
  - goldens are untouched and never regenerated;
  - every cargo test and pytest passes, and the `edi_835_parser` parity tests pass;
  - `scripts/compat_oracle.py` is not needed, because the samples cover it.
- Public API: only the changes T47 names, which are `Document`'s span access and its size
  error. `Segment`, `Tokenizer`, `Table`, `Column`, `ColumnData`, `Cell` and the projector's
  public methods keep their shape. `tests/public_paths.rs` is updated only for T47's items,
  and every update is listed in the report.
- No `unwrap`/`expect`/`panic!` or fallible indexing on input in `src/`. An index that is
  provably in range gets a one-line comment saying why, which is the style `frame/mod.rs` uses.
  No `unsafe`.
- Comments describe implementation only. Errors follow P10:
  - they name the rule, the place and the datum;
  - each variant has one full-text `Display` test;
  - `source()` chains.
- Layout rules from CLAUDE.md: one folder per module, about 400 code lines per file, and tests
  in sibling files.
- Every commit passes `make gates`. Commits that touch the binding also pass `make py-test`.
- **Measurement protocol (T50).** Task 1 saves the criterion baseline `master` once. Every
  later candidate runs:
  - `cargo bench -p edi835_core --bench process -- --baseline master`
  - `--bench engine` when the tokenizer or `Document` changed

  Nothing else heavy may run while benches run. The commit message body carries the
  `process` and `process_rows` change for united and versant. A candidate that does not gain
  at least 3% on `process` is reverted. The reverted attempt is recorded in the report with
  its numbers, and nothing is committed for it.
- **Stop rule (T46).** After each candidate, if `process` is at least 50 MiB/s on both large
  samples, skip the remaining candidates and go to Task 7.

## Review Focus
1. **Derived spans at the edges:**
   - a final segment with no terminator;
   - trailing trivia only;
   - an empty input;
   - a byte order mark;
   - `~~`;
   - a fragment without an ISA (`Document::with_delimiters`).

   Span derivation must reproduce today's `Span` values exactly for every fixture and sample.
2. **Buffer reuse leaking state:** a reused element buffer that keeps data from the previous
   segment, such as a stale composite or a shorter segment after a longer one, changes
   events silently.
3. **Diagnostics parity in the check path:** validating without building the parsed value
   must emit the same level-2 diagnostics, with the same text, positions and order.
4. **Column buffers on error paths:** a value that fails its type check after its text was
   written straight into the column buffer must leave the column exactly as today, with a
   null bitmap bit and no stray bytes.
5. **Noise:** a 3% gain on one run can be noise. Accept a candidate only when criterion's
   confidence interval for the change excludes zero.

---

## Batch A (Opus → Opus review): baseline and compact spans

### Task 1: Baseline
- On this branch, whose code equals `master` at the start, run
  `cargo bench -p edi835_core -- --save-baseline master`, bench by bench (`--bench tokenize`,
  `engine`, `check`, `process`). The lib harness rejects baseline flags.
- Measure the index memory of a `Document` for united, versant and eyemed:
  `spans.len() * size_of::<Span>()` against `as_bytes().len()`.
- Record both in `.superpowers/sdd/stage-5d/baseline.md`, which is not committed. Nothing is
  committed in this task.

### Task 2: Compact spans (T47)
- `Document` stores, per segment, the body start and body end as `u32`, 8 bytes per segment,
  in one `Vec`. Everything else is derived:
  - `raw.start` is the previous segment's `raw.end`, or 0 for the first;
  - `raw.end` is `body.end + 1` when terminated, else `body.end`;
  - `terminated` is true for every segment but the last, and for the last it is
    `body.end < len`.

  Before relying on these rules, verify each one against `frame/mod.rs` and the tokenizer,
  including the trailing-trivia frame. If any rule fails on some input, stop and report: the
  §7 decision rests on them.
- The public `Span` type stays, with its `usize` ranges and `terminated`, and is computed on
  demand. `Document::spans() -> &[Span]` is replaced by:
  - `Document::span(index) -> Option<Span>`;
  - an iterator of `Span`s, `Document::spans()` returning `impl Iterator<Item = Span> + '_`,
    or a named type if lifetimes need it.

  Update the callers: `diagnostic/mod.rs` and the tests that use `.spans()`.
- **Size limit.** A `Document` over `u32::MAX` bytes cannot be indexed. Today
  `with_delimiters` cannot fail, so this changes its signature. Design the error per P10:
  - it names the rule, the length and the limit;
  - it has a full-text `Display` test;
  - `source()` chains when it wraps `IsaError`.

  Keep `parse` returning one error type that callers can match. Update the binding
  (`crates/oxedi835_py/src/document.rs`) so Python sees the same message through its existing
  error class. The test must not allocate 4 GiB: factor the check so it can be unit-tested on a
  length value.
- Add a test that fixes `size_of` of the stored per-segment entry at 8.
- Add a property-style test, or extend `tests/document_props.rs`: for every fixture and sample,
  and for the edge inputs in Review Focus 1, the derived spans equal the spans the previous
  algorithm produced. Keep the old computation as a test-only reference function.
- Measure the index memory again, and run `--bench engine` and `--bench process` against
  `master`.
- Commit: `perf: compact document spans, 8 bytes per segment`. The body holds the memory
  before and after for the three samples, and the bench changes.

## Batch B (Opus → Opus review): cheaper segments, then the check path

### Task 3: Segment buffers reused in the internal pass (T48)
- Today `Segment::parse` allocates on every segment:
  - `split_raw` builds a `Vec` of pieces;
  - `elements` is a fresh `Vec`;
  - each composite element allocates its own `Vec<Value>`.
- Add a crate-internal way to parse a frame into an existing `Segment`, reusing the
  `elements` capacity and, where the types allow, the composite vectors. Use it in
  `Processor::run` and in `Document`'s iteration path when the caller is `Processor::run`.
  `Segment::parse`, `Tokenizer` and `Segments` keep their public behaviour.
- If reusing composite vectors needs a public type change, do not make it: reuse only
  `elements` and the split buffer, and report what is left on the table.
- Review Focus 2 applies. Add a test that parses a long segment, then a short one, then a
  composite one, then a simple one, all into the same buffer. Each result must equal
  `Segment::parse` of the same frame.
- Measure, keep or revert (T50), and check the stop rule.
- Commit: `perf: reuse segment buffers in the processor pass`.

### Task 4: Check without parsing unread elements (T49, candidate 1)
- In `project/check.rs`, elements that no column reads are fully parsed, building a `Parsed`
  value, only to validate them. Validate their length, charset and type shape without building
  the value. The diagnostics must be identical: same rule, text, positions and order. The
  goldens prove this, together with the projector tests.
- Elements that a column reads keep today's path, because their parsed value is reused.
- Measure, keep or revert, and check the stop rule.
- Commit: `perf: validate unread elements without building their value`.

## Batch C (Opus → Opus review): column writes (only if the gate is not reached)

### Task 5: Text straight into column buffers (T49, candidate 2)
- Today a row gathers `Cell`s in a per-row vector and copies `row.bytes` into the binary
  column. Write text values directly into the column's value buffer and offsets. Typed values
  write directly into their typed buffers. Remove the per-row cell vector where possible.
- Review Focus 4: a value that fails after a partial write must roll the column back to its
  previous length, offsets and bitmap.
- Measure, keep or revert, and check the stop rule.
- Commit: `perf: write projected values straight into column buffers`.

### Task 6: One-pass columnar append (T49, candidate 3)
- `push_row` checks every cell, then pushes. Append per column and type in a single pass,
  keeping the all-or-nothing row semantics through the rollback from Task 5.
- Measure, keep or revert.
- Commit: `perf: single-pass row append`.

## Batch D (Sonnet → Opus final triage): close-out

### Task 7: Close-out
- If the gate is not reached, open one issue on Project #8 (Backlog, priority Media) in
  Context / Problem / Recommendation form. It holds the final numbers, the remaining profile
  and the next candidates, including the lazy `Segment` that §7 left out of scope.
- Every candidate that was skipped because the gate was reached also goes in one issue, with
  priority Baja.
- Record the final `process` numbers for all samples, and the index memory, in the commit
  message.
- `.doc/state.md` and `.doc/roadmap.md` mark 5d done or in PR, with the numbers.
- Commit: `docs: Stage 5d results`.

## Exit gate (§7 Stage 5d)
- `process` reaches at least 50 MiB/s on united and versant. Otherwise the numbers reached are
  recorded and the next step is an issue.
- Index at most 8 bytes per segment, proven by test.
- No output changed: goldens, cargo tests, pytest and parity all pass.
- `make gates` and `make py-test` are green.
