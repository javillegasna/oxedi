# Roadmap — oxedi835

Orientation only: where each stage stands, what it delivers, what it unlocks, and which
open decisions it carries. The contract is `architectural-commitment.md` (§7 has the
per-stage commitments); the executable detail is `plans/stage-N-*.md`. Update this file
when a stage changes state.

## Status

| Stage | Delivers | State | Where |
|-------|----------|-------|-------|
| 0 · Scaffolding | Workspace, CI gates, test/property/bench harness | **Done** 2026-10-02 | commits up to `0e1b827` |
| 1 · Framing + Tokenizer | Lossless `Segment` stream, ISA delimiters, symmetric writer | **Done** 2026-10-02 | PR #1, fix PR #2 |
| 2 · Lossless Document | `Document` as `Cow` bytes + spans, borrowed or owned | **Done** 2026-10-02 | PR #3; samples PR #5 |
| 3 · Loop engine | JSON spec, `LoopEngine` events, `LoopTree`, merge-patch extension, P10 errors | **Done** 2026-10-03 | PR #6; deferred findings: issues #7–#19 on Project #8; test hygiene PR #20 closed #9–#13 |
| 4 · Projection + validation | `segments` and `tables` in the spec, Arrow-layout columns, SNIP 1–2 diagnostics with full location (P10), closes #7 #17 #18 #19 | **4a done** 2026-10-03 (PR #30: spec `segments`, validation, `Diagnostic`, `EnvelopeChecker`); 4b in progress | 4a findings: #22–#29; 4b: branch `stage-4b-projection` (columns, `Projector`, `tables`, `Processor`) |
| 5 · Python binding | PyO3/maturin, GIL released, iterator and table APIs | Not started | — |
| 5b · Compatibility oracle | A `tables` spec (plus patch) whose output matches `edi-835-parser` (keiron-stoddart, Python) row for row on the shared files; the diff is the compatibility test | Not started (after 5, D12) | — |
| 5c · Module layout | Split the large source files into navigable submodules with short files and a one-glance discovery path; no behaviour change | Not started (after 5, D13) | — |
| 6 · Distribution | crates.io, PyPI wheels, release CI | Not started | — |
| 7 · Writer | Data → loops → bytes, same spec | Deferred (D7) | — |

## What each stage unlocks

```
0 ──► 1 ──► 2 ──► 3 ──► 4 ──► 5 ──► 6
            │     │     │
            │     │     └─ tables → Polars/Arrow export (D10, decided in 4/5)
            │     └─ real files become a business oracle (claims, services, adjustments)
            └─ random access, owned documents for FFI, Arc measurement (D8)
```

- After **3**: a real 835 is a tree of loops; unknown or proprietary segments are visible
  and extendable with a JSON patch. First stage with business meaning.
- After **4**: rows comparable with the old Python parser; SNIP validation on real files.
- After **5**: the original problem (slow Python ingestion) can be benchmarked end to end.
- After **5b**: a user of `edi-835-parser` can switch with a spec, not a rewrite; the
  row-for-row diff is a public, reproducible compatibility test.
- After **5c**: every module is short enough to read in one sitting and findable from
  `lib.rs` without grepping; a prerequisite for crates.io docs and outside contributors.

## Open decisions by stage

| Decision | Stage that closes it | One line |
|----------|----------------------|----------|
| D2 · Spec format and merge | 3 (closed as T6–T8) | JSON + serde, flat loops with `parent`, RFC 7386 merge patch |
| D8 · `Cow`+spans vs `Arc`+spans | 5 | Measure time and memory with Python holding documents |
| D9 · YAML specs | after 3, when someone writes specs by hand | Second deserializer over the same `Spec` |
| D10 · Columnar projection / Arrow | 4 (closed as T14–T15) and 5 | Stage 4 emits Arrow-layout columns without the crate; Python exports zero-copy |
| D11 · Segment cardinality per loop, declarative balancing rules | after 4 | SNIP 2 completion and SNIP 3 need a spec extension; decide once tables are in use |
| D6 · Chunked tokenizer (S3 streaming) | when a file does not fit in memory | Framing is already pure; segments would own or lend |
| D7 · Writer | when a generation case exists | `write_to` is half of it; spec is already structural |
| D12 · Compatibility with `edi-835-parser` | 5b | Can a data-only `tables` spec reproduce its DataFrame exactly? Measures N3 and gives a migration path for its users |
| D13 · Module layout | 5c | `spec.rs` is 2.5k lines after Stage 4a; decide the submodule split and the discovery rules (file length, one noun per file, index in `lib.rs`) |
| D3 · WASM/Extism extensions | never, unless data patches prove insufficient | — |

## Conventions that hold across stages

- Every stage: spec section approved in `architectural-commitment.md` → plan in `plans/` →
  feature branch → TDD per task → fresh-context review of the whole branch → PR with
  intent and verification → the owner merges.
- Gates that never relax: lossless on every file, no panic on input, no runtime or I/O
  dependency in the core, clippy `-D warnings`, fixtures and samples untouched.
- Real-shaped oracle: five synthetic fixtures (`tests/fixtures/`) and six anonymized
  payer files (`tests/samples/`). New invariants are expected to hold on all eleven.
