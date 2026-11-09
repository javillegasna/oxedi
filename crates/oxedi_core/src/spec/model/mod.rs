//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `rules` holds the
//! operations of a summary.

mod rules;

pub use rules::{
    SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel, SummaryTableModel,
};
