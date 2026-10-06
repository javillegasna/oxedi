//! What the emitter works out once per spec: the order of each loop's
//! segments and child loops, which segments are written even without data,
//! the columns that tell one grouped instance from the next, the depth of
//! each envelope loop and how many element groups a repeat segment holds.

use crate::spec::{LoopId, Spec};

use super::plan::{Instances, SegmentSource, ValueSource, WritePlan};

/// One thing a loop instance writes, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Item {
    /// A segment, by index into the loop's plan segments.
    Segment(usize),
    /// The instances of a child loop.
    Loop(LoopId),
}

/// The per-spec layout of the emitter.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    /// By loop: what an instance writes, in order.
    pub(super) order: Vec<Vec<Item>>,
    /// By loop, by plan segment: written even when no column of it has a
    /// value, because it opens the instance (the first segment of the
    /// trigger occurrence). Another required occurrence without a value is
    /// left out, and reading the file back reports it missing.
    pub(super) forced: Vec<Vec<bool>>,
    /// By loop given instances by groups of rows: the columns whose values
    /// tell one group from the next.
    pub(super) group_columns: Vec<Vec<usize>>,
    /// By loop: its depth among the envelope loops, for one with a `control`.
    pub(super) depth: Vec<Option<usize>>,
    /// By table: how many element groups one of its segments holds (1 for a
    /// table without a repeat).
    pub(super) capacity: Vec<usize>,
}

impl Layout {
    pub(super) fn new(spec: &Spec, plan: &WritePlan) -> Layout {
        let mut layout = Layout {
            order: Vec::with_capacity(spec.loops().len()),
            forced: Vec::with_capacity(spec.loops().len()),
            group_columns: Vec::with_capacity(spec.loops().len()),
            depth: Vec::with_capacity(spec.loops().len()),
            capacity: spec
                .tables()
                .iter()
                .map(|table| {
                    let (Some(segment), Some(repeat)) = (&table.segment, table.repeat) else {
                        return 1;
                    };
                    let last = spec
                        .segment(segment)
                        .and_then(|def| def.elements.keys().next_back().copied())
                        .unwrap_or(repeat.from);
                    match last.checked_sub(repeat.from) {
                        Some(span) if repeat.step > 0 => span / repeat.step + 1,
                        _ => 1,
                    }
                })
                .collect(),
        };
        for (index, def) in spec.loops().iter().enumerate() {
            let segments = plan
                .loops
                .get(index)
                .map_or(&[][..], |plan| plan.segments.as_slice());
            let pos = |at: usize| def.occurrences.get(at).map_or(usize::MAX, |o| o.pos);
            let own = def.trigger_occurrence().map(pos);
            let mut items: Vec<(usize, u8, Item)> = segments
                .iter()
                .enumerate()
                .map(|(i, segment)| (pos(segment.occurrence), 0, Item::Segment(i)))
                .collect();
            for &child in &def.children {
                let child_def = spec.get(child);
                let at = child_def
                    .trigger_occurrence()
                    .and_then(|at| child_def.occurrences.get(at))
                    .map(|o| o.pos)
                    .filter(|&at| own.is_some_and(|own| at > own))
                    .unwrap_or(usize::MAX);
                items.push((at, 1, Item::Loop(child)));
            }
            items.sort_by_key(|&(pos, kind, _)| (pos, kind));
            layout
                .order
                .push(items.into_iter().map(|(_, _, item)| item).collect());
            let trigger = def.trigger_occurrence();
            let mut seen = false;
            layout.forced.push(
                segments
                    .iter()
                    .map(|segment| {
                        let opens = Some(segment.occurrence) == trigger
                            && matches!(segment.source, SegmentSource::Row { .. });
                        let first = opens && !seen;
                        seen |= opens;
                        first
                    })
                    .collect(),
            );
            let grouped = match plan.loops.get(index).map(|plan| plan.instances) {
                Some(Instances::Groups { table }) => Some(table),
                _ => None,
            };
            layout.group_columns.push(
                segments
                    .iter()
                    .filter(|segment| {
                        matches!(segment.source, SegmentSource::Row { table, .. } if Some(table) == grouped)
                    })
                    .flat_map(|segment| &segment.elements)
                    .filter_map(|element| match element.value {
                        ValueSource::Column { column } => Some(column),
                        _ => None,
                    })
                    .collect(),
            );
            let depth = def.control.map(|_| {
                let mut depth = 0;
                let mut above = def.parent;
                while let Some(id) = above {
                    if spec.get(id).control.is_some() {
                        depth += 1;
                    }
                    above = spec.get(id).parent;
                }
                depth
            });
            layout.depth.push(depth);
        }
        layout
    }
}
