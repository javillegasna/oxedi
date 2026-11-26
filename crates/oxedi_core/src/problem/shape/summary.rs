//! The shape of a summary error.

use super::{Place, Problem, SpecPlace};
use crate::summary::SummaryError;

problem!(SummaryError [Undeclared, MissingTable, MissingColumn, NotDecimal, NotOneRow, Overflow] this => {
        match this {
            SummaryError::Undeclared { summary, .. } => {
                Place::Spec(SpecPlace::Summary { name: summary })
            }
            SummaryError::MissingTable { table, .. } | SummaryError::NotOneRow { table, .. } => {
                Place::Cell {
                    table,
                    row: None,
                    column: None,
                }
            }
            SummaryError::MissingColumn { table, column, .. }
            | SummaryError::NotDecimal { table, column, .. } => Place::Cell {
                table,
                row: None,
                column: Some(column),
            },
            SummaryError::Overflow {
                table, column, row, ..
            } => Place::Cell {
                table,
                row: Some(*row),
                column: Some(column),
            },
        }
});
