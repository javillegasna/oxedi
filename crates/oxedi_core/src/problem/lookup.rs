//! The text of a table, column, mask or summary that cannot be found or built.

use std::fmt;

use super::text::render_names;
use crate::mask::{Mask, MaskError};
use crate::spec::{BatchLoopError, UnknownColumn, UnknownTable};
use crate::summary::SummaryError;
use crate::vocabulary::{DateMode, words_text};

impl fmt::Display for UnknownTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "table {:?} is not a table of the spec; its tables are {}",
            self.name,
            render_names(&self.tables)
        )
    }
}

impl fmt::Display for UnknownColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "table {:?} has no column {:?} in the spec; its columns are {}",
            self.table,
            self.column,
            render_names(&self.columns)
        )
    }
}

impl fmt::Display for BatchLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchLoopError::Unknown { by, loops } => write!(
                f,
                "by {by:?}: the spec has no such loop; its loops are {}",
                loops.join(", ")
            ),
            BatchLoopError::NoEnvelope => f.write_str(
                "the spec has no envelope loop (a loop whose end segment checks it) to cut \
                 batches at; name a loop",
            ),
        }
    }
}

impl fmt::Display for MaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaskError::ShortKey { length } => write!(
                f,
                "the mask key must be at least {} bytes long; got {length}",
                Mask::MIN_KEY
            ),
            MaskError::UnknownDateMode { given } => write!(
                f,
                "the mask date mode must be {}; got {given:?}",
                words_text::<DateMode>()
            ),
        }
    }
}

impl fmt::Display for SummaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SummaryError::Undeclared { summary, declared } => write!(
                f,
                "the spec declares no summary {summary:?}; it declares{}",
                Names(declared)
            ),
            SummaryError::MissingTable {
                summary,
                table,
                present,
            } => write!(
                f,
                "summary {summary:?} reads the table {table:?}, which the tables do not hold; \
                 they hold{}",
                Names(present)
            ),
            SummaryError::MissingColumn {
                summary,
                table,
                column,
            } => write!(
                f,
                "summary {summary:?} reads the column {column:?} of the table {table:?}, which \
                 the table does not hold"
            ),
            SummaryError::NotDecimal {
                summary,
                table,
                column,
                found,
            } => write!(
                f,
                "summary {summary:?} sums the column {column:?} of the table {table:?}, which \
                 holds {found}, not a decimal"
            ),
            SummaryError::NotOneRow {
                summary,
                table,
                rows,
            } => write!(
                f,
                "summary {summary:?} reads the one row of the table {table:?}, which has {rows} \
                 rows"
            ),
            SummaryError::Overflow {
                summary,
                table,
                column,
                row,
                amount,
            } => write!(
                f,
                "summary {summary:?}: the sum of {column:?} in the table {table:?} overflows a \
                 128-bit decimal at row {row} (amount {amount})"
            ),
        }
    }
}

/// Names as a message lists them after a verb: `: a, b`, or ` none`.
struct Names<'a>(&'a [String]);

impl fmt::Display for Names<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [] => write!(f, " none"),
            names => write!(f, ": {}", names.join(", ")),
        }
    }
}
