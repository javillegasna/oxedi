//! The text of a run's output, as a kit's expected files hold it.

use std::fmt::Write as _;

use crate::column::Tables;
use crate::diagnostic::Diagnostic;
use crate::spec_load::KitOutput;

/// The text of one output of a run: the tables as their `Display` writes them,
/// or the diagnostics, one `Display` line each. The runner compares an expected
/// file with it, and a tool that writes an expected file writes it.
pub fn render(output: KitOutput, tables: &Tables, diagnostics: &[Diagnostic]) -> String {
    match output {
        KitOutput::Tables => tables.to_string(),
        KitOutput::Diagnostics => diagnostics.iter().fold(String::new(), |mut out, d| {
            let _ = writeln!(out, "{d}");
            out
        }),
    }
}
