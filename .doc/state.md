# State — 2026-10-06 (0.3.0 as oxedi; Stage 7a in PR)

Snapshot for picking the project up cold. Update when a stage changes state.

## Pick up here (2026-10-06)

- **Now:** Stage 7a (occurrence model per loop) is complete on branch `stage-7a-occurrences`
  and goes to PR against `master`. Loops declare `occurrences` (object keyed by name: `segment`,
  `pos`, `usage`, `max`, `qualifier`, `codes`) plus loop `usage`/`max`; the 5010 and 4010
  occurrences come from `pyx12` (`spec_vs_pyx12.py --check` covers them); reading emits six
  SNIP 2 occurrence diagnostics and checks codes per occurrence; text cells tell absent
  (`null`) from empty (`""`); a column may name an occurrence, read a loop above its anchor
  and `pick` `first`/`last`/n-th. Table goldens changed only by T80; parity 113/113. After
  merge: deferred minors to Project #8, copy the ledger to `.doc/analysis/stage-7a-ledger.md`,
  then Stage 7 (writer).
- **Now:** release 0.3.0, the first under the name `oxedi` (branch `release-0.3.0`, on top of the
  rename PR #113). The PyPI pending publisher for `oxedi` is registered; TestPyPI's is pending
  (TestPyPI was in maintenance), so 0.3.0 goes without a release candidate. After merge:
  `make release-check TAG=v0.3.0 && make tag`, approve `pypi`; then on PyPI yank `oxedi835`
  0.1.0/0.2.0/0.2.1, set its description to "renamed to oxedi" and archive it (T72); then the
  `duckdb-v0.1.0` tag and the community PR with `repo.ref` = that tag's commit SHA.
- **Released:** 0.2.1 on PyPI 2026-10-05 (#105: type stubs #102, quick-wins sprint #100, PLB
  codes #104); 0.2.0 on 2026-10-04 (#99).
- **Stage 5e merged** (PR #80, 2026-10-04) with its amendment (PR #81: one diagnostic type,
  T58–T61): `scripts/spec_vs_pyx12.py` with `--check` in CI for 5010 and 4010; `codes` and
  `version` in the spec format; 5010 spec completed from pyx12's map (12 `required` flips, TA1,
  42 code lists); `specs/835.4010.json` patch; spec picked by declared version in Python;
  `oxedi.pyx12.validate` returning `oxedi.Diagnostic` (`Rule::External`, `origin`, `code`,
  `Segment.span`). Golden changes: BPR16 on multi_claim and SVC03 on blue_cross, both real data
  issues. Ledgers: `analysis/stage-5e-ledger.md`, `analysis/stage-5e-unify-ledger.md`. Deferred
  findings: issues #82–#97 on Project #8. `.doc/analysis/` notes still at 7f4f8cf (update for 5e).
- **Owner decision pending:** `CLAUDE.md` names only `Spec::builtin_835` as the `expect`
  exception; `Spec::builtin_835_4010` uses the same pattern.
- **Merged today:** Stage 6 (PR #67) and `0.1.0` on PyPI by trusted publishing (rc1 on TestPyPI
  only); README for users (#68); flaky GIL test replaced (#72); Stage 5c module layout (#75);
  Stage 5d performance (#78: index 8 B/segment, `process` 33 → 37 MiB/s, gate 50 not met → #76);
  DuckDB spike and roadmap order (#79).
- **Order after 5e:** 5f DuckDB extension (read; D15 naming before publishing) → 7 D11 + writer in
  core and Python with `pyx12` as gate → 7b DuckDB write through `COPY` → 8 → 9.
- **Key facts for 5e:** `pyx12` 4.0.0 needs Python ≥ 3.11 and bundles `835.4010.X091.A1.xml`,
  `835.5010.X221.A1.xml`, `dataele.xml`, `codes.xml`; validates united clean in ~3.6 s; its 999
  writer crashes (`Cannot create AK2: err_st.vriic was not set`), so use its validation API only;
  `x12valid -J` writes nested JSON (interchange → group → transaction → segments, `cur_line`).
  Five samples are 4010, united is 5010. The spec defines 29 segments (name, type, required,
  min, max), no code lists. No Rust crate validates X12 against HIPAA guides.
- **Research files (git-ignored, local):** `.superpowers/835-parsers-other-languages-survey.md`,
  `.superpowers/duckdb-adoption-survey.md`, `.superpowers/x12-validators-rust-survey.md`;
  spike findings are committed in `.doc/spikes/duckdb-extension.md`; prototype branch
  `spike-duckdb-prototype` (not to merge).
- **Open board (Project #8):** #53 (tables format, with D11), #66 (TestPyPI check coverage), #73
  (type stubs), #74 (Changelog URL), #76 (next performance step, lazy `Segment`), #77 (unread
  property-test coverage).
- **Credentials:** GitHub token secrets deleted; `~/.pypirc` kept by the owner for now (remove
  later); releases go through trusted publishing (`make release-check TAG=… && make tag`, owner
  approves the environment).
- **Analysis notes** (`.doc/analysis/`) are current at 7f4f8cf (after 5d); update after 5e.

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
- The crate (`crates/oxedi_core`) offers: `Delimiters::from_isa`, `frame::next_frame`,
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

## Stage 5d · Pipeline performance — merged (PR #78, 2026-10-04)

Branch `stage-5d-performance`. Gate (`process` >= 50 MiB/s on united and versant) NOT met; the
stage closes with the figure reached and issue #76 (§7 allows it).
- `process`: united ~33 -> 37.1 MiB/s (runs 36.4-38.2), versant ~33 -> 37.8 (38.8-39.3), eyemed
  31.9. Engine 89 / 98 / 102 MiB/s.
- `Document` index: 40 -> 8 bytes per segment (1.93x -> 0.39x the file on united), proven by test.
- Kept: compact spans (T47), segment buffer reuse (T48), validation of unread elements without
  building values (T49-1). Reverted with numbers: direct column writes and single-pass append.
  Upper bound: removing every row append still tops united at ~44.8 MiB/s.
- Next: #76 (lazy `Segment`, cheaper bitmap push, capacity reuse, a real profile). #45 closes
  with the PR; #39 is superseded by #76 and closes with it.

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
the binding (`crates/oxedi_py`, package `oxedi`: `parse`, `parse_file`, `stream`,
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
  `int(N104)`, #46); what `edi-835-parser` drops and we keep; `oxedi.edi_835_parser`
  behind the extra `oxedi[edi-835-parser]` covering its whole surface; native
  counterparts (`count_claims`, `count_patients`, `sum_payments`, `payer`, `payee`,
  `to_polars`, `to_pandas`). Base wheel has no Python dependencies; polars/pandas only as
  extras with lazy imports. DuckDB reads our tables with no dependency (verified); 5b adds a
  test. Survey of Python 835 parsers: only `edi-835-parser` merits a layer.
- D16 `pyx12`: maps as spec oracle/generator inside Stage 9; `validate` and `ContextReader`
  optional (9b) behind `oxedi[pyx12]`.
- D7 writer scheduled after 5b and 6: tables (our schema, Arrow) → `.RMT`, spec inverted,
  derived fields, diagnostics before writing, DuckDB connectors as ingestion adapter outside
  the core, round-trip + `pyx12` gate; D11 is its prerequisite.

Stage 5b is implemented on branch `stage-5b-edi835parser` and in PR (2026-10-04): the
layer `oxedi.edi_835_parser` reproduces `edi-835-parser` 1.8.0's DataFrame cell for cell
(dtypes and per-cell Python types included) on the six samples through path, bytes, memoryview,
file and directory; `parse` is a strict drop-in, memory via `parse_bytes`/`parse_file_obj`/
`parse_many`; `extended=True` recovers claims without services, claim adjustments, PLB and
unmapped REF/AMT; native `count_claims`/`count_patients`/`sum_payments` (Decimal)/`payer`/
`payee`, `to_polars`/`to_pandas` behind extras; CI parity with the real library, DuckDB test,
`scripts/compat_oracle.py` for the originals. 338 cargo + 240 pytest. Known: compat is
0.76–0.93x the library's speed (#55, profile: per-row Python object layer, native parse 19.5 ms
on united); documented divergences in the README. Ledger `analysis/stage-5b-ledger.md`.

Backlog sprint 2 (16 issues; ledger `analysis/backlog-sprint-2-ledger.md`) is in PR: compat frame
built column-wise (united 82 ms vs the library's ~320 ms), sdist without tests, MIT `LICENSE` and
`THIRD_PARTY_NOTICES` shipped in sdist and wheel, key paths for unknown and missing spec keys,
`Rule::kind` and the table renderer in the core, a UTF-8 BOM accepted losslessly
(`Rule::ByteOrderMark`, SNIP 1 by owner ruling), trivia-only input reported as such, patient ids
counted as text. 361 cargo + 268 pytest. Open board after it: #39, #45, #53 (design) and #64.

Stage 6 is merged (PR #67, 2026-10-04): `.github/workflows/release.yml` builds the sdist and six
abi3 wheels (manylinux_2_28 x86_64/aarch64, musllinux_1_2 x86_64, macOS x86_64/arm64, win_amd64),
runs pytest on each platform, publishes by trusted publishing (pre-releases to TestPyPI, finals to
PyPI, environments `testpypi`/`pypi` behind the owner's approval), checks the published version from
TestPyPI and creates the GitHub release from the changelog section. Only a `push` of a `v*` tag on a
commit of `master` publishes; re-runs are idempotent. Deferred: #66. Ledger `analysis/stage-6-ledger.md`.
The README was rewritten for users and contributors (owner's request); the PyPI page carries the
user-facing part and the edi-835-parser migration guide lives in `docs/`.

`0.1.0` is published (2026-10-04): `v0.1.0rc1` went to TestPyPI only and `v0.1.0` to PyPI
(7 files, GitHub release with the changelog notes). The first `v0.1.0` attempt failed a flaky
GIL timing test on musllinux; re-running the failed jobs passed, and the test was replaced by a
thread-progress check (#70). Open after the release: #66, #71 (`__version__`), token clean-up.

Stage 5c is merged (PR #75, 2026-10-04): the core is one folder per module
(`x/mod.rs`, unit tests in `x/tests.rs` or `x/tests/` by topic; clippy `self_named_module_files`
denies the `x.rs` + `x/` form); `spec` and `project` are split by responsibility; a test names
every public item by its module path; benches are one file per layer with unchanged group ids.
No behaviour change: same tests (lib 300, pytest 270), goldens untouched, rustdoc pages
identical, bench within +3.3% of master. Closes #64 and #71 (`oxedi835.__version__`); new board
items #73 (type stubs) and #74 (Changelog link). Stage 5d is merged (PR #78): compact index, 37 MiB/s, #76 holds the next performance step. Order decided 2026-10-04: 5e (`pyx12`: spec cross-check and `validate`) → 5f (DuckDB extension, read) → 7 (D11 and the writer, in the core and Python, `pyx12` as gate) → 7b (DuckDB write via `COPY`) → 8 → 9.

Release path: `make release-check TAG=v0.1.0rc1 && make tag` → approve `testpypi` → bump to `0.1.0`
and date the changelog → tag `v0.1.0` → approve `pypi` → revoke the account-wide tokens, delete the
secrets `PYPI_API_TOKEN` and `pypi_test_api_token`, remove `~/.pypirc`.

Next (owner moved Stage 6 ahead of 5c on 2026-10-04): Stage 6 (wheel matrix, trusted publishing,
release workflow; build the sdist on Linux because the LICENSE symlinks do not survive a Windows
checkout) and `0.1.0` on PyPI, then 5c (D13), Stage 7 (writer), 8 (durable docs), 9 (X12 family
+ `pyx12` maps).

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
  Stage 9; `validate` and `ContextReader` are optional (9b), behind `oxedi[pyx12]`.
- After the roadmap (owner's request 2026-10-03): Stage 8 durable human documentation
  (ideas, patterns, concepts, no code; Python and CLI guides; D14) and Stage 9 X12 family
  toolkit starting with the 837 (D15).

## Private material (never in git)

Real 835 originals, the verified anonymized output and the re-identification key:
`~/Desktop/org/personal/oxedi835-private-samples/` (`originals/`, `anonymized/`,
`mapping.json`). Re-anonymize with `scripts/anonymize_835.py --in-dir <originals>
--out-dir crates/oxedi_core/tests/samples` and `cmp` against `anonymized/` before any commit.
