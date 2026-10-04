# State — 2026-10-03 (Stage 5 merged, PyPI reserved)

Snapshot for picking the project up cold. Update when a stage changes state.

## Where we are

- Stages 0–3 are merged on `master` (PRs #1 #2 #3 #5 #6), plus the test-hygiene sprint
  (PR #20, closed #9–#13) and the merge-patch docs (PR #21, closed #14).
- Stage 4a is merged (PR #30, 2026-10-03; closed #7 #17 #18 #19; plan
  `plans/stage-4a-spec-diagnostics.md`, ledger `analysis/stage-4a-ledger.md`). Suite: 229
  tests; clippy, fmt, rustdoc `-D warnings` clean. Baseline: envelope checker 84–96 MiB/s on
  the three largest samples (commit 616599d message).
- The crate after 4a adds: `Spec` `segments` section (`ElementType`, `ElementDef`,
  `SegmentDef`), shape pre-check (`NotAnObject`), `EmptySegmentId`, `OverlappingTriggers`
  (narrow rule), `control` per envelope loop, `Event::LoopOpened.segment`,
  `Node::opened_by`, `Diagnostic`/`Rule`/`SnipLevel`/`LoopRef`, `EnvelopeChecker` (SNIP 1).
- The crate (`crates/edi835_core`) offers: `Delimiters::from_isa`, `frame::next_frame`,
  `Tokenizer<'a>` (lazy, lossless `Segment` stream), `Segment::write_to` (symmetric writer),
  `Document<'a>` (`Cow` bytes + spans, borrowed or owned), `Spec` (JSON loop spec with
  validation and RFC 7386 `merge_patch`, built-in `specs/835.json`), `LoopEngine` (events by
  index: `LoopOpened{implicit}`, `LoopClosed`, `Captured`, `Unmatched`, `Empty`), `LoopTree`.
- Oracle: 5 synthetic fixtures + 6 anonymized real payer files (1 to 1,332 claims); golden
  event streams in `tests/golden/`. Known anomalies, pinned exactly: trizetto's bogus `XX`
  (1 unmatched), multi_claim's patient `N3`/`N4` inside 2100 (4 unmatched), blue_cross
  fragment (2 implicit envelope opens).
- Baselines (local, release): tokenize ~180 MiB/s, index ~1.1 GiB/s, engine ~110 MiB/s on
  fixtures; engine on the three largest samples measured in bytes/s and events/s
  (commit acc465e message).

## Open items

- Project #8: open are #28 (rename `ISA11` → `isa11` in `specs/835.json`, one line, owner
  ruled "no semantics"; the permission classifier blocked the implementer's edit, so it waits
  for the owner's explicit go), #39 (projector throughput needs design changes to reach
  50 MiB/s; profile in the issue; decide with Stage 5's zero-copy export), #40–#42 (Baja).
- `.doc/analysis/` study notes describe the crate before Stage 4 (rewritten 2026-10-03 at
  commit 8f86e42); one update from the real code is due after the 4b PR merges.
- Older plans (`stage-1`, `stage-2`) show pre-P10 error shapes; they are historical records,
  not to be edited.

## Stage 5 · Python binding — merged (PR #50)

Stage 4 is done (4a PR #30, 4b PR #38, backlog sprint PR #43). §7 Stage 5 (T18–T24) and
plan `plans/stage-5-python.md` were approved 2026-10-03; the branch `stage-5-python` holds
the binding (`crates/oxedi835_py`, package `oxedi835`: `parse`, `parse_file`, `stream`,
`Spec`, `Document`/`Segment`/`Delimiters`, `Result`/`Tables`/`Table`/`Diagnostic`, Arrow
export by PyCapsule, CI job on 3.11 and 3.13, clean-venv wheel smoke script, comparison
script against `edi-835-parser`), the D8 example and docs. Core `src/` unchanged. Suite:
337 cargo tests, 112 pytest; the 22 goldens are reproduced from Python; `stream` holds ~14×
less memory than `parse`; two threads run in ~0.5× the sequential time; 16–24× faster than
`edi-835-parser`. D8 closed as T24 (keep `Cow`). Deferred findings: #47–#49. PR #44 (#28,
`isa11`) merged too.

PyPI: `oxedi835 0.0.1a1` is published on TestPyPI and PyPI (2026-10-03; wheel
`cp311-abi3-manylinux_2_34_x86_64` + sdist; tag `v0.0.1a1` on 5c03a76). PR #51 (pre-release
metadata, root `Makefile` with gates/py-dev/py-test/dist/smoke/publish via twine/tag) awaits
merge. `0.1.0` comes after 5b and the Stage 6 wheel matrix; `1.0` once the API holds for two
or three releases and the Stage 8 documentation exists. Tokens live in `~/.pypirc` (local)
and as GitHub secrets, never in the repo; the account-wide token should be replaced by a
project-scoped one.

Decisions taken 2026-10-03 after the merge (all in §6.2 and the roadmap):
- 5b in four parts (D12): DataFrame parity by spec on the originals (shim for the library's
  `int(N104)`, #46); what `edi-835-parser` drops and we keep; `oxedi835.edi_835_parser`
  behind the extra `oxedi835[edi-835-parser]` covering its whole surface; native
  counterparts (`count_claims`, `count_patients`, `sum_payments`, `payer`, `payee`,
  `to_polars`, `to_pandas`). Base wheel has no Python dependencies; polars/pandas only as
  extras with lazy imports. DuckDB reads our tables with no dependency (verified); 5b adds a
  test. Survey of Python 835 parsers: only `edi-835-parser` merits a layer.
- D16 `pyx12`: maps as spec oracle/generator inside Stage 9; `validate` and `ContextReader`
  optional (9b) behind `oxedi835[pyx12]`.
- D7 writer scheduled after 5b and 6: tables (our schema, Arrow) → `.RMT`, spec inverted,
  derived fields, diagnostics before writing, DuckDB connectors as ingestion adapter outside
  the core, round-trip + `pyx12` gate; D11 is its prerequisite.

Stage 5b is implemented on branch `stage-5b-edi835parser` and in PR (2026-10-04): the
layer `oxedi835.edi_835_parser` reproduces `edi-835-parser` 1.8.0's DataFrame cell for cell
(dtypes and per-cell Python types included) on the six samples through path, bytes, memoryview,
file and directory; `parse` is a strict drop-in, memory via `parse_bytes`/`parse_file_obj`/
`parse_many`; `extended=True` recovers claims without services, claim adjustments, PLB and
unmapped REF/AMT; native `count_claims`/`count_patients`/`sum_payments` (Decimal)/`payer`/
`payee`, `to_polars`/`to_pandas` behind extras; CI parity with the real library, DuckDB test,
`scripts/compat_oracle.py` for the originals. 338 cargo + 240 pytest. Known: compat is
0.76–0.93x the library's speed (#55, profile: per-row Python object layer, native parse 19.5 ms
on united); documented divergences in the README. Ledger `analysis/stage-5b-ledger.md`. Next:
backlog sprint 2 (16 issues, briefs in `.superpowers/sdd/backlog-sprint-2/`), then 5c, 6, 7…; then 5c (D13), Stage 6
(wheel matrix, trusted publishing, `0.1.0`), Stage 7 (writer), 8 (durable docs), 9 (X12
family + `pyx12` maps).

Stage 4 decisions, settled in §7 (kept for reference):
The decisions that were open, now settled in §7:
- `Diagnostic` as a first-class deliverable (P10): segment index, byte range, element and
  component position, loop path, rule code, message; rendering of engine anomalies
  (unmatched, implicit opens; see #19).
- Spec grows a `segments` section: element names, types, required, composites; validation
  pass also closes #7 and #18 (and decides #17).
- Columnar projection (D10): typed columns per level (payments / claims / services /
  adjustments) with parent indices, so Arrow/Polars export in Stage 5 is zero-copy.
- SNIP validation levels to cover now vs later; how diagnostics and projection share the
  same walk over the tree.
- D8 (Cow+spans vs Arc+spans) is measured in Stage 5, not 4; D9 YAML stays deferred.
- After Stage 5 (owner's request 2026-10-03): 5b compatibility oracle against
  `edi-835-parser` (D12, three parts: same DataFrame via a spec; prove what that library
  drops and we keep; a compatible `TransactionSets`/`to_dataframe()` API) and 5c module
  layout plan (D13); see roadmap. `pyx12` interop (D16): map cross-check/generator is part of
  Stage 9; `validate` and `ContextReader` are optional (9b), behind `oxedi835[pyx12]`.
- After the roadmap (owner's request 2026-10-03): Stage 8 durable human documentation
  (ideas, patterns, concepts, no code; Python and CLI guides; D14) and Stage 9 X12 family
  toolkit starting with the 837 (D15).

## Private material (never in git)

Real 835 originals, the verified anonymized output and the re-identification key:
`~/Desktop/org/personal/oxedi835-private-samples/` (`originals/`, `anonymized/`,
`mapping.json`). Re-anonymize with `scripts/anonymize_835.py --in-dir <originals>
--out-dir crates/edi835_core/tests/samples` and `cmp` against `anonymized/` before any commit.
