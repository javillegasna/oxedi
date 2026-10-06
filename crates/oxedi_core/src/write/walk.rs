//! The loop instances the rows give inside an instance: one per row of the
//! table anchored on the loop, one per run of rows with equal values for a
//! loop the columns group, one around a row's columns of a loop below its
//! anchor (when they have a value or the loop is required), one around the
//! instances of the loops inside an implied loop, and one per envelope.

use crate::column::Cell;
use crate::spec::{LoopId, Usage};

use super::emit::Emitter;
use super::plan::{Instances, SegmentSource, ValueSource};

/// The rows an instance sits in.
#[derive(Debug, Clone)]
pub(super) struct Context {
    /// By table: the position of the row the instance sits in.
    pub(super) rows: Vec<Option<usize>>,
    /// Inside an instance given by a run of grouped rows: the table and the
    /// run, as a range of the rows its parent row holds.
    pub(super) run: Option<(usize, usize, usize)>,
}

impl Emitter<'_> {
    pub(super) fn loop_instances(&self, id: LoopId) -> Option<Instances> {
        self.plan.loops.get(id.index()).map(|plan| plan.instances)
    }

    /// The rows of `table` in the context: those its parent row holds, cut
    /// to the context's run when the run is of that table.
    fn rows_in(&self, table: usize, context: &Context) -> &[usize] {
        let parent = self.spec.tables().get(table).and_then(|def| def.parent);
        let rows = match parent {
            Some(parent) => match context.rows.get(parent).copied().flatten() {
                Some(at) => self.nest.rows(table, Some(at)),
                None => &[],
            },
            None => self.nest.rows(table, None),
        };
        match context.run {
            Some((run_table, from, to)) if run_table == table => rows.get(from..to).unwrap_or(&[]),
            _ => rows,
        }
    }

    /// The instances of loop `id` inside an instance with `context`.
    pub(super) fn instances(&self, id: LoopId, context: &Context) -> Vec<Context> {
        let Some(kind) = self.loop_instances(id) else {
            return Vec::new();
        };
        match kind {
            Instances::Absent => Vec::new(),
            Instances::Envelope => vec![context.clone()],
            Instances::Rows { table } => self
                .rows_in(table, context)
                .iter()
                .map(|&row| {
                    let mut inner = context.clone();
                    if let Some(slot) = inner.rows.get_mut(table) {
                        *slot = Some(row);
                    }
                    inner.run = None;
                    inner
                })
                .collect(),
            Instances::Groups { table } => self.groups(id, table, context),
            Instances::Inside { table } => {
                let Some(row) = context.rows.get(table).copied().flatten() else {
                    return Vec::new();
                };
                let required = self.spec.get(id).usage == Usage::Required;
                if required || self.has_data(id, table, row) {
                    vec![context.clone()]
                } else {
                    Vec::new()
                }
            }
            Instances::Implied => {
                let written = self
                    .spec
                    .get(id)
                    .children
                    .iter()
                    .any(|&child| !self.instances(child, context).is_empty());
                if written {
                    vec![context.clone()]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// One instance per run of consecutive rows of `table` whose columns
    /// reading loop `id` hold the same values.
    fn groups(&self, id: LoopId, table: usize, context: &Context) -> Vec<Context> {
        let rows = self.rows_in(table, context);
        let offset = match context.run {
            Some((run_table, from, _)) if run_table == table => from,
            _ => 0,
        };
        let columns = self
            .layout
            .group_columns
            .get(id.index())
            .map_or(&[][..], Vec::as_slice);
        let Some(data) = self.data.get(table) else {
            return Vec::new();
        };
        let same = |a: usize, b: usize| {
            columns
                .iter()
                .all(|&column| data.cell(column, a) == data.cell(column, b))
        };
        let mut instances = Vec::new();
        let mut from = 0;
        while let Some(&first) = rows.get(from) {
            let to = rows
                .iter()
                .skip(from)
                .position(|&row| !same(first, row))
                .map_or(rows.len(), |at| from + at);
            let mut inner = context.clone();
            if let Some(slot) = inner.rows.get_mut(table) {
                *slot = Some(first);
            }
            inner.run = Some((table, offset + from, offset + to));
            instances.push(inner);
            from = to;
        }
        instances
    }

    /// Whether a column of `table` writes a value in loop `id`, or in a loop
    /// inside it that the same row gives, for `row`.
    fn has_data(&self, id: LoopId, table: usize, row: usize) -> bool {
        let Some(data) = self.data.get(table) else {
            return false;
        };
        let own = self.plan.loops.get(id.index()).is_some_and(|plan| {
            plan.segments
                .iter()
                .filter(|segment| matches!(segment.source, SegmentSource::Row { table: t, .. } if t == table))
                .flat_map(|segment| &segment.elements)
                .any(|element| {
                    matches!(element.value, ValueSource::Column { column } if data.cell(column, row) != Cell::Null)
                })
        });
        own || self.spec.get(id).children.iter().any(|&child| {
            self.loop_instances(child) == Some(Instances::Inside { table })
                && self.has_data(child, table, row)
        })
    }
}
