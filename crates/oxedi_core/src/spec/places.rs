//! Where each declared column reads, compiled once with the spec.

use super::Spec;
use crate::value::ColumnType;

/// What of a segment a column reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// 1-based element, and 1-based component when it reads one.
    Element {
        element: usize,
        component: Option<usize>,
    },
    /// The element at 0-based `offset` in each group of the table's repeat,
    /// and its 1-based component when it reads one.
    Group {
        offset: usize,
        component: Option<usize>,
    },
    /// The 0-based index of the segment in the stream.
    Index,
}

/// Where one declared column reads, compiled once with the spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnPlace {
    /// The type of the values the column holds.
    pub(crate) kind: ColumnType,
    /// What of a segment the column reads.
    pub(crate) target: Target,
}

impl Spec {
    /// The places of the declared columns of table `table` (index into
    /// `tables()`), in column order; empty for an index that is no table.
    pub(crate) fn places(&self, table: usize) -> &[ColumnPlace] {
        self.places.get(table).map_or(&[], Vec::as_slice)
    }
}
