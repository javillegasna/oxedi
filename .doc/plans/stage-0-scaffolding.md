# Stage 0 — Andamiaje y arnés de verificación · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `oxedi835` Cargo workspace with a working verification harness (tests, property tests, benchmarks, CI gates) before any parsing logic exists.

**Architecture:** A multi-crate Cargo workspace rooted at `oxedi835/`, containing only `crates/edi835_core` for now. Verification infrastructure is wired from commit zero: inline unit tests, a `proptest` harness, a `criterion` benchmark skeleton, an integration test that proves the fixture-loading pipeline, and a CI workflow enforcing build/test/clippy/fmt gates.

**Tech Stack:** Rust (edition 2024, stable toolchain), `proptest`, `criterion`, GitHub Actions (`dtolnay/rust-toolchain`).

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/` unless stated otherwise. This directory already exists (it holds `.doc/`).

---

## File Structure

```
oxedi835/
├── Cargo.toml                         # workspace manifest (members, resolver, shared metadata)
├── Cargo.lock                         # committed (project, not just a lib)
├── .gitignore                         # ignore /target
├── rust-toolchain.toml                # pin stable + clippy/rustfmt
├── README.md                          # minimal project intro
├── .github/workflows/ci.yml           # lint + test gates
├── .doc/                              # (exists) architectural-commitment.md, plans/
└── crates/
    └── edi835_core/
        ├── Cargo.toml                 # crate manifest + dev-deps + [[bench]]
        ├── src/lib.rs                 # crate root + smoke unit test (no logic)
        ├── benches/smoke.rs           # criterion skeleton bench
        └── tests/
            ├── common/mod.rs          # shared fixture-loading helper
            ├── proptest_harness.rs    # trivial property test (proves proptest runs)
            ├── fixtures_harness.rs     # integration test: fixtures load & are non-empty
            └── fixtures/              # copied from the fast_edi835 POC
                ├── blue_cross_nc_sample.txt
                ├── united_healthcare_legacy_sample.txt
                ├── trizetto_sample.rmt
                ├── emedny_sample.txt
                └── multi_claim_sample.txt
```

Each file has one responsibility: manifests declare structure, `lib.rs` is the (currently empty) crate root, and each `tests/*.rs` proves one piece of the verification pipeline independently.

---

## Task 0: Initialize repo + workspace skeleton

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `rust-toolchain.toml`, `README.md`
- Create: `crates/edi835_core/Cargo.toml`, `crates/edi835_core/src/lib.rs`

- [ ] **Step 1: Initialize git**

Run:
```bash
git init
```
Expected: `Initialized empty Git repository in .../oxedi835/.git/`

- [ ] **Step 2: Create the workspace manifest**

Create `Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/edi835_core"]

[workspace.package]
edition = "2024"
license = "MIT"
authors = ["javillegasna <javillegasna@gmail.com>"]
repository = "https://github.com/javillegasna/oxedi835"

[workspace.lints.clippy]
# Project-wide lint posture; CI enforces with -D warnings.
all = "warn"
```

- [ ] **Step 3: Create `.gitignore`**

Create `.gitignore`:
```gitignore
/target
**/*.rs.bk
```

- [ ] **Step 4: Pin the toolchain**

Create `rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
```

- [ ] **Step 5: Create the README**

Create `README.md`:
```markdown
# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 0 — scaffolding.** No parsing logic yet.
```

- [ ] **Step 6: Create the core crate manifest**

Create `crates/edi835_core/Cargo.toml`:
```toml
[package]
name = "edi835_core"
version = "0.0.0"
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Lossless, fast, data-driven EDI 835 parser core (oxedi835)"

[lints]
workspace = true

[dependencies]

[dev-dependencies]
proptest = "1"
criterion = "0.5"
```

- [ ] **Step 7: Create the crate root with a smoke test**

Create `crates/edi835_core/src/lib.rs`:
```rust
//! `edi835_core` — lossless, fast, data-driven EDI 835 parser core.
//!
//! Stage 0: scaffolding only. No parsing logic yet — see
//! `.doc/architectural-commitment.md` for the roadmap.

