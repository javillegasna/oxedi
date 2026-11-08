# AGENTS.md — how work is done in oxedi

Lossless, fast, data-driven EDI 835 parser core in Rust (edition 2024), with Python, DuckDB and
browser bindings. Public repo `javillegasna/oxedi`. Code, commits, pull requests and issues are in
English. These instructions are for any coding agent; a person contributing follows the same
rules from `CONTRIBUTING.md`.

## Non-negotiables in code

Follow these sections of `CONTRIBUTING.md` exactly as written there: **Code conventions**,
**Tests and test data**, **Pull requests** and **Complexity**. `ARCHITECTURE.md` is the public map
of the code.

- Open an issue before adding any dependency, as `CONTRIBUTING.md` says; the maintainer
  approves each one before it is added.

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
  parallel-friendly data, algorithms and data structures) and name it in the PR.
- Lower complexity without raising cost. Chaining iterator passes is fine on cold paths (once
  per spec, file or option set); on hot paths (per segment, element, cell or row) keep or
  improve the Big-O and the allocations: no nested scans, no search over a growing collection
  inside a loop, no repeated `collect`. Where it matters, choose the algorithm or
  architecture (single pass, an index built once, streaming), and run the affected
  `cargo bench` groups before and after, with the numbers in the commit message.
- Keep verification cheap: compare benchmarks with only the affected groups and short times
  (`cargo bench -p oxedi_core --bench <group> -- --warm-up-time 1 --measurement-time 3`) and
  leave the full measurement to the final review; when checking each commit in a temporary
  worktree, share one `CARGO_TARGET_DIR` so only what changed is rebuilt.
- An open issue a change touches is fixed or fulfilled and closed by that change
  (`Closes #N`), never left out of date.
- Names say what the work does: a branch, a commit subject, a pull request title or a plan is
  named after its change (`spec-vocabulary-tables`), never after a phase or round number.
- A PR changes at most 400 lines (additions plus deletions, tests included, generated goldens
  and stubs excluded); a larger change is planned as several PRs, each merging on its own with
  its own gate. Its body is in English with two sections, **Intent** and **Verification**; never
  list the diff content; no attribution lines. The maintainer merges.

## Agents in the sandbox

- Each implementer works in its own worktree of the feature branch, never in the main checkout.
- Every command that runs project or dependency code runs as `make sandbox CMD="..."`:
  `make gates`, `make py-test`, `make viewer-test` (with `IMAGE=viewer`) and a single
  `cargo test` alike. Only the targets `CONTRIBUTING.md` lists under "What stays on the host" run
  on the host, and so does `.github/scripts/lint.sh`, which runs the sandbox's own tests: they
  need `git`, which the sandbox image does not carry.
- Docker commands use the rootful daemon for that command only, as
  `env -u DOCKER_HOST DOCKER_HOST=unix:///var/run/docker.sock make sandbox CMD="..."`; never
  change the shell's or Docker's configuration. When that daemon does not answer, the agent stops
  and asks the owner to start it.
- One sandbox run per worktree at a time: a second one is refused while the first runs. Do not
  commit while a run of the same worktree is in progress, since its audit reports the change.
- Implementers run in parallel only when the work is split into batches with no file in common,
  each in its own worktree; they are integrated in batch order.

## Final review focus

- Errors explain themselves: every error variant says what failed, where and what to do.
- Null safety and security, as in the non-negotiables above, across the whole branch.

## Where work comes from

Open work anyone can take is a GitHub issue; issues labelled `good first issue` carry a
**Pointers** section. Comment on the issue before starting, work from it, `CONTRIBUTING.md` and
`ARCHITECTURE.md`, and send one pull request that closes it. The maintainer plans the rest
privately; the public summary of what comes next is `docs/roadmap.md`.

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
