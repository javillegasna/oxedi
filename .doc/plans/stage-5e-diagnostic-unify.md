# Stage 5e amendment · One diagnostic type — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan. Lean plan: Task 1 carries the core code (design-bearing); Task 2 is precise
> prose. One batch, Opus implements, Opus reviews, Opus final triage.

**Goal:** `oxedi835.pyx12.validate` returns `list[oxedi835.Diagnostic]` (the core type), so findings
from `parse` and from `validate` mix, sort by level and filter by origin; `Pyx12Diagnostic` goes away.

**Architecture:** the core gains a tool-agnostic `Rule::External { origin, code, message, level }`;
the binding gains an internal constructor for it plus `Diagnostic.origin`, `Diagnostic.code` and
`Segment.span`; the pyx12 adapter builds core diagnostics and assigns levels from where pyx12
reports a finding, never from its error codes.

**Tech Stack:** Rust 2024 (`edi835_core`), pyo3 (`crates/oxedi835_py`), Python ≥ 3.11, pyx12 4.x
(extra only).

**Spec:** `.doc/architectural-commitment.md` §7 "Enmienda al Stage 5e · Un solo tipo de
diagnóstico" (T58–T61), approved 2026-10-04. Branch `stage-5e-pyx12` (PR #80, not merged).
Ledger: `.superpowers/sdd/stage-5e-unify/progress.md`.

## Global Constraints
- `CLAUDE.md` non-negotiables: no `unwrap`/`expect`/`panic!` or fallible indexing on input in
  `src/`; the core names no tool and no 835 segment (`pyx12` appears only as data passed by the
  adapter); the binding names no 835 segment; comments describe implementation only; P10 (rule,
  place, datum; one full-text `Display` test per variant); ~400 code lines per file.
- No golden changes. `make gates` and `make py-test` green, with and without pyx12.
- `Pyx12Diagnostic` and `crates/oxedi835_py/python/oxedi835/pyx12/_diagnostic.py` are removed
  (never published); nothing re-exports them.
- Levels for external findings (T59): pyx12 interchange/group/transaction errors and every pyx12
  failure (rejected file, exception, untranslatable report) → 1; segment and element errors → 2.
  No table keyed by pyx12 error codes anywhere.
- `Rule` becomes `#[non_exhaustive]`.

## Review Focus
1. A list mixing `parse(...).diagnostics` and `validate(...)` sorts by `level` and filters by
   `origin` without type errors (gate test in Task 2).
2. Byte positions are preserved: every byte range the 5e tests pinned through
   `Pyx12Diagnostic.span` is now reached as `document[d.segment].span` with the same values
   (eyemed `(7468, 7485)`, BOM/CRLF/BOM+LF prefixes, inserted `ZZZ*1~`).
3. The internal constructor rejects a level outside 1–3 with `ValueError` naming the value; it
   never panics.
4. `Segment.span` is the raw range (leading trivia, body, terminator), including on a first
   segment with a BOM and on a last segment with no terminator.
5. `Diagnostic.origin` is `"oxedi835"` and `Diagnostic.code` is `None` for every core rule; the
   `__repr__` of an external diagnostic shows its origin.

---

## Task 1: `Rule::External` in the core; binding getters, constructor and `Segment.span`

**Files:**
- Modify: `crates/edi835_core/src/diagnostic/mod.rs` (variant, `level`, `kind`, `Display`,
  `#[non_exhaustive]`)
- Modify: `crates/edi835_core/src/diagnostic/tests.rs` (full-text `Display` test)
- Modify: `crates/oxedi835_py/src/diagnostic.rs` (`origin`, `code` getters, `__repr__`,
  `external_diagnostic` function)
- Modify: `crates/oxedi835_py/src/document.rs` (`PySegment::span`)
- Modify: `crates/oxedi835_py/src/lib.rs` (register `_external_diagnostic`)
- Test: `crates/oxedi835_py/tests/test_diagnostics.py`, `crates/oxedi835_py/tests/test_document.py`

**Interfaces produced (Task 2 relies on these exact names):**
- Rust: `Rule::External { origin: String, code: Option<String>, message: String, level: SnipLevel }`;
  `Rule::kind()` → `"External"`; `Rule::level()` → the variant's `level`.
- Python: `oxedi835._core._external_diagnostic(origin: str, message: str, level: int,
  code: str | None = None, segment: int | None = None, element: int | None = None,
  component: int | None = None, datum: bytes = b"") -> oxedi835.Diagnostic` (path empty).
