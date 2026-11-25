//! The element checks the projector builds per run from the spec's segment
//! definitions, with the read marks of the spec's index.

use super::carry::Carried;
use crate::spec::{ColumnSource, ElementDef, IdMap, OccurrencePlace, Reading, Relation, Spec};
use crate::value::ColumnType;
use crate::vocabulary::Pick;

/// One value the check visits: a simple element, or one declared component.
#[derive(Debug, Clone)]
pub(super) struct LeafCheck<'s> {
    /// 1-based component position; `None` for a simple element.
    pub(super) component: Option<usize>,
    pub(super) def: &'s ElementDef,
    /// The column type the definition maps to.
    pub(super) kind: ColumnType,
    /// Whether a column reads it. A value no column reads is validated
    /// without keeping it.
    pub(super) read: bool,
}

impl<'s> LeafCheck<'s> {
    fn new(component: Option<usize>, def: &'s ElementDef, read: bool) -> Self {
        Self {
            component,
            def,
            kind: ColumnType::of(Some(def.kind)),
            read,
        }
    }
}

/// One defined element, with what checking it needs worked out once.
#[derive(Debug, Clone)]
pub(super) struct ElementCheck<'s> {
    pub(super) position: usize,
    pub(super) def: &'s ElementDef,
    /// Highest component position the definition declares; `None` for a
    /// simple element.
    pub(super) composite: Option<usize>,
    /// The element itself when it is simple, else each declared component,
    /// in position order.
    pub(super) leaves: Vec<LeafCheck<'s>>,
}

impl<'s> ElementCheck<'s> {
    /// The check of the element at `position` of segment `id`, whose leaves
    /// are marked read from the spec's index.
    pub(super) fn new(spec: &Spec, id: &[u8], position: usize, def: &'s ElementDef) -> Self {
        let read = |component| spec.reads().is_read(id, position, component);
        let leaves = if def.composite.is_empty() {
            vec![LeafCheck::new(None, def, read(None))]
        } else {
            def.composite
                .iter()
                .map(|(&at, part)| LeafCheck::new(Some(at), part, read(Some(at))))
                .collect()
        };
        Self {
            position,
            def,
            composite: def.composite.keys().next_back().copied(),
            leaves,
        }
    }
}

/// What the projector does with one segment id, worked out once.
#[derive(Debug, Clone)]
pub(super) struct SegmentPlan<'s> {
    /// The defined elements to check, by position.
    pub(super) elements: Vec<ElementCheck<'s>>,
    /// Per loop: the columns that read this segment when it is captured in
    /// that loop.
    pub(super) watchers: Vec<Vec<Watcher>>,
}

/// A column that reads the segments of one id a loop captures.
#[derive(Debug, Clone, Copy)]
pub(super) struct Watcher {
    /// The table, as an index into the spec's tables.
    pub(super) table: usize,
    /// The column, as an index into the table's declared columns.
    pub(super) column: usize,
    /// For a column that names an occurrence, its index in the capturing
    /// loop's occurrences: the segment must match it, and the column's
    /// conditions are not read.
    pub(super) occurrence: Option<usize>,
    /// Which matching segment gives the value.
    pub(super) pick: Pick,
    /// For a column that reads a loop above the table's anchor, its index
    /// in the table's carried values: the value is kept there until a row
    /// opens, instead of going to the open row.
    pub(super) carried: Option<usize>,
}

/// The plans of every segment id.
#[derive(Debug, Clone, Default)]
pub(super) struct Plans<'s> {
    by_id: IdMap<SegmentPlan<'s>>,
}

impl<'s> Plans<'s> {
    pub(super) fn get(&self, id: &[u8]) -> Option<&SegmentPlan<'s>> {
        self.by_id.get(id)
    }

    /// Makes every column of table `index` (one without `segment`) watch the
    /// segment id it reads in each loop the spec placed it in. A column that
    /// reads a loop above the anchor gets a carried value, which an instance
    /// of that loop opening starts over; the carried values are returned in
    /// column order.
    pub(super) fn watch_places(&mut self, spec: &'s Spec, index: usize) -> Vec<Carried> {
        let loops = spec.loops().len();
        let mut carried = Vec::new();
        let columns = &spec.tables()[index].columns;
        for (column, (place, (_, source))) in spec.places(index).iter().zip(columns).enumerate() {
            let (Reading::Watch { relation, reads }, Some((segment, pick))) =
                (&place.reading, segment_pick(source))
            else {
                continue;
            };
            let slot = match relation {
                Relation::Above { carried: slot } => {
                    carried.push(Carried::new(column));
                    Some(*slot)
                }
                Relation::Anchor | Relation::Inside => None,
            };
            let plan = self.entry(segment, loops);
            for read in reads {
                if let Some(watchers) = plan.watchers.get_mut(read.loop_id.index()) {
                    watchers.push(Watcher {
                        table: index,
                        column,
                        occurrence: named_index(&read.place),
                        pick,
                        carried: slot,
                    });
                }
            }
        }
        carried
    }

    /// The plan of `id`, created empty (with room for `loops` watcher lists)
    /// when there is none.
    pub(super) fn entry(&mut self, id: &[u8], loops: usize) -> &mut SegmentPlan<'s> {
        let empty = || SegmentPlan {
            elements: Vec::new(),
            watchers: vec![Vec::new(); loops],
        };
        self.by_id.entry_or_insert_with(id, empty)
    }
}

/// The segment id and pick of a column that reads a segment of a loop.
fn segment_pick(source: &ColumnSource) -> Option<(&[u8], Pick)> {
    match source {
        ColumnSource::Element { segment, pick, .. }
        | ColumnSource::SegmentIndex { segment, pick, .. } => Some((segment.as_slice(), *pick)),
        ColumnSource::GroupElement { .. } => None,
    }
}

/// The index in its loop's occurrences of the occurrence a read names, which
/// a captured segment must match; `None` when the column reads by its
/// conditions.
fn named_index(place: &OccurrencePlace) -> Option<usize> {
    match place {
        OccurrencePlace::Named(occurrence) => Some(occurrence.index()),
        OccurrencePlace::Chosen(_) | OccurrencePlace::Unplaced(_) => None,
    }
}
