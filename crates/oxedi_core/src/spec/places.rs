//! Where each declared column reads, compiled once with the spec.

use super::{ColumnSource, LoopDef, LoopId, OccurrenceId, Spec, TableDef, holds_code};
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
    /// Which loops and occurrences the column reads.
    pub(crate) reading: Reading,
}

/// Which loops and occurrences a column reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reading {
    /// A column of a table anchored on a segment: the anchor segment, as each
    /// row opens.
    Anchor,
    /// A column of a loop table: its relation to the anchor and one read per
    /// reading loop (each anchor loop in the table's order for
    /// `Relation::Anchor`, else the one loop it names).
    Watch {
        relation: Relation,
        reads: Vec<LoopRead>,
    },
}

impl Reading {
    /// The carried slot and the reads of a column of a loop above its
    /// table's anchor.
    pub(crate) fn above(&self) -> Option<(usize, &[LoopRead])> {
        match self {
            Reading::Watch {
                relation: Relation::Above { carried },
                reads,
            } => Some((*carried, reads)),
            Reading::Watch { .. } | Reading::Anchor => None,
        }
    }
}

/// Where the loop a column reads sits relative to its table's anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relation {
    /// The column names no loop: it reads the anchor loops.
    Anchor,
    /// The column names a loop at or below the anchor.
    Inside,
    /// The column names a loop above the anchor: its value is carried until a
    /// row opens.
    Above {
        /// 0-based slot among the table's carried values, in column order.
        carried: usize,
    },
}

/// What a column reads in one loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopRead {
    /// The loop whose captured segments the column reads.
    pub(crate) loop_id: LoopId,
    /// The occurrence of the segment it reads there.
    pub(crate) place: OccurrencePlace,
}

/// The occurrence a column reads in a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OccurrencePlace {
    /// The occurrence the column names.
    Named(OccurrenceId),
    /// The one occurrence the column's segment and conditions choose.
    Chosen(OccurrenceId),
    /// No one occurrence: every segment of the id that holds the conditions.
    Unplaced(Unplaced),
}

impl OccurrencePlace {
    /// Whether a segment that took `matched` in its loop is one this place
    /// reads; the column's conditions are tested apart.
    pub(crate) fn admits(self, matched: Option<OccurrenceId>) -> bool {
        match self {
            OccurrencePlace::Named(occurrence) | OccurrencePlace::Chosen(occurrence) => {
                matched == Some(occurrence)
            }
            OccurrencePlace::Unplaced(_) => true,
        }
    }
}

/// Why no one occurrence of a loop is the column's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unplaced {
    /// The loop declares no occurrence of the segment.
    NoOccurrence,
    /// The occurrences are told apart at a place the conditions do not fix
    /// (another element, or a component).
    QualifierFree,
    /// The conditions fix the qualifier's element to a code no occurrence
    /// lists.
    NoOccurrenceTakes,
}

impl LoopDef {
    /// The occurrence named `name`, if the loop declares it.
    pub(crate) fn occurrence_named(&self, name: &str) -> Option<OccurrenceId> {
        self.occurrences
            .iter()
            .position(|occurrence| occurrence.name == name)
            .map(OccurrenceId::at)
    }

    /// The place of a column reading `segment` with `conditions` in this
    /// loop. Qualifiers of one segment share no code and sit at one place,
    /// so a `Chosen` occurrence is the one a segment holding the conditions
    /// takes.
    pub(crate) fn place_of(
        &self,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
    ) -> OccurrencePlace {
        let mut taking = self
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| occurrence.segment == segment);
        let Some(first) = taking.next() else {
            return OccurrencePlace::Unplaced(Unplaced::NoOccurrence);
        };
        let Some(qualifier) = &first.1.qualifier else {
            return OccurrencePlace::Chosen(OccurrenceId::at(first.0));
        };
        let fixed = conditions
            .iter()
            .find(|(element, _)| *element == qualifier.element);
        let (None, Some((_, value))) = (qualifier.component, fixed) else {
            return OccurrencePlace::Unplaced(Unplaced::QualifierFree);
        };
        std::iter::once(first)
            .chain(taking)
            .find(|(_, occurrence)| {
                occurrence
                    .qualifier
                    .as_ref()
                    .is_some_and(|qualifier| holds_code(&qualifier.codes, value))
            })
            .map_or(
                OccurrencePlace::Unplaced(Unplaced::NoOccurrenceTakes),
                |(at, _)| OccurrencePlace::Chosen(OccurrenceId::at(at)),
            )
    }
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