- Python: `Diagnostic.origin -> str`, `Diagnostic.code -> str | None`,
  `Segment.span -> tuple[int, int]` (raw range: start inclusive, end exclusive).

- [ ] **Step 1: Write the failing core test** in `diagnostic/tests.rs`:

```rust
#[test]
fn external_displays_the_message_the_origin_and_the_code() {
    let rule = |code: Option<&str>| Rule::External {
        origin: "pyx12".into(),
        code: code.map(Into::into),
        message: "Mandatory data element missing".into(),
        level: SnipLevel::L2,
    };
    let diagnostic = Diagnostic::new(
        rule(Some("1")),
        Some(17),
        Some(2),
        None,
        Vec::new(),
        b"".to_vec(),
    );
    assert_eq!(diagnostic.level, SnipLevel::L2);
    assert_eq!(diagnostic.rule.kind(), "External");
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · Mandatory data element missing (reported by pyx12, code 1) · segment #17, element 2 · datum \"\""
    );
    assert_eq!(
        rule(None).to_string(),
        "Mandatory data element missing (reported by pyx12)"
    );
}
```

  Adjust only the parts of the expected string that `Diagnostic`'s existing `Display` decides
  (separators, how an empty path and an empty datum render): copy them from the neighbouring
  `code_not_in_list_*` test, and keep the rule text exactly as above.

- [ ] **Step 2:** `cargo test -p edi835_core diagnostic` → fails to compile (no variant).

- [ ] **Step 3: Implement** in `diagnostic/mod.rs`:

```rust
/// The rule a diagnostic reports, with the values its message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rule {
    // ... existing variants unchanged ...
    /// A finding reported by a validator outside this crate, carried as that
    /// validator states it.
    External {
        /// Who reported the finding, e.g. the validator's name.
        origin: String,
        /// The validator's own code for the finding, when it gives one.
        code: Option<String>,
        /// The validator's description of the finding.
        message: String,
        /// The SNIP level the caller assigns to the finding.
        level: SnipLevel,
    },
}
```

  `level()`: add `Rule::External { level, .. } => *level`. `kind()`: add
  `Rule::External { .. } => "External"`. `Display`:

```rust
            Rule::External {
                origin,
                code,
                message,
                ..
            } => match code {
                Some(code) => write!(f, "{message} (reported by {origin}, code {code})"),
                None => write!(f, "{message} (reported by {origin})"),
            },
```

  `SnipLevel` is `Copy`. If `diagnostic/mod.rs` crosses its
  current size noticeably, leave it (splitting is a filed follow-up), but do not add other code.

- [ ] **Step 4:** `cargo test -p edi835_core` passes; `cargo clippy --workspace --all-targets
  --locked -- -D warnings` clean (`#[non_exhaustive]` needs no wildcard inside the crate; check
  the binding and tests compile).

- [ ] **Step 5: Binding.** In `crates/oxedi835_py/src/diagnostic.rs`:
  - getter `origin`: the variant's `origin` for `Rule::External`, else `"oxedi835"`;
  - getter `code`: the variant's `code` for `Rule::External`, else `None`;
  - `__repr__`: insert `origin='…'` after `level=` (update existing repr tests);
  - `#[pyfunction] #[pyo3(name = "_external_diagnostic", signature = (origin, message, level,
    code=None, segment=None, element=None, component=None, datum=b"".to_vec()))]` returning
    `PyResult<PyDiagnostic>`; `level` maps 1/2/3 to `SnipLevel::L1/L2/L3`, anything else raises
    `ValueError` with the text `level must be 1, 2 or 3, got {level}`; build with
    `Diagnostic::new(Rule::External { … }, segment, element, component, Vec::new(), datum)`.
  Register it in `lib.rs` with `m.add_function(wrap_pyfunction!(…, m)?)?`. Docstrings say it is
  internal (leading underscore; adapters use it).

- [ ] **Step 6: `Segment.span`.** In `document.rs`, `#[getter] fn span(&self, py) ->
  PyResult<(usize, usize)>` reading `document.inner.span(self.index)` and returning
  `(span.raw.start, span.raw.end)`; out of range raises the same `IndexError` text as `with`.

