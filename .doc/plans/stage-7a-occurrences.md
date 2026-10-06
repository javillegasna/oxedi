# Stage 7a · Occurrence model per loop — Implementation Plan

> Lean plan: design-heavy tasks to Opus with contracts, not transcribed code. Batches of two
> tasks, one Opus review per batch, Opus triage at the end. Golden checkpoints: the implementer
> stops before any `UPDATE_GOLDEN` and the controller reviews every changed diagnostic and cell.

**Goal:** each loop of the spec declares its segment occurrences (name, segment, qualifier, usage,
maximum repeat, position, own codes), generated from `pyx12` and reviewed; reading validates them
(SNIP 2) and validates codes per occurrence; table columns can name occurrences, ancestor loops
and a selector; text cells tell absent (`null`) from empty (`""`).

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 7a" (T75–T80), approved 2026-10-06.
Branch `stage-7a-occurrences`. Ledger `.superpowers/sdd/stage-7a/progress.md`.

## Global Constraints
- `CLAUDE.md` non-negotiables: lossless (no segment is ever dropped: every new rule is a
  diagnostic); no `unwrap`/`expect`/`panic!` or fallible indexing on input in `src/`; no code
  names an 835 segment (occurrences, qualifiers and names are spec data); P10 errors and
  diagnostics with one full-text `Display` test per variant; comments implementation-only;
  ~400 code lines per file; core dependencies unchanged.
- **Format shape (binding, controller ruling):** a loop's occurrences are a JSON **object keyed
  by occurrence name**, ordered by `pos` (ties keep any order among themselves, as the seven NM1
  of 2100), so RFC 7386 patches can change one occurrence by name without restating the list.
  The loop gains a maximum repeat. The loop's `trigger` stays as today and must correspond to one
  occurrence. The old `segments` list is removed (0.x; CHANGELOG notes it).
- Goldens change only after a per-item review written by the implementer and approved by the
  controller (stop and report NEEDS_CONTEXT at each golden checkpoint).
- The `oxedi.edi_835_parser` parity test against `edi-835-parser` 1.8.0 stays green with no
  differing cell, from the first batch to the last.
- Gates on every commit: `make gates`; batches touching Python or the extension also `make
  py-test` (with and without pyx12), `make test_release`, `make duckdb-oracle`.

## Review Focus
1. Occurrence matching on read: a segment matches by segment id plus qualifier; same-position
   occurrences in any order; an unknown qualifier is a diagnostic, never a drop; the trigger
   occurrence and loop opening behave exactly as today (event goldens unchanged).
2. False SNIP 2 diagnostics on real files: 4010 samples under the 4010 occurrences, optional
   loops absent, repeats at their maximum.
3. Codes per occurrence vs. the global union: a segment that matches no occurrence still gets
   the global check; read and unread paths agree.
4. T80 only on text columns; numbers and dates keep `null`; the compat layer's output identical.
5. Patches: a merge patch that changes or removes one occurrence by name works, and the 4010
   patch expresses the version differences without restating whole loops.

---

## Batch A (Opus → Opus review): format and generation

### Task 1: occurrence format, load-time validation, engine membership
- Raw/compiled spec types for occurrences and the loop maximum repeat; load-time validation with
  new P10 `SpecError` variants (key path as written): duplicate position semantics are allowed,
  but reject a qualifier pointing at an element the segment definition lacks, an empty qualifier
  code set, a usage value other than required/situational, a maximum below 1, a trigger with no
  matching occurrence, codes for an element the segment lacks (reuse the 5e code checks).
- Loop membership (which segments a loop holds) derives from the occurrences, so the engine's
  behaviour is unchanged: event goldens must not change.
- Built-in specs: convert the existing `segments` lists into minimal occurrences (one per segment
  id, situational, unbounded, positions in today's list order) so everything keeps passing; the
  real occurrences arrive in Task 2.
- Commit: `feat(spec): segment occurrences per loop`.

### Task 2: occurrences generated from pyx12 and applied
- `scripts/spec_vs_pyx12.py` generates occurrences per loop for 5010 and 4010 from the maps
  (segment, qualifier element and code set from the first element's valid codes or the map's
  identifying element, usage R/S, `max_use`, `pos`, loop `repeat`, per-occurrence codes),
  proposes names from the map's segment names (snake_case, unique within the loop), and
  `--check` compares occurrences; exclusions in the ignore file with reasons.
- Apply to `specs/835.json` (5010) and express 4010 differences in `specs/835.4010.json` by
  occurrence name. Report the generated names for the controller to review.
- No validation uses the occurrences yet, so goldens must not change; if any does, stop.
- Commit: `feat(spec): occurrences from pyx12 for 5010 and 4010`.

## Batch B (Opus → Opus review): validation and absent/empty

### Task 3: SNIP 2 occurrence validation and codes per occurrence
- New level-2 rules (T78): required occurrence missing; occurrence over its maximum; loop over
  its maximum; segment out of position order; segment matching no occurrence of its loop. Each
  names rule, loop path, occurrence, segment index and id, and datum; full-text `Display` tests.
- Codes per occurrence (T77) in the element checks, read and unread paths alike (extend
  `project/unread_tests.rs`).
- Golden checkpoint: write `task-3-golden-review.md` (every new diagnostic per sample/fixture:
  real data issue, spec too strict → fix the spec, or bug) and stop before regenerating.
- Commit: `feat(check): occurrence validation and codes per occurrence`.

### Task 4: absent vs. empty in text columns
- T80 in the projection: text column cells are `null` when the element is absent and `""` when
  present but empty; numeric and date columns unchanged. Python and the DuckDB extension follow
  (VARCHAR `''`, BLOB empty with `binary := true`); the oracle test still compares equal.
- Adapt `oxedi.edi_835_parser` where it relied on empty being `null`; the parity test stays
  green.
- Golden checkpoint: `task-4-golden-review.md` (counts per table/column of cells that become `""`,
  a sample of each, and confirmation that only text columns changed); stop before regenerating.
- Commit: `feat(project): text cells tell absent from empty`.

## Batch C (Opus → Opus review): column sources by occurrence

### Task 5: columns that name occurrences, ancestors and a selector
- T79: a column source may name an occurrence (by loop and name) instead of `segment` + `where`,
  an ancestor loop of the table's anchor (L0), and a selector `first` / `last` / nth for repeated
  occurrences (L2). Load-time validation with P10 errors. Existing `segment` + `where` sources
  keep working.
- Built-in tables name occurrences where it clarifies the source; no cell may change (goldens
  identical).
- CHANGELOGs (Python, extension), READMEs where the spec format is described, `docs/` spec
  format notes, `.doc/state.md`, `.doc/roadmap.md`.
- Commit: `feat(spec): column sources by occurrence, ancestor and selector`.

## Exit gate (§7 Stage 7a)
- `spec_vs_pyx12.py --check` passes for 5010 and 4010 with exclusions listed.
- Every new diagnostic and every `null` → `""` cell reviewed before goldens changed.
- Tables unchanged except T80; event goldens unchanged.
- edi-835-parser parity green; `make gates`, `make py-test` (with and without pyx12), extension
  tests and oracle green.