#[cfg(test)]
mod tests {
    /// Smoke test: proves the unit-test harness compiles and runs.
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 8: Build and run the smoke test**

Run:
```bash
cargo test --workspace
```
Expected: PASS — `test tests::smoke ... ok`, `test result: ok. 1 passed`.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: initialize oxedi835 workspace with edi835_core crate"
```

---

## Task 1: Wire the property-testing harness

**Files:**
- Create: `crates/edi835_core/tests/proptest_harness.rs`

- [ ] **Step 1: Write the property test**

Create `crates/edi835_core/tests/proptest_harness.rs`:
```rust
//! Proves the `proptest` harness runs. In Stage 1 this file's pattern becomes
//! the real tokenize→reconstruct round-trip property.

use proptest::prelude::*;

proptest! {
    /// Reversing a string twice yields the original — a trivial involution,
    /// here only to exercise the property-test machinery end to end.
    #[test]
    fn string_reverse_is_involutive(s in ".*") {
        let reversed: String = s.chars().rev().collect();
        let back: String = reversed.chars().rev().collect();
        prop_assert_eq!(s, back);
    }
}
```

- [ ] **Step 2: Run it to verify the harness works**

Run:
```bash
cargo test --workspace --test proptest_harness
```
Expected: PASS — `test string_reverse_is_involutive ... ok`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test: wire proptest harness with placeholder property"
```

---

## Task 2: Wire the criterion benchmark skeleton

**Files:**
- Modify: `crates/edi835_core/Cargo.toml` (add `[[bench]]`)
- Create: `crates/edi835_core/benches/smoke.rs`

- [ ] **Step 1: Declare the bench target**

In `crates/edi835_core/Cargo.toml`, append after the `[dev-dependencies]` block:
```toml
[[bench]]
name = "smoke"
harness = false
```

- [ ] **Step 2: Write the skeleton benchmark**

Create `crates/edi835_core/benches/smoke.rs`:
```rust
//! Criterion skeleton. Measures nothing meaningful yet — it exists so that
//! `cargo bench` is part of the workflow from commit zero (N4: perf-conscious).

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn smoke_benchmark(c: &mut Criterion) {
    c.bench_function("smoke_add", |b| b.iter(|| black_box(1) + black_box(1)));
}

criterion_group!(benches, smoke_benchmark);
criterion_main!(benches);
```

- [ ] **Step 3: Verify benches compile (fast, no full run)**

Run:
```bash
cargo bench --workspace --no-run
```
Expected: Compiles cleanly; prints an `Executable ... (target/release/deps/smoke-...)` line, no errors.

- [ ] **Step 4: Verify the bench actually runs**

Run:
```bash
cargo bench --workspace
```
Expected: Criterion prints `smoke_add  time: [...]` with timing stats and exits 0.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "bench: add criterion skeleton so cargo bench is wired"
```

---

## Task 3: Bring in fixtures + the integration harness

**Files:**
- Create: `crates/edi835_core/tests/fixtures/` (copied from the POC)
- Create: `crates/edi835_core/tests/common/mod.rs`
- Create: `crates/edi835_core/tests/fixtures_harness.rs`

- [ ] **Step 1: Copy the real fixtures from the POC**

Run:
```bash
mkdir -p crates/edi835_core/tests/fixtures
cp /home/javillegasna/Desktop/org/personal/fast_edi835/fast_edi835_core/tests/fixtures/blue_cross_nc_sample.txt \
   /home/javillegasna/Desktop/org/personal/fast_edi835/fast_edi835_core/tests/fixtures/united_healthcare_legacy_sample.txt \
   /home/javillegasna/Desktop/org/personal/fast_edi835/fast_edi835_core/tests/fixtures/trizetto_sample.rmt \
   /home/javillegasna/Desktop/org/personal/fast_edi835/fast_edi835_core/tests/fixtures/emedny_sample.txt \
   /home/javillegasna/Desktop/org/personal/fast_edi835/fast_edi835_core/tests/fixtures/multi_claim_sample.txt \
   crates/edi835_core/tests/fixtures/
```
Expected: 5 files now present in `crates/edi835_core/tests/fixtures/`.

- [ ] **Step 2: Verify the copy**

Run:
```bash
ls crates/edi835_core/tests/fixtures/
```
Expected: lists `blue_cross_nc_sample.txt`, `emedny_sample.txt`, `multi_claim_sample.txt`, `trizetto_sample.rmt`, `united_healthcare_legacy_sample.txt`.

- [ ] **Step 3: Write the shared fixture helper**

Create `crates/edi835_core/tests/common/mod.rs`:
```rust
//! Shared helpers for integration tests. Lives in `tests/common/mod.rs` so Cargo
//! treats it as a module (not its own test binary) when included via `mod common;`.

use std::path::PathBuf;

/// Absolute path to this crate's `tests/fixtures` directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Read a fixture file's contents by file name. Panics with a clear message if missing.
pub fn load_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}
```

- [ ] **Step 4: Write the failing integration test**

Create `crates/edi835_core/tests/fixtures_harness.rs`:
```rust
//! Integration test that proves the fixture-loading pipeline works — the plumbing
//! the cross-layer (N7) tests will reuse. No parsing yet: Stage 0 only verifies the
//! fixtures are present and readable.

