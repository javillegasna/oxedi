# pyx12 adapter: full error tree, positions, loop paths — Implementation Plan

> Backlog work (issues #133, #96, #91, #90, #89), decisions approved by the owner 2026-10-06.
> One Opus implementer, Opus review, one fix wave, one PR.

**Goal:** `oxedi.pyx12.validate` reports every finding pyx12's engine records (envelope, segment
and element, including ST/SE), without touching global logging state, each at the right segment
with its datum and loop path; the writer stops inventing ST03.

## Decisions (owner, binding)
1. **Error tree capture (#133, #96, #91).** Install once, when `oxedi.pyx12` is first used, a
   subclass of `pyx12.error_handler.err_handler` as `pyx12.error_handler.err_handler`. It behaves
   exactly as the original; on construction it stores itself in a thread-local slot only when the
   current thread is inside `validate`. After `x12n_document` returns, `validate` walks that error
   tree (ISA, GS, ST, segment and element nodes with their lines and codes) with its own visitor.
   The JSON output and the logging capture (`_LogCapture`, `_isolated_logging`, the lock's logging
   override) are removed: no change to `logging.disable`, levels or propagation. Other threads and
   other callers of pyx12 see the original behaviour.
2. **Loop path (#89).** A private `_core` helper returns, for a parse result and a segment index,
   the loop path in the same format as `Diagnostic.path` from `parse`. No public API change.
3. **Envelope positions (#90).** Findings that concern a trailer (segment count, control number
   mismatch of SE/GE/IEA, group/transaction counts) point at the trailer segment, with the
   offending value as datum; other envelope findings stay at the header. The mapping is by pyx12's
   error codes, listed in one place and pinned by tests.
4. **ST03 (#133).** The writer no longer writes ST03 (`implementation_convention_reference`): no
   real 5010 sample or fixture carries it, pyx12's 835 5010 map marks it Not Used, the spec has
   it optional, and the writer does not invent envelope data. `validate` reports what pyx12 says,
   with no exclusion list. A future optional `Envelope` field for it is an issue, not this work.
5. **Failures (#91).** A file pyx12 rejects at the start points at segment 0 with the ISA bytes
   as datum; a failure after reading names the last segment pyx12 completed; with the tree there
   is no JSON step to fail.

## Global constraints
- `CLAUDE.md` non-negotiables (P10 diagnostics with rule, place, datum; comments
  implementation-only; module layout; binding names no 835 segment — the trailer mapping is by
  pyx12 error codes and envelope roles, not hard-coded 835 segments beyond the X12 envelope).
- The core stays sans-IO with no new dependency; a core API for the loop path is allowed if the
  binding cannot compute it from what the core already exposes.
- `pyx12` stays an optional extra (`oxedi[pyx12]`), pinned `>=4.0,<5`.
- Fixtures and samples are never edited. Goldens that change (writer output without ST03) are
  reviewed before regeneration.
- Gates: `make gates`, `make py-test` with and without pyx12, `make stubtest`,
  `make duckdb-oracle` (the writer change reaches the DuckDB byte oracle).

## Review focus
1. Thread safety and isolation of the error-handler patch: concurrent `validate` calls, a plain
   `x12n_document` call by user code, re-import, pyx12 not installed.
2. Findings parity: everything the old JSON path reported is still reported (same levels, codes,
   positions or better), plus the ST/SE element errors; compare old vs new on every sample and
   fixture and explain each difference.
3. Positions: trailer findings on the trailer, datum filled; rejection at segment 0; loop paths
   equal to those of `parse` diagnostics at the same segment.
4. The ST03 change: writer output, round trips, the DuckDB byte oracle (both sides change
   together), CHANGELOG entries (Python and extension), README.

## Tasks (one implementer, commits per issue)
- T1 (#96, #133, #91): error-tree capture replacing JSON and logging; tests: no global logging
  change during a call (disabled logging stays disabled, user handlers on pyx12 loggers receive
  their records), concurrency, ST03/SE element findings captured, failure cases.
- T2 (#90): trailer positions and datum; tests per code.
- T3 (#89): loop path helper and its use; test equality with `parse` paths.
- T4 (ST03): writer stops writing ST03; goldens reviewed then regenerated; byte oracle green;
  CHANGELOG (Python `[Unreleased]` Changed; extension `[0.2.0]`), README if it shows ST03.
- Docs: `oxedi.pyx12` README section states what `validate` reports and that logging is not
  touched.

## Exit gate
- Old-vs-new findings comparison over all samples and fixtures written to the PR's Verification.
- All gates green; issues #133 #96 #91 #90 #89 resolved; an issue for an optional ST03 envelope
  field filed after merge.
