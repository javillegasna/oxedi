//! The only place that writes message text.
//!
//! The layers raise problems as data (the rule that failed, where, the
//! offending value); this module writes their `Display` and remedies, so a
//! wording changes in one folder and no layer holds prose.
//!
//! The folder holds: `shape/` (the shape every problem has: kind, place, detail
//! and a record), `diagnostic/` (the text of a diagnostic and its rules),
//! `write/` (the text of the writer's refusals, findings and errors),
//! `load/` (the text of the problems a spec, a package or a kit raises),
//! `syntax.rs` (the interchange header, a document, a segment), `value.rs`
//! (conversions, cells and rows), `lookup.rs` (tables, columns, masks and
//! summaries), and `validate.rs` and `conformance.rs` (what the tools answer).

mod conformance;
mod diagnostic;
mod load;
mod lookup;
mod shape;
mod spec;
mod syntax;
mod text;
#[cfg(test)]
mod tests;
mod validate;
mod value;
mod write;

pub use shape::{Detail, Place, Problem, ProblemRecord, SpecPlace};
