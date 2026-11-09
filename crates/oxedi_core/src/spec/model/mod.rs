//! The spec as typed data, read from and written to its JSON: the parts a
//! program can build, hold and serialize without going through a JSON string.
//! `rules` holds the operations of a summary.

mod rules;

pub use rules::{
    SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel, SummaryTableModel,
};
