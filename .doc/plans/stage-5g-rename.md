# Stage 5g · Rename to `oxedi` — Implementation Plan

> Lean plan: one mechanical but wide task for one implementer (Sonnet), Opus review, one fix wave.

**Goal:** the project, crates and Python package are named `oxedi` (§7 Stage 5g, T70–T74, approved
2026-10-05). The GitHub repo is already `javillegasna/oxedi`.

## Task 1: rename
- `git mv crates/edi835_core crates/oxedi_core`, `git mv crates/oxedi835_py crates/oxedi_py`;
  package names `oxedi_core` and `oxedi_py` in their `Cargo.toml`; workspace members; every
  `use edi835_core` → `use oxedi_core`; `Cargo.lock` regenerated with `--locked`-compatible update.
- Python: `crates/oxedi_py/python/oxedi835/` → `python/oxedi/`; `pyproject.toml` name `oxedi`,
  `module-name = "oxedi._core"`, URLs to `https://github.com/javillegasna/oxedi`; the pyo3 module
  and every class `module = "oxedi"`; stub generator target path; regenerate `_core.pyi` with
  `make stubs`; tests `import oxedi`; extras unchanged.
- Version: workspace `0.3.0` (T71); CHANGELOG `[Unreleased]` → Changed: the package is now
  `oxedi` (`pip install oxedi`, `import oxedi`); `oxedi835` is retired on PyPI.
- Everything else that names the old crates, package or repo: Makefile, `scripts/`,
  `.github/workflows/*.yml` (including the CI path filters and release checks such as
  `check_wheel.py`, `smoke_wheel.sh`, `verify_wheel.sh`), the DuckDB crate (its dependency on the
  core, its oracle test `import oxedi`, its README and `description.yml` `repo.github`), READMEs,
  `docs/`, `.doc/state.md`, `.doc/roadmap.md`, and `CLAUDE.md` lines naming crates, paths and
  commands (owner permission given; keep the Project #8 board title as is).
- Do NOT rewrite history (T73): `.doc/plans/*` of past stages, approved §7 sections, ledgers,
  and published CHANGELOG entries keep their names.
- Commit as one or a few commits; the first one a pure `git mv` so history follows.

## Exit gate
- `git grep -n -e oxedi835 -e edi835_core` matches only T73 history and the CHANGELOG notice.
- `make gates`; `make py-test` with and without pyx12; `make stubs` then `git diff --exit-code`;
  `make stubtest`; `make release`, `make test_release`, `make duckdb-oracle`;
  `make dist && make smoke` gives `oxedi-0.3.0-*.whl` and `import oxedi` works; no golden content
  changes; `make release-check TAG=v0.3.0` on a clean tree (after a dated 0.3.0 section is NOT
  required here — release is a separate PR).
