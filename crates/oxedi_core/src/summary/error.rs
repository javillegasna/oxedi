//! Why a summary cannot be computed over a set of tables.

use std::fmt;

use crate::value::ColumnType;

/// Why [`summarize`](super::summarize) cannot compute a summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryError {
    /// The spec declares no summary of that name.
    Undeclared {
        /// The name asked for.
        summary: String,
        /// The summaries the spec declares, in name order.
        declared: Vec<String>,
    },
    /// The tables do not hold a table the summary reads.
    MissingTable {
        /// The summary.
        summary: String,
        /// The table.
        table: String,
        /// The names of the tables that are present, in their order.
        present: Vec<String>,
    },
    /// The table does not hold a column the summary reads.
    MissingColumn {
        /// The summary.
        summary: String,
        /// The table.
        table: String,
        /// The column.
        column: String,
    },
    /// The column a `sum` reads holds something other than decimals.
    NotDecimal {
        /// The summary.
        summary: String,
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// What the column holds.
        found: ColumnType,
    },
    /// The table a `row` summary reads has other than one row.
    NotOneRow {
        /// The summary.
        summary: String,
        /// The table.
        table: String,
        /// How many rows it has.
        rows: usize,
    },
    /// Adding a row's amount to the sum overflows a 128-bit decimal.
    Overflow {
        /// The summary.
        summary: String,
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// 0-based row whose amount overflows the sum.
        row: usize,
        /// That amount, as decimal text at the column's scale.
        amount: String,
    },
}

impl std::error::Error for SummaryError {}
