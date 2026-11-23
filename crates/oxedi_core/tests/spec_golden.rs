//! The compiled form of each built-in spec and of the `edi_835_parser.json`
//! patch, compared against committed golden files in `tests/golden/spec/`:
//! version, default table, roots, loops, segments, tables, balancing rules
//! and summaries, one `Debug` line each. The source JSON is deliberately not
//! printed, so the files pin what the spec compiles to, whatever loader built
//! it; the specs are compiled from their models. Regenerate with `UPDATE_GOLDEN=1 cargo test -p oxedi_core --test
//! spec_golden`, inspect the diff, commit.

mod common;

use std::fmt::Write as _;
use std::path::PathBuf;

use oxedi_core::{Builtin, Spec, Version};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/spec")
}

/// Every compiled definition of `spec`, in the order its accessors document.
fn compiled(spec: &Spec) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "name: {:?}", spec.name());
    let _ = writeln!(out, "version: {:?}", spec.version());
    let _ = writeln!(out, "default_table: {:?}", spec.default_table());
    let _ = writeln!(out, "roots: {:?}", spec.roots());
    for (i, def) in spec.loops().iter().enumerate() {
        let _ = writeln!(out, "loop {i}: {def:?}");
    }
    for (id, def) in spec.segments() {
        let _ = writeln!(out, "segment {:?}: {def:?}", String::from_utf8_lossy(id));
    }
    for (i, def) in spec.tables().iter().enumerate() {
        let _ = writeln!(out, "table {i}: {def:?}");
    }
    for (i, rule) in spec.balancing().iter().enumerate() {
        let _ = writeln!(out, "balance {i}: {rule:?}");
    }
    for (name, summary) in spec.summaries() {
        let _ = writeln!(out, "summary {name:?}: {summary:?}");
    }
    out
}

#[test]
fn compiled_specs_match_the_golden_files() {
    let built = |version| &Builtin::find("835", version).unwrap().spec;
    let (five, four) = (built(Version::V5010), built(Version::V4010));
    let parser = common::compat();
    let outputs: Vec<(PathBuf, String)> = [
        ("5010.txt", five),
        ("4010.txt", four),
        ("edi_835_parser.txt", &parser),
    ]
    .into_iter()
    .map(|(file, spec)| (golden_dir().join(file), compiled(spec)))
    .collect();
    let failures = common::compare_goldens(&golden_dir(), &outputs, &[]);
    assert!(failures.is_empty(), "{failures:#?}");
}
