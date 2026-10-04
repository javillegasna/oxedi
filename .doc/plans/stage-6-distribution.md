# Stage 6 · Distribución — Implementation Plan

> Lean plan: mostly configuration. One implementer (Sonnet), one review (Opus), then the
> owner-assisted release run (`v0.1.0rc1` to TestPyPI, then `v0.1.0` to PyPI).

**Goal:** `pip install oxedi835` works on Linux (x86_64, aarch64, musl x86_64), macOS (x86_64,
arm64) and Windows (x86_64) from verified `abi3` wheels published by trusted publishing from a
`v*` tag, with one source of version and a changelog.

**Spec:** `.doc/architectural-commitment.md` §7 "Stage 6 · Distribución" (T30–T36).

## Global Constraints
- No change to library behaviour; `make gates`, `make py-test`, `make dist` green on every commit.
- Core `[dependencies]` unchanged; the base wheel declares no Python dependencies.
- Pin third-party actions by full commit SHA with the version in a comment.
- No secrets in the repo; publishing only via OIDC trusted publishing from the `pypi`/`testpypi`
  environments.
- Comments in workflow files describe what a step does, not history or plan codes.
- `git add` named files; one commit per task.

## Task 1: One source of version (Sonnet)
- `[workspace.package] version = "0.1.0rc1"` in the root `Cargo.toml`; both crates inherit
  (`version.workspace = true`); `crates/oxedi835_py/pyproject.toml` drops `version` and adds
  `dynamic = ["version"]`; check `maturin build` produces `oxedi835-0.1.0rc1-…whl` (maturin
  converts Cargo's `0.1.0-rc.1` / `0.1.0rc1` per its rules — use the Cargo form that maturin
  turns into PEP 440 `0.1.0rc1`, and record which in the report).
- `Makefile`: `version` prints the PEP 440 version read from the built metadata or Cargo;
  `release-check TAG=v…` fails unless the tag equals `v<version>`, the working tree is clean,
  and `CHANGELOG.md` has a section for that version.
- Commit: `build: one version source in the workspace; pyproject reads it`.

## Task 2: CHANGELOG (Sonnet)
- `CHANGELOG.md` in Keep a Changelog format with `## [0.1.0rc1] - <date>` summarizing, for
  users, what 0.1.0 offers (lossless parse, Arrow tables for Polars/pyarrow/DuckDB, diagnostics,
  streaming, the `edi-835-parser` compatibility layer, Python ≥ 3.11, platforms) and an
  `## [Unreleased]` section; link references at the bottom.
- README: a short "Install" section (`pip install oxedi835`, extras) replacing "from a clone"
  as the primary path, keeping the from-source steps below it.
- Commit: `docs: changelog and install instructions for the first release`.

## Task 3: Release workflow (Sonnet)
- `.github/workflows/release.yml`, triggered by `push` of tags `v*` and by `workflow_dispatch`
  (dry run: builds and verifies, never publishes). Jobs:
  1. `check`: `make release-check TAG=${{ github.ref_name }}` (skipped on dispatch).
  2. `sdist`: Linux, `maturin sdist`, `make sdist-check`, upload artifact.
  3. `wheels`: matrix per T30 with `PyO3/maturin-action` (`manylinux: 2_28` for Linux glibc,
     `musllinux_1_2` for musl, `target: aarch64` for the ARM Linux build), `--release`,
     `--manifest-path crates/oxedi835_py/Cargo.toml`, `-o dist`; Windows checkout with
     `git config --global core.symlinks true` before `actions/checkout`; upload artifacts.
  4. `verify`: matrix of OS × Python 3.11/3.13 (ubuntu, macos x86_64 + arm64 runners, windows):
     download the platform's wheel, install it into a fresh venv with the test extras, run
     `pytest crates/oxedi835_py/tests` from outside the source tree with `OXEDI835_CORE_TESTS`
     pointing at the checkout's core tests (as `smoke_wheel.sh` does), and assert the wheel's
     `dist-info/licenses/` holds `LICENSE` and `THIRD_PARTY_NOTICES` identical to the root. The
     aarch64 and musl wheels are verified in a container step (`uraimo/run-on-arch-action` or a
     `python:3.13-alpine` container) running the same commands; if that proves too slow,
     verify them with import + one parse and record the choice.
  5. `publish-testpypi`: needs all verify jobs; runs only for tags with a pre-release suffix;
     `environment: testpypi`; `permissions: id-token: write`; `pypa/gh-action-pypi-publish`
     with `repository-url: https://test.pypi.org/legacy/`.
  6. `publish-pypi`: same, for tags without a suffix; `environment: pypi`.
  7. `github-release`: after publishing, creates the GitHub release for the tag with the
     changelog section as body and the artifacts attached (`gh release create`), marked
     pre-release for suffixed versions.
- Validate the workflow locally with `actionlint` (install it in a temp dir if absent) and run
  it once with `workflow_dispatch` on the branch after pushing (`gh workflow run release.yml
  --ref stage-6-distribution`), watching that build and verify jobs pass on every platform;
  record durations and any platform failure in the report.
- Commit: `ci: release workflow — sdist, abi3 wheel matrix, per-platform verification,
  trusted publishing`.

## Owner and controller steps (outside the implementer's tasks)
- Controller: create GitHub environments `testpypi` and `pypi` with the owner as required
  reviewer (`gh api -X PUT repos/javillegasna/oxedi835/environments/<name>`), restricted to tags
  `v*`.
- Owner: add the trusted publisher on pypi.org and test.pypi.org (project `oxedi835`, owner
  `javillegasna`, repository `oxedi835`, workflow `release.yml`, environment `pypi` /
  `testpypi`).
- Release run: merge the branch, tag `v0.1.0rc1` on master, approve the `testpypi` deployment,
  check `pip install --pre --index-url https://test.pypi.org/simple/ oxedi835` on a machine;
  then bump to `0.1.0`, update the changelog, tag `v0.1.0`, approve `pypi`.
- Afterwards: revoke the account-wide PyPI and TestPyPI tokens and remove `PYPI_API_TOKEN`,
  `pypi_test_api_token` secrets and `~/.pypirc` entries.

## Exit gate
- The dispatch dry run is green on every platform; `make gates`, `make py-test`, `make dist`
  green; `actionlint` clean; then the §7 gate (rc1 on TestPyPI, 0.1.0 on PyPI, GitHub release,
  tokens revoked).