- [ ] **Step 7: Python tests** (`test_diagnostics.py`, `test_document.py`):
  - every diagnostic from `parse` on a fixture with findings has `origin == "oxedi835"` and
    `code is None`;
  - `_external_diagnostic("pyx12", "msg", 2, code="1", segment=3, element=2, datum=b"X")` has
    `kind == "External"`, `level == 2`, `origin == "pyx12"`, `code == "1"`, `path == ""`,
    `str(d)` equal to the full text, and `isinstance(d, oxedi835.Diagnostic)`;
  - level `0` and `4` raise `ValueError` matching `level must be 1, 2 or 3, got 0`;
  - `document[i].span` equals `(start, end)` such that `data[start:end] == document[i].raw` for
    every segment of one sample; the spans partition the input; on a BOM-prefixed input the first
    span starts at 0; on an input whose last segment has no terminator the last span ends at
    `len(data)`.

- [ ] **Step 8:** `make gates` and `make py-test` green. Commit:
  `feat: external findings as a core rule; Diagnostic.origin/code and Segment.span`.

## Task 2: `validate` returns core diagnostics

**Files:**
- Modify: `crates/oxedi835_py/python/oxedi835/pyx12/_validate.py`, `pyx12/__init__.py`
- Delete: `crates/oxedi835_py/python/oxedi835/pyx12/_diagnostic.py`,
  `crates/oxedi835_py/tests/test_pyx12_diagnostic.py`
- Modify: `crates/oxedi835_py/tests/test_pyx12.py`
- Modify: `README.md`, `crates/oxedi835_py/README.md`, `.doc/state.md` (one line: amendment in
  PR #80), `.doc/roadmap.md` 5e row (`validate` returns `Diagnostic`)

**Consumes:** `oxedi835._core._external_diagnostic`, `Diagnostic.origin/code`, `Segment.span`
(Task 1).

- [ ] **Step 1: Tests first** in `test_pyx12.py` (pyx12-skipped as today): change every
  assertion on `Pyx12Diagnostic` to the core type:
  - `kind` filters (`Pyx12Failure`, `Pyx12SegmentError`, …) become `d.origin == "pyx12"` plus
    `d.level` / `d.code` / `d.element is not None` as the case needs; a failure is
    `level == 1 and code is None` and its `rule` starts with `pyx12 could not finish validating`;
  - every `d.span` becomes `document[d.segment].span` with the same expected numbers;
  - every `segment_name` assertion is removed (or checked inside `d.rule` only if the message
    carries it);
  - keep the `str(d)` expectations, rewritten to the core `Display` shape
    (`SNIP n · <message> (reported by pyx12[, code c]) · segment #… · datum "…"`);
  - add the gate test: `findings = parse(data).diagnostics + validate(data)` on a sample with
    both kinds (or a crafted file), `sorted(findings, key=lambda d: d.level)` works,
    `{d.origin for d in findings} == {"oxedi835", "pyx12"}`, and every item is an
    `oxedi835.Diagnostic`;
  - add: envelope errors (interchange/group/transaction) come out at level 1, segment/element
    errors at level 2, failures at level 1.
  Run `make py-test` → the new expectations fail.

- [ ] **Step 2: Adapter.** In `_validate.py`, build every finding with
  `_external_diagnostic("pyx12", message, level, code=…, segment=…, element=…, component=…,
  datum=…)`; levels per Global Constraints, decided by which branch of `_translate` emits the
  finding (node scope vs segment vs element) and by `_failure`, never by `err_cde`. The message is
  pyx12's `err_str` (for failures, today's sentence). Drop `span` and `segment_name`; keep
  `_Positions` only for mapping pyx12 lines to segment indexes. `validate` is annotated
  `-> list[Diagnostic]`; its docstring describes the core type, the levels, `origin`/`code`, the
  empty `path`, and `document[d.segment].span` for bytes. Delete `_diagnostic.py`; `__all__ =
  ["validate"]`.

- [ ] **Step 3: Docs.** READMEs: the validation section shows `validate` returning
  `oxedi835.Diagnostic` with `origin == "pyx12"`, mixing with `parse` diagnostics, and bytes via
  `document[d.segment].span`; remove `Pyx12Diagnostic`. `.doc/state.md` "Pick up here" and the
  roadmap 5e row mention the amendment.

- [ ] **Step 4:** `make py-test` with pyx12 and without it (uninstall, run, reinstall
  `pyx12>=4.0,<5`), `make gates`, `.venv/bin/python scripts/spec_vs_pyx12.py --check` for 5010 and
  `--version 4010` (unchanged, sanity). Commit: `feat(py): validate returns core diagnostics`.

## Exit gate
- `validate` returns `oxedi835.Diagnostic`; mixed lists sort by level and filter by origin.
- Every byte range pinned in 5e is reproduced through `Segment.span`.
- No golden change; `make gates`, `make py-test` (with and without pyx12) green.
