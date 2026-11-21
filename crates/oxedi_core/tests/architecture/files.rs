//! Finds the files of the core and reads every use in them.

use super::scan::{Use, uses_in};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The entries of a folder, with the folder named when it cannot be read.
fn entries(dir: &Path) -> Vec<PathBuf> {
    let read = fs::read_dir(dir).unwrap_or_else(|e| {
        panic!(
            "cannot read the folder {}: {e} · check its permissions",
            dir.display()
        )
    });
    let listed = read.map(|entry| {
        entry
            .unwrap_or_else(|e| {
                panic!(
                    "cannot list an entry of {}: {e} · check its permissions",
                    dir.display()
                )
            })
            .path()
    });
    listed.collect()
}

/// Whether the file holds tests or helpers for tests, which are not part of the module's
/// dependencies.
pub fn is_test_file(rel: &Path) -> bool {
    let in_tests_dir = rel.components().any(|c| c.as_os_str() == "tests");
    let named = rel
        .file_name()
        .is_some_and(|n| n == "tests.rs" || n == "test_support.rs");
    in_tests_dir || named
}

/// The folders directly under `src`: the modules of the core.
pub fn modules_of(src: &Path) -> BTreeSet<String> {
    entries(src)
        .iter()
        .filter(|p| p.is_dir())
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .collect()
}

/// Every use of a module by another in the non-test files under `src`, but `lib.rs`.
pub fn core_uses(src: &Path) -> Vec<Use> {
    let modules = modules_of(src);
    let mut files = Vec::new();
    collect_rs(src, src, &mut files);
    files.sort();
    let mut found = Vec::new();
    for rel in files
        .iter()
        .filter(|r| *r != Path::new("lib.rs") && !is_test_file(r))
    {
        let path = src.join(rel);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "cannot read the source file {}: {e} · it must be UTF-8 and readable",
                path.display()
            )
        });
        found.extend(uses_in(rel, &text, &modules));
    }
    found
}

fn collect_rs(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    for path in entries(dir) {
        if path.is_dir() {
            collect_rs(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.extend(path.strip_prefix(root).map(Path::to_path_buf));
        }
    }
}

#[test]
fn only_files_of_tests_are_test_files() {
    let files = [
        ("spec/tests.rs", true),
        ("spec/tests/display.rs", true),
        ("write/tests/plan.rs", true),
        ("delimiters/test_support.rs", true),
        ("spec/tables.rs", false),
        ("write/plan.rs", false),
    ];
    for (rel, expected) in files {
        assert_eq!(is_test_file(Path::new(rel)), expected, "{rel}");
    }
}
