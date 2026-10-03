//! Tables and diagnostics compared against committed golden files in
//! `tests/golden/project/`. Small files keep every row of every table; the
//! two large samples keep a row count per table. Every file keeps its whole
//! diagnostic list, one `Display` line each. Regenerate with
//! `UPDATE_GOLDEN=1 cargo test --test project_golden`, inspect the diff,
//! commit.

mod common;

use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Diagnostic, Document, Processor, Spec, Tables};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/project")
}

/// Every table: a title with its row count, the header, one line per row.
fn all_rows(tables: &Tables) -> String {
    let mut out = String::new();
    for table in tables {
        let _ = writeln!(out, "## {} (rows: {})", table.name(), table.len());
        let _ = writeln!(out, "{}", common::table_header(table));
        for row in common::table_rows(table) {
            let _ = writeln!(out, "{row}");
        }
        out.push('\n');
    }
    out
}

/// One line per table with its row count.
fn row_counts(tables: &Tables) -> String {
    tables.iter().fold(String::new(), |mut out, table| {
        let _ = writeln!(out, "{} rows: {}", table.name(), table.len());
        out
    })
}

fn diagnostic_lines(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .fold(String::new(), |mut out, diagnostic| {
            let _ = writeln!(out, "{diagnostic}");
            out
        })
}

#[test]
fn tables_and_diagnostics_match_the_golden_files() {
    let spec = Spec::builtin_835();
    let mut outputs = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (tables, diagnostics) = Processor::run(&spec, &document);
        outputs.push(if common::SUMMARY_ONLY.contains(&name.as_str()) {
            (
                golden_dir().join(format!("{name}.tables.summary.txt")),
                row_counts(&tables),
            )
        } else {
            (
                golden_dir().join(format!("{name}.tables.txt")),
                all_rows(&tables),
            )
        });
        outputs.push((
            golden_dir().join(format!("{name}.diagnostics.txt")),
            diagnostic_lines(&diagnostics),
        ));
    }
    let failures = common::compare_goldens(&golden_dir(), &outputs);
    assert!(failures.is_empty(), "{failures:#?}");
}
