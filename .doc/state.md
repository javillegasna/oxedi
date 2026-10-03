# State — 2026-10-03

Snapshot for picking the project up cold. Update when a stage changes state.

## Where we are

- Stages 0–3 are merged on `master` (PRs #1 #2 #3 #5 #6). Suite: 156 tests; clippy, fmt,
  rustdoc `-D warnings` clean; CI green.
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

- Project #8 Backlog: issues #7–#19 (deferred review minors). Priority Alta: #18 (array-form
  specs accepted; odd message for `[]`). Media: #7, #12, #15, #17, #19. Baja: the rest.
  Suggested first sprint before Stage 4: test hygiene (#9, #10, #11, #12, #13), small enough
  for a Haiku implementer in one PR.
- `.doc/analysis/` study notes cover Stage 0–1 code only; Stages 2 and 3 (document, spec,
  engine, tree, goldens) still need their sections (structure, flow, tests with reasons,
  concepts, patterns), written from the real code.
- Older plans (`stage-1`, `stage-2`) show pre-P10 error shapes; they are historical records,
  not to be edited.

## Next stage: 4 · Projection + validation

Design questions to settle in chat before writing §7 Stage 4:
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
