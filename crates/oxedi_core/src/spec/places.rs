//! Where each declared column reads, compiled once with the spec.

use super::{ColumnSource, Spec, TableDef};
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

    /// The segment id, element and component of every position `source`, a
    /// column of `table`, can read. A column of a table anchored on a
    /// segment reads that segment too, and a group column reads its offset
    /// in every group of the defined elements the segment can hold.
    pub(crate) fn positions<'t>(
        &'t self,
        table: &'t TableDef,
        source: &'t ColumnSource,
    ) -> Vec<(&'t [u8], usize, Option<usize>)> {
        let mut positions = Vec::new();
        match source {
            ColumnSource::Element {
                segment,
                element,
                component,
                ..
            } => {
                positions.push((segment.as_slice(), *element, *component));
                if let Some(anchor) = &table.segment {
                    positions.push((anchor.as_slice(), *element, *component));
                }
            }
            ColumnSource::GroupElement { offset, component } => {
                let (Some(anchor), Some(repeat)) = (&table.segment, table.repeat) else {
                    return positions;
                };
                let defined = self
                    .segment(anchor)
                    .into_iter()
                    .flat_map(|def| def.elements.keys().copied());
                positions.extend(
                    defined
                        .filter(|&position| repeat.holds(*offset, position))
                        .map(|position| (anchor.as_slice(), position, *component)),
                );
            }
            ColumnSource::SegmentIndex { .. } => {}
        }
        positions
    }
}
