//! The per-segment plans the projector builds once from the spec.

use std::collections::BTreeMap;

use crate::column::ColumnType;
use crate::spec::{ColumnSource, ElementDef, Spec, TableDef};

/// One defined element, with what checking it needs worked out once.
#[derive(Debug, Clone)]
pub(super) struct ElementPlan<'s> {
    pub(super) position: usize,
    pub(super) def: &'s ElementDef,
    /// The column type the definition maps to.
    pub(super) kind: ColumnType,
    /// Highest component position the definition declares (0 for a simple
    /// element).
    pub(super) declared: usize,
    /// The components by position, with their column types and whether a
    /// column reads them.
    pub(super) components: Vec<(usize, &'s ElementDef, ColumnType, bool)>,
    /// Whether a column reads the element as a whole. An element or
    /// component no column reads is validated without keeping its value.
    pub(super) read: bool,
}

impl<'s> ElementPlan<'s> {
    pub(super) fn new(position: usize, def: &'s ElementDef) -> Self {
        Self {
            position,
            def,
            kind: ColumnType::of(Some(def.kind)),
            declared: def.composite.keys().copied().max().unwrap_or_default(),
            components: def
                .composite
                .iter()
                .map(|(&at, part)| (at, part, ColumnType::of(Some(part.kind)), false))
                .collect(),
            read: false,
        }
    }

    /// Records that a column reads this element (`component` `None`) or one
    /// of its declared components.
    fn mark_read(&mut self, component: Option<usize>) {
        match component {
            None => self.read = true,
            Some(wanted) => self
                .components
                .iter_mut()
                .filter(|(at, ..)| *at == wanted)
                .for_each(|(.., read)| *read = true),
        }
    }
}

/// What the projector does with one segment id, worked out once.
#[derive(Debug, Clone)]
pub(super) struct SegmentPlan<'s> {
    /// The defined elements to check, by position.
    pub(super) elements: Vec<ElementPlan<'s>>,
    /// Per loop: `(table, column)` for the columns that read this segment
    /// when it is captured in that loop.
    pub(super) watchers: Vec<Vec<(usize, usize)>>,
}

/// The plans of every segment id. Ids of up to seven bytes are keyed by their
/// packed bytes, which compare faster than slices.
#[derive(Debug, Clone, Default)]
pub(super) struct Plans<'s> {
    short: BTreeMap<u64, SegmentPlan<'s>>,
    long: BTreeMap<Vec<u8>, SegmentPlan<'s>>,
}

impl<'s> Plans<'s> {
    /// The packed key of an id, or `None` when it is too long to pack. A
    /// leading 1 bit keeps ids of different lengths apart.
    fn key(id: &[u8]) -> Option<u64> {
        (id.len() <= 7).then(|| {
            id.iter()
                .fold(1u64, |key, &byte| key << 8 | u64::from(byte))
        })
    }

    pub(super) fn get(&self, id: &[u8]) -> Option<&SegmentPlan<'s>> {
        match Self::key(id) {
            Some(key) => self.short.get(&key),
            None => self.long.get(id),
        }
    }

    /// Records that a column reads `element` (or its `component`) of the
    /// segments with `id`, when the spec defines that element.
    fn mark_read(&mut self, id: &[u8], element: usize, component: Option<usize>) {
        let plan = match Self::key(id) {
            Some(key) => self.short.get_mut(&key),
            None => self.long.get_mut(id),
        };
        plan.into_iter()
            .flat_map(|plan| plan.elements.iter_mut())
            .filter(|plan| plan.position == element)
            .for_each(|plan| plan.mark_read(component));
    }

    /// Marks every element and component some column of `table` can read.
    /// A column of a table anchored on a segment reads that segment too, and
    /// a group column reads its offset in every group the segment can hold.
    pub(super) fn mark_columns(&mut self, table: &TableDef) {
        for (_, source) in &table.columns {
            match source {
                ColumnSource::Element {
                    segment,
                    element,
                    component,
                    ..
                } => {
                    self.mark_read(segment, *element, *component);
                    if let Some(anchor) = &table.segment {
                        self.mark_read(anchor, *element, *component);
                    }
                }
                ColumnSource::GroupElement { offset, component } => {
                    let (Some(anchor), Some(repeat)) = (&table.segment, table.repeat) else {
                        continue;
                    };
                    let Some(first) = repeat.from.checked_add(*offset) else {
                        continue;
                    };
                    let positions: Vec<usize> = self
                        .get(anchor)
                        .map(|plan| plan.elements.iter().map(|e| e.position).collect())
                        .unwrap_or_default();
                    for position in positions {
                        let in_group = position
                            .checked_sub(first)
                            .is_some_and(|gap| gap.checked_rem(repeat.step).unwrap_or(gap) == 0);
                        if in_group {
                            self.mark_read(anchor, position, *component);
                        }
                    }
                }
                ColumnSource::SegmentIndex { .. } => {}
            }
        }
    }

    /// The plan of `id`, created empty (with room for `loops` watcher lists)
    /// when there is none.
    pub(super) fn entry(&mut self, id: &[u8], loops: usize) -> &mut SegmentPlan<'s> {
        let empty = || SegmentPlan {
            elements: Vec::new(),
            watchers: vec![Vec::new(); loops],
        };
        match Self::key(id) {
            Some(key) => self.short.entry(key).or_insert_with(empty),
            None => self.long.entry(id.to_vec()).or_insert_with(empty),
        }
    }
}

/// The column type of a declared column: its element's type, `Int64` for a
/// segment index, `Binary` for an element the spec does not define.
pub(super) fn column_type(spec: &Spec, table: &TableDef, source: &ColumnSource) -> ColumnType {
    let def = match source {
        ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } => spec.element_def(segment, *element, *component),
        ColumnSource::SegmentIndex { .. } => return ColumnType::Int64 { scale: 0 },
        ColumnSource::GroupElement { offset, component } => match (&table.segment, table.repeat) {
            (Some(segment), Some(repeat)) => repeat
                .from
                .checked_add(*offset)
                .and_then(|position| spec.element_def(segment, position, *component)),
            _ => None,
        },
    };
    ColumnType::of(def.map(|def| def.kind))
}
