# State — 2026-10-03 (night)

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

- Project #8 Backlog: #8, #15, #16 (Stage 3 leftovers) and #22–#29 (Stage 4a final-review
  findings: schema key paths, composite control values, lossy UTF-8 in Display, ImplicitLoop
  naming the missing trigger, `end == trigger`, R scale cap, ISA11 name, separator-ambiguous
  paths). #7, #17, #18, #19 closed with PR #30.
- `.doc/analysis/` study notes describe the whole crate from the real code (rewritten
  2026-10-03 at commit 8f86e42); update them again when Stage 4 lands.
- Older plans (`stage-1`, `stage-2`) show pre-P10 error shapes; they are historical records,
  not to be edited.

## Current stage: 4 · Projection + validation

§7 Stage 4 approved 2026-10-03 (T11–T17, resolves D10, opens D11). The stage runs as two
plans and two PRs. 4a (spec `segments` + validation closing #7 #17 #18, `LoopOpened.segment`
for #19, `Diagnostic`, `EnvelopeChecker` SNIP 1) is merged. Current: plan 4b on branch
`stage-4b-projection`
(`plans/stage-4b-projection.md`: Arrow-layout columns, `Projector` SNIP 2, `tables`,
`Processor`, table and diagnostic goldens, bench), written from §7 T14–T16 and the Rule
level-2 variants already declared in `diagnostic.rs`; it must also settle #27 (R scale cap)
and decide on #25 (Display contract of ImplicitLoop) before freezing diagnostics. Execution
with subagents: main session orchestrates only; implementer by complexity, reviewer one tier up.
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

## Private material (never in git)

Real 835 originals, the verified anonymized output and the re-identification key:
`~/Desktop/org/personal/oxedi835-private-samples/` (`originals/`, `anonymized/`,
`mapping.json`). Re-anonymize with `scripts/anonymize_835.py --in-dir <originals>
--out-dir crates/edi835_core/tests/samples` and `cmp` against `anonymized/` before any commit.
