# Stage 7 · Writer — Implementation Plan

> Lean plan: design-heavy tasks to Opus with contracts, not transcribed code. Batches of two tasks,
> one Opus review per batch, Opus triage at the end. Golden checkpoints: the implementer stops
> before any `UPDATE_GOLDEN` and the controller reviews every change with the owner.

**Goal:** `write(spec, tables, envelope)` turns tables with the spec's schema into a valid 835
(5010 and 4010), checked before emitting; balancing rules are spec data, checked on write and
reported as SNIP 3 diagnostics on read; Python exposes `oxedi.write`.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 7 · Escritor" (T81–T87), approved
2026-10-06. Builds on Stage 7a (occurrences, positions, usage, max, qualifiers, column sources by
occurrence/ancestor/pick). Branch `stage-7-writer`. Ledger `.superpowers/sdd/stage-7/progress.md`.

## Global Constraints
- `CLAUDE.md` non-negotiables: the core stays sans-IO (bytes out, no clock, no files) with only
  `serde`/`serde_json`; no `unwrap`/`expect`/`panic!` or fallible indexing on input in `src/`; no
  code names an 835 segment (envelope segments, balancing rules and qualifiers are spec data or
  generic X12 envelope knowledge expressed through the spec's `control` sections); P10 errors and
  diagnostics with one full-text `Display` test per variant; comments implementation-only; ~400
  code lines per file; one folder per module.
- The writer never changes money: balancing rules are checked, never used to fill values.
- Strict by default: any finding that prevents a valid file returns an error with every
  diagnostic and no bytes; `allow_findings` returns bytes plus diagnostics.
- Goldens change only after a per-item review written by the implementer and approved by the
  owner (stop with NEEDS_CONTEXT at each golden checkpoint). Existing table cells never change;
  new columns in built-in tables are allowed and reviewed.
- `edi-835-parser` parity stays 113/113; the DuckDB extension tests and oracle stay green.
- Gates on every commit: `make gates`; batches touching Python or the extension also
  `make py-test` (with and without pyx12), `make stubs` + no diff, `make stubtest`,
  `make test_release`, `make duckdb-oracle`.

## Review Focus
1. Round trip: `parse` → tables → `write` → `parse` gives the same tables on the columns the writer
   consumes, for every sample and valid fixture, both versions; no new diagnostics.
2. Generated files pass `pyx12` validation with zero findings (5010 and 4010).
3. Inversion edge cases: occurrences with a single qualifier code emitted automatically, multi-code
   qualifiers from their column, composite elements and components, `""` (written empty) vs `null`
   (absent) per T80, trailing empty elements trimmed, repeats in order with `pick`.
4. Envelope correctness: ISA fixed widths, control numbers matching (ISA13/IEA02, GS06/GE02,
   ST02/SE02), counts, delimiters never appearing inside data (a data value containing a
   delimiter is a diagnostic, not silent corruption).
5. Strictness and P10: every refusal names table, row, column, occurrence and value.

---

## Batch A (Opus → Opus review): rules and writability

### Task 1: balancing rules in the spec + SNIP 3 on read
- Spec format for declarative balancing rules (T84): a rule names a target value and a signed sum
  of values grouped by a loop instance (e.g. per transaction, per claim, per service), each value
  addressed by loop + occurrence + element (component), with a tolerance of zero on decimal
  amounts. Load-time validation with P10 `SpecError` variants.
- The three 835 rules in both built-in specs (5010; 4010 via the patch if it differs):
  BPR02 = Σ CLP04 − Σ PLB adjustment amounts (PLB04/06/08/10/12/14) per transaction;
  CLP03 − CLP04 = Σ CAS amounts of the claim and its services per claim;
  SVC02 − SVC03 = Σ CAS amounts of the service per service. Check the exact rules against the 5010
  implementation guide's balancing section as reflected in pyx12 or its docs; record sources.
- Reading: a new SNIP 3 diagnostic rule for each failed balance (names the rule, the loop instance,
  the expected and computed amounts, and the segments involved). Golden checkpoint
  `task-1-golden-review.md`: every new SNIP 3 finding on samples/fixtures with a verdict (real
  imbalance in the file / rule wrong → fix the rule / bug). Stop before regenerating.
- Commit: `feat(spec): balancing rules and SNIP 3 diagnostics`.

### Task 2: writability analysis and complete built-in tables
- A write plan compiled from spec + tables: for every occurrence of every loop, which column (or
  automatic qualifier) supplies each element; refuse at preparation with P10 errors: required
  occurrence without a source, required element without a source, a source that cannot be
  inverted (e.g. ambiguous `where`, a `pick` other than a contiguous series, a column reading an
  ancestor loop that another table also writes).
- Complete the built-in tables (5010 and 4010) with the columns the guide requires for a valid file
  and that the tables lack today (e.g. payer technical contact `PER*BL`); report the full list.
  Existing columns and cells unchanged; golden checkpoint `task-2-golden-review.md` listing the new
  columns per table and confirming no existing cell changed. Stop before regenerating.
- Commit: `feat(write): write plan and complete built-in tables`.

## Batch B (Opus → Opus review): emission and Python

### Task 3: the writer
- Rebuild the loop tree from the ordinal columns (T82); emit loops and occurrences in position
  order, repeats in row order; elements from the write plan; written-empty (`""`) vs absent
  (`null`); trim trailing empty elements; composites with the component separator.
- Envelope (T83): parameter type with sender/receiver and qualifiers, date/time, usage indicator,
  first control number, delimiters and line break option; compute counts and control numbers;
  ISA fixed widths; a data value containing a delimiter is a diagnostic.
- Pre-write validation (T85): element type/length/codes, required occurrences/elements, broken or
  out-of-order references, balancing rules; strict error vs `allow_findings`.
- Core round-trip tests on the fixtures (Rust). Commit: `feat(write): emit 835 from tables`.

### Task 4: `oxedi.write` in Python
- `oxedi.write(tables, envelope=..., spec=None, allow_findings=False) -> bytes` (or bytes plus
  diagnostics with `allow_findings`): accepts `Result.tables` or a mapping of Arrow/Polars/pandas
  tables with the spec's schema; envelope as a typed class; the binding converts Arrow to core
  columns and names no segment. Stubs regenerated (`make stubs`), `stubtest` clean, usage file
  extended. Commit: `feat(py): oxedi.write`.

## Batch C (Opus → Opus review): gate and docs

### Task 5: the round-trip and pyx12 gate
- New synthetic valid fixtures (owner decision 2026-10-06; existing fixtures are never edited): at least one 5010 and one 4010 file that balance (several claims, services with CAS, a PLB), meet the guide's required occurrences, and validate clean with pyx12; with a README entry saying what each covers. Unbalanced existing fixtures (multi_claim, trizetto) and the excerpt samples (file, not_available_claim_id) are used for the strict-refusal and `allow_findings` cases.
- Tests (Python, pyx12-dependent ones skipped without it) for every sample and valid fixture,
  5010 and 4010: parse → write → parse equal on written columns, no new diagnostics, pyx12
  validates the output with zero findings. If 4010 turns out disproportionate, stop and report
  (T86 allows falling back to 5010 only with the owner informed).
- Commit: `test: round trip and pyx12 validation of written files`.

### Task 6: docs
- READMEs (writing section with an example), CHANGELOG (`[Unreleased]`: writer, balancing rules,
  SNIP 3 diagnostics, new table columns), `docs/` if relevant, `.doc/state.md`, `.doc/roadmap.md`.
- Commit: `docs: Stage 7 writer`.

## Exit gate (§7 Stage 7)
- Round trip and pyx12 validation green on every sample and valid fixture, both versions.
- Every SNIP 3 diagnostic and every table change reviewed before goldens changed.
- Strict refusals with full P10 errors; `allow_findings` path tested.
- All gates green.
