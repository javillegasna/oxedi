# Stage 5e · Spec completeness and validation with `pyx12` — Implementation Plan

> Lean plan: design-heavy tasks (spec format, cross-check) go to Opus with contracts, not
> transcribed code; the Python wrapper goes to Sonnet. Batches of two tasks, one review each,
> an Opus triage at the end.

**Goal:** our 835 spec is checked against `pyx12`'s maps and completed from them, with one spec
per version (5010 default, 4010 as a patch) and code lists validated natively; Python users can
run `pyx12` validation and get our `Diagnostic`s back.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 5e" (T52–T57); D16.

## Global Constraints
- Core rules from `CLAUDE.md`: sans-IO, only `serde`/`serde_json`; no `unwrap`/`expect`/`panic!`
  or fallible indexing on input in `src/`; no code names an 835 segment (choices such as "read
  GS08" are data in the spec); P10 errors (rule, place, datum; full-text `Display` test per
  variant; `source()` chains); one folder per module, ~400 code lines per file.
- `pyx12` is never a dependency of the core or the base wheel: it lives behind the extra
  `oxedi835[pyx12]`, pinned to the tested series (`pyx12>=4.0,<5`), and `scripts/`.
- Data derived from `pyx12` maps carries the BSD notice in `THIRD_PARTY_NOTICES` (shipped in sdist
  and wheels, checked by `scripts/check_wheel.py`).
- Goldens change only with a per-diagnostic review written in the batch report: every new
  diagnostic on a sample or fixture is explained (real data issue, spec gap, or bug).
- Public API: additions only, except where T53 needs a choice (see Task 2); list every public
  addition in the report and in `tests/public_paths.rs`.
- Gates on every commit: `make gates`; batches touching Python also `make py-test`, with and
  without `pyx12` installed.

## Review Focus
1. **4010 files under the 5010 spec:** without version selection, 4010 samples would show false
   code and usage diagnostics. Selection must pick the 4010 spec for the five 4010 samples and the
   5010 spec for united, proven by test.
2. **Code lists on composites and conditional elements:** a code list on a component, an empty
   optional element, and a value that is valid only in another loop's context (pyx12 codes are
   per element *in a loop*) must not produce false diagnostics.
3. **Mapping `pyx12` positions to ours:** `pyx12` counts segments (its `cur_line`); our
   `Diagnostic` uses segment index and byte range. Off-by-one and BOM/leading-trivia files are the
   traps.
4. **`pyx12` crashes:** an exception inside `pyx12` (like its 999 bug) must become a diagnostic,
   not a traceback.
5. **Drift test stability:** the cross-check test must fail only for real disagreement in what the
   spec covers, not for things the spec leaves out on purpose (those are listed with reasons).

---

## Batch A (Opus → Opus review): cross-check tool and spec format

### Task 1: `scripts/spec_vs_pyx12.py` (T52)
- Reads `pyx12`'s installed maps (`835.5010.X221.A1.xml`, `835.4010.X091.A1.xml`,
  `dataele.xml`, `codes.xml`; locate them through the installed package, never vendor them) and
  a spec (`specs/835.json`, optionally with `specs/835.4010.json` applied as merge patch).
- Compares loops (ids, parents, triggers), segments per loop, usage (required/situational/not
  used), element types and min/max lengths, and code lists per element. Maps pyx12 loop ids to our
  loop names explicitly (a table in the script), and reports anything unmapped.
- Outputs (a) a Markdown report: per category, what matches, what differs, what the spec lacks;
  (b) a draft RFC 7386 patch that would close the differences, written to a path given on the
  command line (default under `.superpowers/`, never committed by the script).
- Has an `--check` mode for CI: exits non-zero only for disagreements in elements the spec already
  defines, listing loop, segment, element and both values (P10 shape); things in a
  `scripts/spec_vs_pyx12.ignore.json` list (each entry with a reason) are skipped.
- Tests: a small pytest with a tiny synthetic map and spec in `scripts/tests/` or the py tests
  folder (skipped without `pyx12` only where the real maps are needed).
- Commit: `feat(scripts): cross-check the 835 spec against pyx12 maps`.

### Task 2: Spec format — code lists, version declaration, selection (T53, T54)
- Element definitions (and component definitions, if the format has them) accept
  `"codes": ["...", ...]`. Load-time validation with new `SpecError` variants (P10, key path as
  written): empty list; a code longer than the element's `max` or shorter than its `min`;
  duplicate codes. Full-text `Display` test per variant.
- New diagnostic rule (level 2) for a value outside its element's code list: names the rule,
  segment index and id, element and component position, the loop path, and the value. Emitted by
  the projector's element checks for read and unread elements alike (the unread fast path must
  match the full path; extend `project/unread_tests.rs`). `Display` test.
