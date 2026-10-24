# CLAUDE.md — how work is done in oxedi

Lossless, fast, data-driven EDI 835 parser core in Rust (edition 2024); learning Rust in
depth is a co-equal goal. Public repo `javillegasna/oxedi`. Conversation language is
Spanish; code, commits, PRs and issues are in English.

## Where the truth lives

`.doc/` is local and git-ignored: the maintainers' working notes, never committed. The only
public planning page is `docs/roadmap.md` on the site.

- `.doc/architecture/`: the contract, one file per part; start at its `README.md`.
  `contract.md` holds the invariants (N1–N7) and principles (P1–P10), `processing-model.md`
  and `stage-map.md` the model and the map, `decisions/` the index of every decision (taken
  T*, open D*), and `stages/` one commitment file per stage with its decisions, deliverable,
  gate and scope, listed in `stages/README.md`. A stage starts only after its commitment file
  is approved by the owner; a plan is written only after that. A new stage is a new file plus
  one row in each index, never an edit at the end of a long document.
- `.doc/roadmap.md`: stage status, what each unlocks, open decisions by stage.
- `.doc/plans/stage-N-*.md`: executable plans with full code, TDD steps and an exit gate.
- `.doc/state.md`: current snapshot and next steps (update it when a stage changes state).
- `.doc/analysis/`: the owner's study notes with Mermaid diagrams about the
  code (structure, flow, tests and their reasons, Rust concepts, patterns). Update them per
  stage from the real code, not from plans; classDef colours need `color:#111`.
- `docs/roadmap.md` (public, on the site): a short, user-facing summary in English: available
  now, next, being considered; no stage codes, decision ids or internal detail. Update it with
  one line when a stage ships or the next item changes.
- GitHub Project #8 "oxedi835 — Roadmap & Deuda Técnica", the one board: work organized as
  initiative → epic → task. Every issue gets `Level`, `Initiative`, `Priority` and `Size`;
  epics are issues whose tasks are native sub-issues. Deferred review findings are issues
  (Context / Problem / Recommendation, no solutions). Status moves with the work: the
  current sprint (about 15 days) in Sprint Backlog, In Progress before work starts, In Review
  when its PR opens, Done when the PR closes it; Done items are archived. An issue a change
  touches is fixed or fulfilled and closed by that change, never left out of date.

## Non-negotiables in code

The code conventions and test-data rules are in `CONTRIBUTING.md` (sections Code conventions
and Tests and test data), imported here; follow them exactly as written there.
`ARCHITECTURE.md` is the public map of the code.

@CONTRIBUTING.md

Agent-specific additions:

- The public names map to the contract's codes: "Lossless" is N1, "Errors explain themselves"
  is P10.
- "Comments describe the code, not its history" also excludes stage numbers, principle codes
  (N1, P3) and links to `.doc/` or other planning docs.
- Each new dependency is approved in a stage's commitment file before it is added.

Rules for every change and refactor, checked by implementers and reviewers alike:

- Null safety and security come first by default. No `None`/null sentinel for "the default"
  and no untyped values (`Any`, `dict[str, Any]`, stringly-typed options) in a Rust or Python
  API: a default is the typed value (rendered in the stub), an absence is a type. Conversions
  are checked (no silent truncation, wrap or fallback). When readability or brevity competes
  with null safety or security, null safety and security win.
- Touch it, fix it: a change that modifies a function listed in
  `.github/complexity-baseline.txt` leaves it within the targets; `make complexity` fails
  with `TOUCHED` otherwise. Only flat per-variant dispatch is accepted (`flat`), with its
  reason in the PR.
- Fix the cause, never only split a function: choose the fitting pattern from any family
  (design, architecture, functional, dynamic programming and memoization, concurrency- and
  parallel-friendly data, algorithms and data structures) and name it in the report.

## How a stage runs

1. Discuss open decisions with trade-offs in chat; write the stage's commitment file in
   `.doc/architecture/stages/`; owner approves.
2. Write the plan (superpowers:writing-plans); owner approves.
3. Feature branch from `master`. Execute with superpowers:subagent-driven-development.
   The main session only orchestrates: it plans and dispatches, never implements. One
   implementer per task, chosen by complexity: Haiku transcribes complete code from the
   plan, Sonnet handles prose-described or multi-file tasks, Opus takes design judgment.
   The reviewer is one tier above the implementer (Haiku work → Sonnet review, Sonnet
   work → Opus review, Opus work → Opus review). Token budget rule (since Stage 4b): group
   consecutive tasks into batches of two to four for one implementer, review per batch, not
   per task; the plan carries executed code only for design-heavy tasks and precise prose
   for mechanical ones. Every dispatch and review carries the rules above. A whole-branch
   review on Opus at the end with P10, null safety and security as explicit focus (triage,
   not a second audit), one fix wave, one scoped re-review. Rulings go in the
   ledger; the ledger is copied to `.doc/analysis/stage-N-ledger.md` before the workspace
   is deleted.
4. PR against `master`, body in English with two sections, **Intent** and **Verification**;
   never list the diff content; no attribution lines. The owner merges.
5. Deferred minors become issues on Project #8. Then update `.doc/roadmap.md`,
   `.doc/state.md` and, when what users can do changed, `docs/roadmap.md`.

## Commands

`make help` lists every target. The gates and the Python binding have make targets; the raw
commands are kept here because CI runs them directly.

```bash
make gates            # fmt-check, clippy -D warnings, test, bench --no-run, doc -D warnings
make py-test          # maturin develop into .venv, then pytest crates/oxedi_py/tests
make dist && make smoke   # sdist + release wheel, then install in a clean venv outside the repo
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all && cargo fmt --all -- --check
UPDATE_GOLDEN=1 cargo test -p oxedi_core --test engine_golden   # regenerate goldens, then inspect
UPDATE_GOLDEN=1 cargo test -p oxedi_core --test project_golden  # table and diagnostic goldens
cargo bench --workspace                                           # ~5 min; baselines go in commit messages
```

Publishing (`make publish-test`, `make publish`, `make tag`) reads `~/.pypirc`; tokens never
enter the repo. Python floor 3.11; the base wheel declares no Python dependencies.
