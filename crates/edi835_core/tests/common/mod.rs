#![allow(dead_code)] // each test binary compiles this module; not all use every helper
//! Shared helpers for integration tests. Lives in `tests/common/mod.rs` so Cargo
//! treats it as a module (not its own test binary) when included via `mod common;`.

use std::path::PathBuf;

/// Absolute path to this crate's `tests/fixtures` directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Read a fixture file's raw bytes by file name. Panics with a clear message if missing.
///
/// Returns bytes, not a `String`: the core consumes `&[u8]` and real payer files
/// are not guaranteed to be UTF-8. The fixture pipeline must never decode on the way in.
pub fn load_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}
