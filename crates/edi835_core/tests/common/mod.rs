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
