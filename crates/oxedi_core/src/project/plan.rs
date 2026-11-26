//! The per-segment plans the projector builds once from the spec.

use super::carry::Carried;
use crate::spec::{ColumnSource, ElementDef, IdMap, Spec, TableDef};
use crate::value::ColumnType;
use crate::vocabulary::Pick;

/// One value the check visits: a simple element, or one declared component.
#[derive(Debug, Clone)]
pub(super) struct Leaf<'s> {
    /// 1-based component position; `None` for a simple element.
    pub(super) component: Option<usize>,
    pub(super) def: &'s ElementDef,
    /// The column type the definition maps to.
    pub(super) kind: ColumnType,
    /// Whether a column reads it. A value no column reads is validated
    /// without keeping it.
    pub(super) read: bool,
}

impl<'s> Leaf<'s> {
    fn new(component: Option<usize>, def: &'s ElementDef) -> Self {
        Self {
            component,
            def,
            kind: ColumnType::of(Some(def.kind)),
            read: false,
        }
    }
}

/// One defined element, with what checking it needs worked out once.
#[derive(Debug, Clone)]
pub(super) struct ElementPlan<'s> {
    pub(super) position: usize,
    pub(super) def: &'s ElementDef,
    /// Highest component position the definition declares; `None` for a
    /// simple element.
    pub(super) composite: Option<usize>,
    /// The element itself when it is simple, else each declared component,
    /// in position order.
    pub(super) leaves: Vec<Leaf<'s>>,
}

impl<'s> ElementPlan<'s> {
    pub(super) fn new(position: usize, def: &'s ElementDef) -> Self {
        let leaves = if def.composite.is_empty() {
            vec![Leaf::new(None, def)]
        } else {
            def.composite
                .iter()
                .map(|(&at, part)| Leaf::new(Some(at), part))
                .collect()
        };
        Self {
            position,
            def,
            composite: def.composite.keys().next_back().copied(),
            leaves,
        }
    }

    /// Records that a column reads this element (`component` `None`) or one
    /// of its declared components. A composite read whole marks nothing.
    fn mark_read(&mut self, component: Option<usize>) {
        self.leaves
            .iter_mut()
            .filter(|leaf| leaf.component == component)
            .for_each(|leaf| leaf.read = true);
    }
}

/// What the projector does with one segment id, worked out once.
#[derive(Debug, Clone)]
pub(super) struct SegmentPlan<'s> {
    /// The defined elements to check, by position.
    pub(super) elements: Vec<ElementPlan<'s>>,
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

    /// Records that a column reads `element` (or its `component`) of the
    /// segments with `id`, when the spec defines that element.
    fn mark_read(&mut self, id: &[u8], element: usize, component: Option<usize>) {
        self.by_id
            .get_mut(id)
            .into_iter()
            .flat_map(|plan| plan.elements.iter_mut())
            .filter(|plan| plan.position == element)
            .for_each(|plan| plan.mark_read(component));
    }

    /// Marks every element and component some column of `table` can read.
    pub(super) fn mark_columns(&mut self, table: &TableDef) {
        for (_, source) in &table.columns {
            for (id, element, component) in self.positions(table, source) {
                self.mark_read(id, element, component);
            }
        }
    }

    /// The segment id, element and component of every place `source`, a
    /// column of `table`, can read. A column of a table anchored on a
    /// segment reads that segment too, and a group column reads its offset
    /// in every group of the defined elements the segment can hold.
    pub(super) fn positions<'t>(
        &self,
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
                    .get(anchor)
                    .into_iter()
                    .flat_map(|plan| plan.elements.iter().map(|e| e.position));
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

    /// Makes every column of table `index` (one without `segment`) watch the
    /// segment id it reads in each loop it reads from. A column that reads
    /// a loop above the anchor gets a carried value, which an instance of
    /// that loop opening starts over (recorded in `resets`, per loop); the
    /// carried values are returned in column order.
    pub(super) fn watch_columns(
        &mut self,
        spec: &'s Spec,
        index: usize,
        resets: &mut [Vec<(usize, usize)>],
    ) -> Vec<Carried> {
        let def = &spec.tables()[index];
        let loops = spec.loops().len();
        let mut carried = Vec::new();
        for (column, (_, source)) in def.columns.iter().enumerate() {
            let (reader, segment, occurrence, pick) = match source {
                ColumnSource::Element {
                    loop_id,
                    segment,
                    occurrence,
                    pick,
                    ..
                }
                | ColumnSource::SegmentIndex {
                    loop_id,
                    segment,
                    occurrence,
                    pick,
                    ..
                } => (*loop_id, segment, occurrence.as_deref(), *pick),
                ColumnSource::GroupElement { .. } => continue,
            };
            let above = reader.filter(|&id| {
                def.loops
                    .iter()
                    .any(|&anchor| spec.ancestors(anchor).contains(&id))
            });
            let slot = match above.and_then(|id| resets.get_mut(id.index())) {
                Some(reset) => {
                    reset.push((index, carried.len()));
                    carried.push(Carried::new(column));
                    Some(carried.len() - 1)
                }
                None => None,
            };
            let readers = reader.map_or_else(|| def.loops.clone(), |id| vec![id]);
            let plan = self.entry(segment, loops);
            for id in readers {
                let occurrence = match occurrence {
                    None => None,
                    // Compilation checked that every reading loop declares
                    // the occurrence.
                    Some(name) => {
                        match spec.get(id).occurrences.iter().position(|o| o.name == name) {
                            Some(found) => Some(found),
                            None => continue,
                        }
                    }
                };
                if let Some(watchers) = plan.watchers.get_mut(id.index()) {
                    watchers.push(Watcher {
                        table: index,
                        column,
                        occurrence,
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

/// One place the projector's plans read a column at, for tests that compare
/// it with the writer's places.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadPlace {
    /// Index into the spec's tables.
    pub(crate) table: usize,
    /// Index into the table's declared columns.
    pub(crate) column: usize,
    /// The loop whose captured segments the column reads.
    pub(crate) loop_id: crate::spec::LoopId,
    /// The occurrence (index in the loop's occurrences) a captured segment
    /// must match; `None` for any segment that holds the column's conditions
    /// and for a table anchored on a segment.
    pub(crate) occurrence: Option<usize>,
    /// Which matching segment gives the value; `None` for a table anchored
    /// on a segment, where each segment gives its own rows.
    pub(crate) pick: Option<Pick>,
}

#[cfg(test)]
impl super::Projector<'_> {
    /// Every place the plans read: one per watcher, then one per column of
    /// each table anchored on a segment, for each of its anchor loops.
    pub(crate) fn read_places(&self) -> Vec<ReadPlace> {
        use crate::spec::LoopId;
        let mut places = Vec::new();
        for plan in self.plans.by_id.values() {
            for (at, watchers) in plan.watchers.iter().enumerate() {
                places.extend(watchers.iter().map(|watcher| ReadPlace {
                    table: watcher.table,
                    column: watcher.column,
                    loop_id: LoopId(at),
                    occurrence: watcher.occurrence,
                    pick: Some(watcher.pick),
                }));
            }
        }
        for (at, tables) in self.segment_tables.iter().enumerate() {
            for &table in tables {
                let columns = self
                    .spec
                    .tables()
                    .get(table)
                    .map_or(0, |def| def.columns.len());
                places.extend((0..columns).map(|column| ReadPlace {
                    table,
                    column,
                    loop_id: LoopId(at),
                    occurrence: None,
                    pick: None,
                }));
            }
        }
        places
    }
}
