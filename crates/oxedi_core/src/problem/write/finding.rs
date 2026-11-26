//! The text of where a value came from, of the findings about written tables
//! and of the errors that stop a write.

use std::fmt;

use crate::delimiters::{HeaderPositions, ISA_ID};
use crate::message::{Counted, Quoted};
use crate::write::{Finding, ValueOrigin, WriteError};

use super::wording::{Numbered, ReadBackOrigin, Reference};

impl fmt::Display for ValueOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueOrigin::Cell { table, row, column } => {
                write!(f, "table {table:?} row {row} column {column:?}")
            }
            ValueOrigin::Row { table, row } => write!(f, "table {table:?} row {row}"),
            ValueOrigin::Envelope { field } => write!(f, "envelope field {:?}", field.name()),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::DelimiterInValue {
                origin,
                place,
                value,
                delimiter,
                role,
            } => write!(
                f,
                "{origin} writes {place} with {}, which holds the {role} {}; a value cannot \
                 hold the interchange's delimiters",
                Quoted(value),
                Quoted(&[*delimiter])
            ),
            Finding::MissingParent {
                table,
                row,
                column,
                value,
                parent,
            } => write!(
                f,
                "table {table:?} row {row} column {column:?} {}, so the row has no place in the \
                 file",
                Reference(*value, parent)
            ),
            Finding::MismatchedReference {
                table,
                row,
                column,
                value,
                through,
                parent,
                parent_row,
                expected,
            } => write!(
                f,
                "table {table:?} row {row} column {column:?} is {value}, but the row it belongs \
                 to by column {through:?}, row {parent_row} of table {parent:?}, has {column:?} \
                 {expected}"
            ),
            Finding::DuplicateRowNumber {
                table,
                row,
                value,
                first,
            } => write!(
                f,
                "table {table:?} row {row} carries row number {value}, which row {first} \
                 already carries; references to it are ambiguous"
            ),
            Finding::OutOfOrder {
                table,
                row,
                column,
                value,
                previous,
            } => write!(
                f,
                "table {table:?} row {row} (column {column:?} = {value}) is written after row \
                 {previous}; the rows of one parent must be together and in their parents' order"
            ),
            Finding::NotWritable {
                origin,
                place,
                value,
                reason,
            } => write!(
                f,
                "{origin} holds {value}, which {place} cannot hold: {reason}"
            ),
            Finding::ReadBack { origin, diagnostic } => {
                write!(f, "{}: {diagnostic}", ReadBackOrigin(origin.as_ref()))
            }
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Plan(error) => write!(f, "{error}"),
            WriteError::UnknownTable(error) => write!(f, "{error}"),
            WriteError::UnknownColumn(error) => write!(f, "{error}"),
            WriteError::ColumnType {
                table,
                column,
                expected,
                found,
            } => write!(
                f,
                "table {table:?} column {column:?} is {found}; the spec makes it {expected}"
            ),
            WriteError::SameDelimiter {
                first,
                second,
                byte,
            } => write!(
                f,
                "the {first} and the {second} are both {}; each delimiter needs its own byte",
                Quoted(&[*byte])
            ),
            WriteError::DelimiterNotAllowed { role, byte } => write!(
                f,
                "the {role} {} is a letter, a digit or white space, which values hold",
                Quoted(&[*byte])
            ),
            WriteError::NoRepetition { version } => write!(
                f,
                "the interchange version {} ({ISA_ID}{}) carries a repetition separator in {ISA_ID}{}, but \
                 the envelope field \"delimiters.repetition\" is not set; set it to the separator \
                 to write",
                Quoted(version),
                HeaderPositions::ISA.version,
                HeaderPositions::ISA.repetition
            ),
            WriteError::Unreadable(error) => {
                write!(f, "the written file could not be read back: {error}")
            }
            WriteError::Findings(findings) => write!(
                f,
                "the tables do not make a valid file ({}); nothing was written{}",
                Counted(findings.len(), "finding", "findings"),
                Numbered(findings)
            ),
        }
    }
}
