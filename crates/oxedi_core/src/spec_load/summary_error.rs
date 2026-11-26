//! Why a summary of the spec is rejected.

use std::fmt;

/// Why a summary is rejected. `key` is where inside the summary the fault
/// is, as written, e.g. `sum.column` or `row.fields[2].name` (0-based index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryDefError {
    /// The summary's name is the empty string.
    EmptyName,
    /// The summary holds no operation, or more than one. It is reported inside
    /// `SpecError::Format`, at the summary's path.
    Operations {
        /// The operations it holds, in alphabetical order.
        found: Vec<String>,
    },
    /// The summary names a table the spec does not define.
    UnknownTable {
        /// Where the table is named.
        key: String,
        /// The table as written.
        table: String,
        /// The tables the spec defines, in name order.
        known: Vec<String>,
    },
    /// The summary names a column its table does not declare.
    UnknownColumn {
        /// Where the column is named.
        key: String,
        /// The table.
        table: String,
        /// The column as written.
        column: String,
        /// The columns the table declares, in name order.
        known: Vec<String>,
    },
    /// A `sum` names a column that does not read a decimal element.
    NotDecimal {
        /// Where the column is named.
        key: String,
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// What the column reads, e.g. `AN (string)`.
        found: String,
    },
    /// A `row` lists no field.
    NoFields {
        /// Where the list is.
        key: String,
    },
    /// A `row` names one field twice.
    RepeatedField {
        /// Where the second one is named.
        key: String,
        /// The field name.
        name: String,
        /// Where the first one is named.
        first: String,
    },
}

impl std::error::Error for SummaryDefError {}
