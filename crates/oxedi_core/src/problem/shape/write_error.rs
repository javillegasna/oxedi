//! The shape of a write error.

use super::{Place, Problem};
use crate::write::WriteError;

problem!(WriteError [Plan, UnknownTable, UnknownColumn, ColumnType, SameDelimiter, DelimiterNotAllowed, NoRepetition, Unreadable, Findings] this => {
        match this {
            WriteError::UnknownTable(unknown) => Place::Cell {
                table: &unknown.name,
                row: None,
                column: None,
            },
            WriteError::UnknownColumn(unknown) => Place::Cell {
                table: &unknown.table,
                row: None,
                column: Some(&unknown.column),
            },
            WriteError::ColumnType { table, column, .. } => Place::Cell {
                table,
                row: None,
                column: Some(column),
            },
            WriteError::Unreadable(error) => error.place(),
            WriteError::Plan(_)
            | WriteError::SameDelimiter { .. }
            | WriteError::DelimiterNotAllowed { .. }
            | WriteError::NoRepetition { .. }
            | WriteError::Findings(_) => Place::Nowhere,
        }
});
