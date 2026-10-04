# CLAUDE.md — how work is done in oxedi835

Lossless, fast, data-driven EDI 835 parser core in Rust (edition 2024); learning Rust in
depth is a co-equal goal. Public repo `javillegasna/oxedi835`. Conversation language is
Spanish; code, commits, PRs and issues are in English.

## Where the truth lives

- `.doc/architectural-commitment.md`: the contract. §2 invariants (N1–N7), §3 principles
  (P1–P10), §4 processing model, §5 stage map, §6 decisions (taken T*, open D*), §7 one
  approved section per stage. A stage starts only after its §7 section is approved by the
  owner; a plan is written only after that.
- `.doc/roadmap.md`: stage status, what each unlocks, open decisions by stage.
- `.doc/plans/stage-N-*.md`: executable plans with full code, TDD steps and an exit gate.
- `.doc/state.md`: current snapshot and next steps (update it when a stage changes state).
- `.doc/analysis/` (git-ignored): the owner's study notes with Mermaid diagrams about the
  code (structure, flow, tests and their reasons, Rust concepts, patterns). Update them per
  stage from the real code, not from plans; classDef colours need `color:#111`.
- GitHub Project #8 "oxedi835 — Roadmap & Deuda Técnica": deferred review findings as
  issues (Context / Problem / Recommendation, no solutions), Backlog + priority.

## Non-negotiables in code

- Lossless on every file: every input byte lands in exactly one `Segment::raw`; unknown
  input is data or an event, never dropped.
- No `unwrap`/`expect`/`panic!` or fallible indexing on input in `src/` (documented
  exception: `Spec::builtin_835`). Indexing with ids the owner created (`LoopId`, `NodeId`)
  is fine.
- Core is sans-IO: `[dependencies]` holds only `serde` and `serde_json`; nothing that does
  I/O, threads or a runtime. The Python binding (`crates/oxedi835_py`) delegates everything to
  the core, holds the only `arrow-*`/`pyo3` dependencies, and names no 835 segment.
- Comments and doc comments describe implementation only: no stage numbers, principle
  codes (N1, P3), history or links to planning docs.
- Module layout: one folder per module (`x/mod.rs`, tests in `x/tests.rs` or `x/tests/` by
  topic), enforced by clippy `self_named_module_files`. About 400 lines of code per file,
  tests excluded; a longer file needs a reason it is one coherent piece. A folder's `mod.rs`
  re-exports and lists what each file holds.
- P10: every error or diagnostic names the rule that failed, where (loop and key as written
  for specs; segment index, byte range, element/component position and loop path for files)
  and the offending datum; one full-text `Display` test per variant; `source()` chains.
- Gates on every commit: `cargo test --workspace --locked`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`,
  `cargo bench --workspace --no-run --locked`. CI runs the same.
- Fixtures (`tests/fixtures/`, synthetic) and samples (`tests/samples/`, anonymized real
  payer files) are never edited. The real originals and the re-identification key live
  outside the repo; never commit them.

## How a stage runs

1. Discuss open decisions with trade-offs in chat; write the §7 section; owner approves.
2. Write the plan (superpowers:writing-plans); owner approves.
3. Feature branch from `master`. Execute with superpowers:subagent-driven-development.
   The main session only orchestrates: it plans and dispatches, never implements. One
   implementer per task, chosen by complexity: Haiku transcribes complete code from the
   plan, Sonnet handles prose-described or multi-file tasks, Opus takes design judgment.
   The reviewer is one tier above the implementer (Haiku work → Sonnet review, Sonnet
   work → Opus review, Opus work → Opus review). Token budget rule (since Stage 4b): group
   consecutive tasks into batches of two to four for one implementer, review per batch, not
   per task; the plan carries executed code only for design-heavy tasks and precise prose
   for mechanical ones. A whole-branch review on Opus at the end with P10 as an explicit
   focus (triage, not a second audit), one fix wave, one scoped re-review. Rulings go in the
   ledger; the ledger is copied to `.doc/analysis/stage-N-ledger.md` before the workspace
   is deleted.
4. PR against `master`, body in English with two sections, **Intent** and **Verification**;
   never list the diff content; no attribution lines. The owner merges.
5. Deferred minors become issues on Project #8. Then update `.doc/roadmap.md` and
   `.doc/state.md`.

## Commands

`make help` lists every target. The gates and the Python binding have make targets; the raw
commands are kept here because CI runs them directly.

```bash
make gates            # fmt-check, clippy -D warnings, test, bench --no-run, doc -D warnings
make py-test          # maturin develop into .venv, then pytest crates/oxedi835_py/tests
make dist && make smoke   # sdist + release wheel, then install in a clean venv outside the repo
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all && cargo fmt --all -- --check
UPDATE_GOLDEN=1 cargo test -p edi835_core --test engine_golden   # regenerate goldens, then inspect
UPDATE_GOLDEN=1 cargo test -p edi835_core --test project_golden  # table and diagnostic goldens
cargo bench --workspace                                           # ~5 min; baselines go in commit messages
```

Publishing (`make publish-test`, `make publish`, `make tag`) reads `~/.pypirc`; tokens never
enter the repo. Python floor 3.11; the base wheel declares no Python dependencies.
