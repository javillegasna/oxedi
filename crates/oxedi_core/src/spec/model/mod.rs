//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `loops` holds the loops
//! and their occurrences, `tables` the tables, and `rules` the version
//! declaration and the operations of a summary.

mod loops;
mod rules;
mod tables;

pub use loops::QualifierModel;
pub use rules::{
    SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel, SummaryTableModel,
    VersionModel,
};
pub use tables::RepeatModel;
