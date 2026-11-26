//! The shape of a write finding.

use super::{Detail, Place, Problem};
use crate::write::{Finding, ValueOrigin};

impl Problem for Finding {
    fn kind(&self) -> &'static str {
        match self {
            Finding::DelimiterInValue { .. } => "DelimiterInValue",
            Finding::MissingParent { .. } => "MissingParent",
            Finding::MismatchedReference { .. } => "MismatchedReference",
            Finding::DuplicateRowNumber { .. } => "DuplicateRowNumber",
            Finding::OutOfOrder { .. } => "OutOfOrder",
            Finding::NotWritable { .. } => "NotWritable",
            Finding::ReadBack { .. } => "ReadBack",
        }
    }

    fn place(&self) -> Place<'_> {
        match self {
            Finding::MissingParent {
                table, row, column, ..
            }
            | Finding::MismatchedReference {
                table, row, column, ..
            }
            | Finding::OutOfOrder {
                table, row, column, ..
            } => Place::Cell {
                table,
                row: Some(*row),
                column: Some(column),
            },
            Finding::DuplicateRowNumber { table, row, .. } => Place::Cell {
                table,
                row: Some(*row),
                column: None,
            },
            Finding::DelimiterInValue { origin, .. } | Finding::NotWritable { origin, .. } => {
                origin_place(origin)
            }
            Finding::ReadBack {
                origin: Some(origin),
                ..
            } => origin_place(origin),
            Finding::ReadBack {
                origin: None,
                diagnostic,
            } => diagnostic.place(),
        }
    }

    fn detail(&self) -> Detail<'_> {
        let (origin, diagnostic) = match self {
            Finding::DelimiterInValue { origin, .. } | Finding::NotWritable { origin, .. } => {
                (Some(origin), None)
            }
            Finding::ReadBack {
                origin, diagnostic, ..
            } => (origin.as_ref(), Some(&**diagnostic)),
            _ => (None, None),
        };
        Detail::Finding { origin, diagnostic }
    }
}

/// The place of the value or row a finding is about.
fn origin_place(origin: &ValueOrigin) -> Place<'_> {
    match origin {
        ValueOrigin::Cell { table, row, column } => Place::Cell {
            table,
            row: Some(*row),
            column: Some(column),
        },
        ValueOrigin::Row { table, row } => Place::Cell {
            table,
            row: Some(*row),
            column: None,
        },
        ValueOrigin::Envelope { field } => Place::Envelope { field: *field },
    }
}