- Spec-level version declaration as data: e.g. `"version": {"segment": "GS", "element": 8,
  "value": "005010X221A1"}` (optional). Core API to choose among candidate specs for a document:
  find the first segment with that id, compare the element, fall back to a given default. Exact
  shape is the implementer's call within these constraints; it must not name any segment in code.
- Built-ins: `Spec::builtin_835()` stays the 5010 default; add an accessor for the 4010 spec
  (the 5010 spec merge-patched with `specs/835.4010.json`, which in this task holds only the
  version declaration; Task 4 fills it).
- **Default selection (decide with this plan):** recommended — Python `parse`/`parse_file`/
  `stream` without an explicit `spec=` pick the built-in matching the document's declared version,
  else the 5010 default; an explicit `spec=` is always used as given. Rust keeps explicit choice
  (`Processor::run(spec, …)` unchanged) plus the selection helper.
- Commit: `feat: code lists and version declaration in the spec; spec selection by version`.

## Batch B (Opus → Opus review): complete the specs

### Task 3: 5010 spec completed from the maps (T52, T54)
- Run Task 1's script on `specs/835.json` against the 5010 map. Apply the differences that are
  right for our spec (usage, lengths, types, missing segments in loops, code lists from
  `codes.xml` and the map's per-element code references); list deliberate exclusions in
  `scripts/spec_vs_pyx12.ignore.json` with reasons. External code sets (CARC, RARC, …) stay out.
- Re-run goldens: every new diagnostic on united (5010) and on the fixtures is reviewed in the
  report — data issue in the file, or spec too strict. Then regenerate goldens with
  `UPDATE_GOLDEN=1` and commit them with the review summary in the message body.
- Commit: `feat(spec): complete the 5010 spec from pyx12 maps`.

### Task 4: 4010 spec patch and default selection
- Fill `specs/835.4010.json` with the differences of the 4010 map relative to the completed 5010
  spec (usage, codes, lengths, segments), using the script with the patch applied until `--check`
  passes for 4010.
- Wire the default selection decided in Task 2 into the Python binding; expose a way to get a
  built-in by version from Python (`Spec.builtin(version=...)` or similar; name it consistently
  with the existing `Spec` API).
- The five 4010 samples now project with the 4010 spec: review every diagnostic change as in Task
  3; regenerate goldens with the review in the commit body.
- Commit: `feat(spec): 4010 spec as a patch; Python picks the spec by version`.

## Batch C (Sonnet → Opus review): `validate` and wiring

### Task 5: `oxedi835.pyx12.validate` (T55, T56)
- Subpackage `oxedi835.pyx12` behind `[project.optional-dependencies] pyx12 = ["pyx12>=4.0,<5"]`;
  lazy import with an error naming the extra if `pyx12` is missing.
- `validate(data | path | file) -> list[Diagnostic]`, using `pyx12`'s validation API (not the 999
  writer). Each `pyx12` finding becomes our `Diagnostic` with an origin marking it as `pyx12`, the
  rule text and code from `pyx12`, the segment index and byte range (translated through our
  `Document`), element/component position and the datum. If the core `Diagnostic` cannot carry an
  origin, add the smallest core addition that can (and a `Display`/repr test), or wrap it in Python
  with the same fields — pick one and justify it in the report.
- A `pyx12` exception becomes one diagnostic naming the failure, the exception text and the last
  segment reached.
- Tests on the six samples (skipped without `pyx12`): united and davisvision validate with the
  findings `pyx12` itself reports (compare counts against `x12valid -J`), positions point at the
  right bytes; a crafted bad file produces translated diagnostics; a forced exception produces the
  failure diagnostic.
- Commit: `feat(py): validate with pyx12 into oxedi835 diagnostics`.

### Task 6: CI, notices, docs
- `ci.yml` Python job installs `pyx12` and runs `scripts/spec_vs_pyx12.py --check` for 5010 and
  4010, plus the `validate` tests. The release verification keeps working without `pyx12`.
- `THIRD_PARTY_NOTICES`: `pyx12` BSD 3-clause notice for derived spec data; `check_wheel.py` still
  passes.
- READMEs (root and PyPI page): short section on versions (5010 default, 4010 selection) and on
  `oxedi835[pyx12]` validation; `docs/` untouched otherwise.
- `.doc/state.md`, `.doc/roadmap.md`: 5e in PR with results.
- Commit: `docs: pyx12 validation, spec versions; notices`.

## Exit gate (§7 Stage 5e)
- `spec_vs_pyx12.py --check` passes for 5010 and 4010; exclusions listed with reasons.
- Six samples and fixtures parse as before except reviewed, explained diagnostic changes.
- `validate` runs on the six samples with correct positions.
- `make gates` and `make py-test` green, with and without the extra.
