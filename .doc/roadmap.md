# Roadmap — oxedi835

Orientation only: where each stage stands, what it delivers, what it unlocks, and which
open decisions it carries. The contract is `architectural-commitment.md` (§7 has the
per-stage commitments); the executable detail is `plans/stage-N-*.md`. Update this file
when a stage changes state.

## Status

Order after Stage 5b (owner, 2026-10-04): backlog sprint 2 → **Stage 6 → `0.1.0` on PyPI** → 5c → 7 → 8 → 9. Stage 6 moved ahead of 5c because 5c changes no public API and the first usable release needs the multi-platform wheels; #55 (compat speed) lands in the sprint so the first release is not slower than the library it replaces.

| Stage | Delivers | State | Where |
|-------|----------|-------|-------|
| 0 · Scaffolding | Workspace, CI gates, test/property/bench harness | **Done** 2026-10-02 | commits up to `0e1b827` |
| 1 · Framing + Tokenizer | Lossless `Segment` stream, ISA delimiters, symmetric writer | **Done** 2026-10-02 | PR #1, fix PR #2 |
| 2 · Lossless Document | `Document` as `Cow` bytes + spans, borrowed or owned | **Done** 2026-10-02 | PR #3; samples PR #5 |
| 3 · Loop engine | JSON spec, `LoopEngine` events, `LoopTree`, merge-patch extension, P10 errors | **Done** 2026-10-03 | PR #6; deferred findings: issues #7–#19 on Project #8; test hygiene PR #20 closed #9–#13 |
| 4 · Projection + validation | `segments` and `tables` in the spec, Arrow-layout columns, SNIP 1–2 diagnostics with full location (P10), closes #7 #17 #18 #19 | **Done** 2026-10-03 (4a PR #30, 4b PR #38, backlog sprint PR #43) | findings closed by #43; open: #28, #39–#42; SNIP 3 → D11 |
| 5 · Python binding | PyO3/maturin `abi3` (Python ≥ 3.11), GIL released, `parse`/`stream`/`Spec`, Arrow by PyCapsule, D8 measured | **Done** 2026-10-03 (PR #50; 112 pytest; D8 → T24) | findings #47–#49 |
| 5b · Compatibility oracle | (1) A `tables` spec whose DataFrame equals `edi-835-parser`'s `to_dataframe()` row for row on the originals (shim for its `int(N104)` limitation, #46); (2) find what that library drops (claim-level adjustments, `PLB`, unmapped `REF`/`AMT`) and prove `oxedi835` keeps it on the same files; (3) a compatible Python API as the subpackage `oxedi835.edi_835_parser` behind the extra `oxedi835[edi-835-parser]` (one subpackage per imitated library, named after it; survey other Python 835 parsers first), covering that library's whole public surface (`parse(path\|dir)`, `TransactionSets` with iteration, `len`, `count_claims`, `count_patients`, `sum_payments`, `sort_columns`, `to_dataframe`; `TransactionSet` with `payer`, `payee`, `to_dataframe`, `serialize_service` and its loop objects) so its users migrate without code changes; (4) native counterparts on the Arrow/Polars API (`Result.count_claims/count_patients/sum_payments/payer/payee`, `to_polars()`, optional `to_pandas()`) plus an old→new migration table, so the quick path (extra) and the final path (native) both exist | **In PR** 2026-10-04 (parity cell for cell; 240 pytest) | branch `stage-5b-edi835parser`; follow-ups #55–#62 |
| 5c · Module layout | Split the large source files into navigable submodules with short files and a one-glance discovery path; no behaviour change | Not started; **after Stage 6** (owner, 2026-10-04: internal refactor, does not block `0.1.0`) | — |
| 6 · Distribution | crates.io, PyPI wheels (manylinux/macOS/Windows), release CI with trusted publishing; `0.1.0` after 5b | Name reserved on PyPI with `0.0.1a1` (2026-10-03); **next after backlog sprint 2, before 5c** (owner, 2026-10-04); publishes `0.1.0` | — |
| 7 · Writer | Tables (our schema, `Tables` or Arrow) → loops → segments → bytes with the same spec inverted; derived fields computed; diagnostics before writing; DuckDB connectors feed it from relational databases outside the core; round-trip and `pyx12` validation as gate (also through a SQLite export with an optional lossless `segments` layer, an internal dev tool first); D11 (order, cardinality, balancing) resolved inside | Scheduled after 5b and 6 (D7, 2026-10-03) | — |
| 8 · Durable documentation | Human-readable book of the ideas, concepts and patterns that govern the project (no code snippets, no line references, nothing that rots); user guides for the Python library and the CLI | Not started (after the roadmap closes, D14) | — |
| 9 · X12 family toolkit | The engine, spec format, projection and CLI serve other transaction sets of the same family (837 first: same loop logic, different segment definitions); 835 becomes one spec among several. Includes the `pyx12` map cross-check and spec generator (D16, part 1) | Not started (D15, D16) | — |
| 9b · `pyx12` interop (optional) | `oxedi835.pyx12.validate` merging `pyx12`'s SNIP 3–7 findings into our `Diagnostic` list; `oxedi835.pyx12.ContextReader` as a read-only `X12ContextReader`-shaped view, only on demand; behind the extra `oxedi835[pyx12]` | Optional (D16, parts 2–3) | — |

## What each stage unlocks

```
0 ──► 1 ──► 2 ──► 3 ──► 4 ──► 5 ──► 5b ──► 6 (0.1.0) ──► 5c ──► 7 ──► 8 ──► 9 (──► 9b optional)
            │     │     │           │                   │
            │     │     │           │                   └─ writer: tables → .RMT (D7, D11)
            │     │     │           └─ edi-835-parser parity and compatible API (D12)
            │     │     └─ tables → Arrow by PyCapsule (D10 → T14/T15; Polars, pyarrow, DuckDB)
            │     └─ real files become a business oracle (claims, services, adjustments)
            └─ random access, owned documents for FFI, Cow kept (D8 → T24)
```

- After **3**: a real 835 is a tree of loops; unknown or proprietary segments are visible
  and extendable with a JSON patch. First stage with business meaning.
- After **4**: rows comparable with the old Python parser; SNIP validation on real files.
- After **5**: the original problem (slow Python ingestion) can be benchmarked end to end.
- After **5b**: a user of `edi-835-parser` can switch with a spec and a compatible API, not a
  rewrite; the row-for-row diff is a public, reproducible compatibility test, and the data
  that library drops is shown recovered on the same files.
- After **8**: a reader learns why the project is shaped this way without opening the code,
  and a user of the Python library or the CLI has a guide that does not go stale.
- After **9**: adding a transaction set is writing a spec, the toolkit's promise made good
  beyond the 835; `pyx12`'s maps cross-check our specs and seed the new ones.
- After **7**: a payer or a clearinghouse writes an 835 from its database with one SQL per
  table and the same spec that reads it; the round trip is the proof.
- After **9b** (optional): a single diagnostics list covers SNIP 1–7 by delegating 3–7 to
  `pyx12`; code written against its context reader runs on our document.
- After **5c**: every module is short enough to read in one sitting and findable from
  `lib.rs` without grepping; a prerequisite for crates.io docs and outside contributors.

## Open decisions by stage

| Decision | Stage that closes it | One line |
|----------|----------------------|----------|
| D2 · Spec format and merge | 3 (closed as T6–T8) | JSON + serde, flat loops with `parent`, RFC 7386 merge patch |
| D8 · `Cow`+spans vs `Arc`+spans | 5 (closed as T24: keep `Cow`) | Measured on the largest sample: `Arc` builds slower, same retention memory, wins only on clones nobody makes; spans weigh 2× the bytes → #45 |
| D9 · YAML specs | after 3, when someone writes specs by hand | Second deserializer over the same `Spec` |
| D10 · Columnar projection / Arrow | 4 (closed as T14–T15) and 5 | Stage 4 emits Arrow-layout columns without the crate; Python exports zero-copy |
| D11 · Segment cardinality per loop, declarative balancing rules | 7 (prerequisite of the writer) | Order and repetition per loop for emission; balancing rules to compute or check totals; SNIP 2 completion and SNIP 3 |
| D6 · Chunked tokenizer (S3 streaming) | when a file does not fit in memory | Framing is already pure; segments would own or lend |
| D7 · Writer | 7 (scheduled 2026-10-03) | Case: generate `.RMT` from relational data; input is our table schema via Arrow; DuckDB as the ingestion adapter outside the core |
| D12 · Compatibility with `edi-835-parser` | 5b | Can a data-only `tables` spec reproduce its DataFrame exactly? Measures N3 and gives a migration path for its users |
| D14 · Durable documentation | 8 | What belongs in the book (ideas, patterns, concepts, guides) vs. what stays in rustdoc and the plans (code, signatures); format and where it lives |
| D15 · X12 family toolkit (837…) | 9 | Naming (crate and binary no longer 835-specific), one spec per transaction set, what the CLI exposes, which 835 assumptions leaked into code |
| D16 · `pyx12` interop | 9 (maps as oracle/generator) and 9b (validate, ContextReader, optional) | Complement, not imitate: their maps and validation, our speed and tables |
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
