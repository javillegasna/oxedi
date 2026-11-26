//! The element checks each projector builds once from the spec's segment
//! definitions, with the read marks of the spec's index.

use crate::spec::{ElementDef, IdMap, Spec};
use crate::value::ColumnType;

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

/// The element checks of every segment id.
#[derive(Debug, Clone, Default)]
pub(super) struct Checks<'s> {
    by_id: IdMap<Vec<ElementCheck<'s>>>,
}

impl<'s> Checks<'s> {
    /// The defined elements of segment `id` to check, by position.
    pub(super) fn get(&self, id: &[u8]) -> Option<&[ElementCheck<'s>]> {
        self.by_id.get(id).map(Vec::as_slice)
    }

    /// Sets the element checks of segment `id`.
    pub(super) fn insert(&mut self, id: &[u8], elements: Vec<ElementCheck<'s>>) {
        *self.by_id.entry_or_insert_with(id, Vec::new) = elements;
    }
}
