//! What keeps tables from becoming a valid file: the findings about their
//! data, where each value came from, and the errors that stop a write.

use crate::diagnostic::Diagnostic;
use crate::document::DocumentError;
use crate::spec::{UnknownColumn, UnknownTable};
use crate::value::{ColumnType, ElementTextError};

use super::refusal::PlanError;

/// Where a written value comes from. Rows are 0-based positions in the
/// table as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueOrigin {
    /// One cell of a table.
    Cell {
        /// The table.
        table: String,
        /// The row's position.
        row: usize,
        /// The column.
        column: String,
    },
    /// A row of a table, when no single column is at fault.
    Row {
        /// The table.
        table: String,
        /// The row's position.
        row: usize,
    },
    /// A field of the envelope.
    Envelope {
        /// The field, e.g. `sender_id`.
        field: String,
    },
}

/// One reason the tables do not make a valid file. Rows are 0-based
/// positions in the table as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A value holds one of the interchange's delimiters, so written as it
    /// is it would split its element, segment or repeat.
    DelimiterInValue {
        /// Where the value comes from.
        origin: ValueOrigin,
        /// The element or component it fills, e.g. `NM103` or `SVC01-2`.
        place: String,
        /// The value.
        value: Vec<u8>,
        /// The delimiter byte it holds.
        delimiter: u8,
        /// What the delimiter separates, e.g. `element separator`.
        role: &'static str,
    },
    /// A row's reference to the table above it is null or names no row of
    /// that table, so the row has no place in the file.
    MissingParent {
        /// The table.
        table: String,
        /// The row.
        row: usize,
        /// The reference column.
        column: String,
        /// The reference; `None` when null.
        value: Option<i64>,
        /// The table the reference points into.
        parent: String,
    },
    /// A row's reference to a table further up contradicts the row it
    /// belongs to: that row refers to another one.
    MismatchedReference {
        /// The table.
        table: String,
        /// The row.
        row: usize,
        /// The contradicting reference column.
        column: String,
        /// Its value.
        value: i64,
        /// The column that places the row.
        through: String,
        /// The table the row belongs to a row of.
        parent: String,
        /// That row.
        parent_row: usize,
        /// That row's reference in `column`.
        expected: i64,
    },
    /// Two rows of a table carry the same row number, so a reference to it
    /// cannot tell them apart.
    DuplicateRowNumber {
        /// The table.
        table: String,
        /// The second row.
        row: usize,
        /// The row number both carry.
        value: i64,
        /// The first row that carries it.
        first: usize,
    },
    /// A row's place in the file comes before a row its table lists ahead
    /// of it: the rows of one parent are not together, or not in their
    /// parents' order.
    OutOfOrder {
        /// The table.
        table: String,
        /// The row written late.
        row: usize,
        /// The reference column that places it.
        column: String,
        /// Its reference.
        value: i64,
        /// The row of the same table already written.
        previous: usize,
    },
    /// A value has no text its element can hold.
    NotWritable {
        /// Where the value comes from.
        origin: ValueOrigin,
        /// The element or component it fills.
        place: String,
        /// The value as text.
        value: String,
        /// Why, e.g. `a date needs a year from 1 to 9999`.
        reason: ElementTextError,
    },
    /// A diagnostic from reading the written file back, with where the
    /// value at fault came from when the writer knows it.
    ReadBack {
        /// Where the value or row at fault comes from.
        origin: Option<ValueOrigin>,
        /// The diagnostic about the written file.
        diagnostic: Box<Diagnostic>,
    },
}

/// Why tables could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The spec's tables cannot make a valid file.
    Plan(PlanError),
    /// A table the spec does not define.
    UnknownTable(UnknownTable),
    /// A column the spec's table does not have.
    UnknownColumn(UnknownColumn),
    /// A column whose type is not the one the spec gives it.
    ColumnType {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The spec's type.
        expected: ColumnType,
        /// The column's type.
        found: ColumnType,
    },
    /// Two delimiters are the same byte.
    SameDelimiter {
        /// The first delimiter's role, e.g. `element separator`.
        first: &'static str,
        /// The second delimiter's role.
        second: &'static str,
        /// The byte both use.
        byte: u8,
    },
    /// A delimiter is a letter, a digit or white space, which values hold.
    DelimiterNotAllowed {
        /// The delimiter's role.
        role: &'static str,
        /// The byte.
        byte: u8,
    },
    /// The interchange header carries a repetition separator, and the
    /// envelope's delimiters have none.
    NoRepetition {
        /// The interchange version (ISA12) the header writes.
        version: Vec<u8>,
    },
    /// The written file could not be read back.
    Unreadable(DocumentError),
    /// The tables do not make a valid file; nothing was written.
    Findings(Vec<Finding>),
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // `Plan` and `Unreadable` print their inner error in full, so the
        // chain stops here instead of printing it a second time.
        match self {
            WriteError::Plan(_)
            | WriteError::UnknownTable(_)
            | WriteError::UnknownColumn(_)
            | WriteError::ColumnType { .. }
            | WriteError::SameDelimiter { .. }
            | WriteError::DelimiterNotAllowed { .. }
            | WriteError::NoRepetition { .. }
            | WriteError::Unreadable(_)
            | WriteError::Findings(_) => None,
        }
    }
}

impl From<UnknownTable> for WriteError {
    fn from(error: UnknownTable) -> Self {
        WriteError::UnknownTable(error)
    }
}

impl From<UnknownColumn> for WriteError {
    fn from(error: UnknownColumn) -> Self {
        WriteError::UnknownColumn(error)
    }
}

impl From<PlanError> for WriteError {
    fn from(error: PlanError) -> Self {
        WriteError::Plan(error)
    }
}