mod common;

#[test]
fn all_fixtures_load_and_are_nonempty() {
    let fixtures = [
        "blue_cross_nc_sample.txt",
        "united_healthcare_legacy_sample.txt",
        "trizetto_sample.rmt",
        "emedny_sample.txt",
        "multi_claim_sample.txt",
    ];
    for name in fixtures {
        let content = common::load_fixture(name);
        assert!(!content.trim().is_empty(), "fixture {name} is empty");
    }
}
```

- [ ] **Step 5: Run the integration test**

Run:
```bash
cargo test --workspace --test fixtures_harness
```
Expected: PASS — `test all_fixtures_load_and_are_nonempty ... ok`.

- [ ] **Step 6: Run the full suite to confirm nothing regressed**

Run:
```bash
cargo test --workspace
```
Expected: all tests pass (smoke + proptest + fixtures harness).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "test: add fixtures and integration harness proving the fixture pipeline"
```

---

## Task 4: CI pipeline + quality gates

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Verify formatting is clean locally**

Run:
```bash
cargo fmt --all
cargo fmt --all -- --check
```
Expected: second command exits 0 with no output (everything formatted).

- [ ] **Step 2: Verify clippy is clean with warnings-as-errors**

Run:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: `Finished` with no warnings/errors. If anything fires, fix it before continuing.

- [ ] **Step 3: Create the CI workflow**

Create `.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:

jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - name: Format check
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build
        run: cargo build --workspace --all-targets
      - name: Test
        run: cargo test --workspace
      - name: Benches compile
        run: cargo bench --workspace --no-run
```

- [ ] **Step 4: Final local gate — full verification sweep**

Run:
```bash
cargo build --workspace --all-targets && \
cargo test --workspace && \
cargo clippy --workspace --all-targets -- -D warnings && \
cargo fmt --all -- --check && \
cargo bench --workspace --no-run
```
Expected: every command exits 0. This mirrors what CI will enforce on push.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "ci: add build/test/clippy/fmt gates with dtolnay/rust-toolchain"
```

---

## Stage 0 exit gate (definition of done)

All of these must hold before Stage 1:

- [ ] `cargo test --workspace` passes (smoke + proptest + fixtures harness).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --all -- --check` is clean.
- [ ] `cargo bench --workspace --no-run` compiles; `cargo bench` runs.
- [ ] Workspace contains exactly one crate, `edi835_core` (no empty `python`/`cli` crates).
- [ ] Fixtures present and loaded by an integration test (N7 plumbing ready).
- [ ] CI workflow committed (runs green once the repo is pushed to GitHub).

> **Note on CI:** the workflow runs on GitHub. Until a remote exists and the project is
> pushed, Step 4's local sweep is the authoritative gate. Pushing to GitHub (creating the
> remote) is optional for Stage 0 and can happen here or at the start of Stage 1.
