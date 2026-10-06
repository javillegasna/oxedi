//! Occurrence checks: the occurrences and child loops an instance requires,
//! how many times each may repeat, their position order, and segments that
//! match none of their loop's occurrences.
//!
//! Each open instance keeps, in one buffer shared by the whole stack, a count
//! per occurrence of its loop followed by a count per child loop, and the
//! occurrence with the highest position seen in it so far. A child loop
//! whose trigger occurrence has a higher position than its parent's shares
//! the parent's position space: opening it counts as its trigger occurrence
//! appearing in the parent. An implicit instance never saw its trigger and is
//! already reported as such, so what it lacks is not reported again.
//!
//! A segment of a loop that declares one occurrence of it always matches that
//! occurrence, even when its qualifier element holds an unexpected code: the
//! occurrence has no qualifier, and its own code list reports the value
//! instead. A validator that reads that code as a qualifier (pyx12 does)
//! reports the segment as not found and the occurrence as missing.

use crate::diagnostic::Rule;
use crate::segment::Segment;
use crate::spec::{LoopId, Spec, Usage, render_selector, render_trigger};

use super::EnvelopeChecker;

/// What the occurrence checks need of one loop, worked out once so that
/// opening an instance and capturing a segment read no more than this.
#[derive(Debug, Clone, Copy)]
pub(super) struct Layout {
    /// Index of the loop among its parent's children, or among the roots.
    slot: usize,
    /// Number of occurrences the loop declares.
    pub(super) occurrences: usize,
    /// Number of child loops.
    pub(super) children: usize,
    /// Most instances under one parent instance; `usize::MAX` for no limit.
    max: usize,
    /// Position of the trigger occurrence, when the loop shares its
    /// parent's position space.
    shared_pos: Option<usize>,
}

/// The layout of every loop, by loop index.
pub(super) fn layouts(spec: &Spec) -> Vec<Layout> {
    let first_pos = |id: LoopId| spec.get(id).occurrences.first().map(|o| o.pos);
    spec.loops()
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let siblings = match def.parent {
                Some(parent) => spec.get(parent).children.as_slice(),
                None => spec.roots(),
            };
            let own = def.occurrences.first().map(|o| o.pos);
            let parent = def.parent.and_then(first_pos);
            Layout {
                slot: siblings
                    .iter()
                    .position(|sibling| sibling.index() == index)
                    .unwrap_or(usize::MAX),
                occurrences: def.occurrences.len(),
                children: def.children.len(),
                max: def.max.unwrap_or(usize::MAX),
                shared_pos: own.filter(|&own| parent.is_some_and(|parent| own > parent)),
            }
        })
        .collect()
}

/// What opening an instance raises, worked out before it is pushed and
/// reported once it is, so the path names it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Opening {
    /// The instance's count under its parent, when past the loop's maximum.
    over_max: Option<usize>,
    /// The parent's occurrence the instance's trigger comes after.
    out_of_order: Option<(LoopId, usize, usize)>,
}

impl<'s> EnvelopeChecker<'s> {
    /// Counts a new instance of `id` under the innermost open instance (or
    /// the root) and, when its own trigger opened it, places it among the
    /// parent's occurrences.
    #[inline]
    pub(super) fn count_instance(&mut self, id: LoopId, implicit: bool) -> Opening {
        let Some(&layout) = self.layouts.get(id.index()) else {
            return Opening::default();
        };
        let counter = match self.open.last() {
            Some(parent) => self.layouts.get(parent.id.index()).and_then(|above| {
                self.counts
                    .get_mut(parent.counts + above.occurrences + layout.slot)
            }),
            None => self.root_counts.get_mut(layout.slot),
        };
        let mut opening = Opening::default();
        if let Some(count) = counter {
            *count = count.saturating_add(1);
            if *count > layout.max {
                opening.over_max = Some(*count);
            }
        }
        if !implicit
            && let Some(pos) = layout.shared_pos
            && let Some(parent) = self.open.last_mut()
        {
            match parent.last {
                Some(after) if after.2 > pos => opening.out_of_order = Some(after),
                _ => parent.last = Some((id, 0, pos)),
            }
        }
        opening
    }

    /// Reports what opening the instance of `id` now on top of the stack
    /// raised.
    pub(super) fn report_opening(
        &mut self,
        id: LoopId,
        opening: Opening,
        trigger: usize,
        datum: &[u8],
    ) {
        if let Some(count) = opening.over_max {
            self.loop_over_max(id, count, trigger, datum);
        }
        if let Some(after) = opening.out_of_order {
            let rule = out_of_order(self.spec, id, 0, after);
            self.report(rule, Some(trigger), None, datum.to_vec());
        }
    }

    #[cold]
    fn loop_over_max(&mut self, id: LoopId, count: usize, trigger: usize, datum: &[u8]) {
        let spec = self.spec;
        let def = spec.get(id);
        let Some(max) = def.max else {
            return;
        };
        let rule = Rule::LoopOverMax {
            loop_name: def.name.clone(),
            parent: def.parent.map(|parent| spec.loop_name(parent).to_string()),
            max,
            count,
        };
        self.report(rule, Some(trigger), None, datum.to_vec());
    }

    /// Counts a segment captured by the instance on top of the stack, of
    /// loop `id`, as the occurrence it `matched` (its index in the loop's
    /// occurrences), and reports a segment that matches none, an occurrence
    /// past its maximum, and a position out of order. Runs for every
    /// captured segment, so it reads the compact `limits` table and leaves
    /// the spec to the findings.
    #[inline]
    pub(super) fn occurrence_captured(
        &mut self,
        id: LoopId,
        segment: &Segment<'_>,
        matched: Option<usize>,
    ) {
        let Some(index) = matched else {
            self.unknown_occurrence(id, segment);
            return;
        };
        let Some(&(pos, max)) = self
            .first
            .get(id.index())
            .and_then(|&first| self.limits.get(first + index))
        else {
            return;
        };
        let Some(top) = self.open.last_mut().filter(|top| top.id == id) else {
            return;
        };
        let Some(count) = self.counts.get_mut(top.counts + index) else {
            return;
        };
        *count = count.saturating_add(1);
        let count = *count;
        let after = top.last;
        let before = after.is_some_and(|(_, _, last)| last > pos);
        if !before {
            top.last = Some((id, index, pos));
        }
        if count > max {
            self.over_max(id, index, segment, count);
        }
        if before && let Some(after) = after {
            let rule = out_of_order(self.spec, id, index, after);
            self.report(rule, Some(segment.index), None, segment.id.to_vec());
        }
    }

    /// Reports an occurrence that appears for the `count`th time in one
    /// instance, past its maximum.
    #[cold]
    fn over_max(&mut self, id: LoopId, index: usize, segment: &Segment<'_>, count: usize) {
        let def = self.spec.get(id);
        let Some(occurrence) = def.occurrences.get(index) else {
            return;
        };
        let Some(max) = occurrence.max else {
            return;
        };
        let rule = Rule::OccurrenceOverMax {
            loop_name: def.name.clone(),
            occurrence: occurrence.name.clone(),
            selector: render_selector(occurrence),
            max,
            count,
        };
        self.report(rule, Some(segment.index), None, segment.id.to_vec());
    }

    /// Reports a segment of loop `id` that matches none of the loop's
    /// occurrences of its id; the loop's end segment and the segments of a
    /// loop without occurrences match none and raise nothing.
    #[cold]
    fn unknown_occurrence(&mut self, id: LoopId, segment: &Segment<'_>) {
        let def = self.spec.get(id);
        let mut same = def
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.segment == segment.id);
        let Some(qualifier) = same.next().and_then(|first| first.qualifier.as_ref()) else {
            return;
        };
        let (element, component) = (qualifier.element, qualifier.component);
        let rule = Rule::UnknownOccurrence {
            loop_name: def.name.clone(),
            segment_id: segment.id.to_vec(),
            element,
            component,
            occurrences: def
                .occurrences
                .iter()
                .filter(|occurrence| occurrence.segment == segment.id)
                .map(|occurrence| occurrence.name.clone())
                .collect(),
        };
        let datum = segment
            .leaf(element, component)
            .unwrap_or_default()
            .to_vec();
        self.report_at(rule, Some(segment.index), Some(element), component, datum);
    }

    /// Reports the required occurrences and child loops the instance on top
    /// of the stack never held; `at` is the segment whose arrival closes it.
    pub(super) fn occurrence_closed(&mut self, at: Option<&Segment<'_>>) {
        let Some(top) = self.open.last() else {
            return;
        };
        let (id, start, implicit) = (top.id, top.counts, top.implicit);
        let missing = !implicit
            && self.required.get(id.index()).is_some_and(|slots| {
                slots
                    .iter()
                    .any(|&slot| self.counts.get(start + slot) == Some(&0))
            });
        if missing {
            self.report_missing(at);
        }
        self.counts.truncate(start);
    }

    /// Reports each required occurrence and child loop the instance on top
    /// of the stack never held.
    #[cold]
    fn report_missing(&mut self, at: Option<&Segment<'_>>) {
        let spec = self.spec;
        let Some(top) = self.open.last() else {
            return;
        };
        let (start, opened_at) = (top.counts, top.opened_at);
        let def = spec.get(top.id);
        let mut rules = Vec::new();
        let counts = self.counts.get(start..).unwrap_or_default();
        for (occurrence, &count) in def.occurrences.iter().zip(counts) {
            if occurrence.usage == Usage::Required && count == 0 {
                rules.push(Rule::RequiredOccurrenceMissing {
                    loop_name: def.name.clone(),
                    opened_at,
                    occurrence: occurrence.name.clone(),
                    selector: render_selector(occurrence),
                });
            }
        }
        let children = counts.get(def.occurrences.len()..).unwrap_or_default();
        for (&child, &count) in def.children.iter().zip(children) {
            let child_def = spec.get(child);
            if child_def.usage == Usage::Required && count == 0 {
                rules.push(Rule::RequiredLoopMissing {
                    loop_name: def.name.clone(),
                    opened_at,
                    child: child_def.name.clone(),
                    expected_trigger: render_trigger(&child_def.trigger),
                });
            }
        }
        for rule in rules {
            self.report(
                rule,
                at.map(|segment| segment.index),
                None,
                at.map(|segment| segment.id.to_vec()).unwrap_or_default(),
            );
        }
    }
}

/// The position and maximum (`usize::MAX` for none) of every occurrence of
/// every loop, loop after loop, and per loop where its occurrences start.
pub(super) fn limits(spec: &Spec) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut limits = Vec::new();
    let mut first = Vec::with_capacity(spec.loops().len());
    for def in spec.loops() {
        first.push(limits.len());
        limits.extend(
            def.occurrences
                .iter()
                .map(|occurrence| (occurrence.pos, occurrence.max.unwrap_or(usize::MAX))),
        );
    }
    (limits, first)
}

/// Per loop: the count slots of its required occurrences, then of its
/// required child loops (an instance's counts hold the occurrences first).
pub(super) fn required(spec: &Spec) -> Vec<Vec<usize>> {
    spec.loops()
        .iter()
        .map(|def| {
            let occurrences = def
                .occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.usage == Usage::Required)
                .map(|(slot, _)| slot);
            let children = def
                .children
                .iter()
                .enumerate()
                .filter(|(_, child)| spec.get(**child).usage == Usage::Required)
                .map(|(at, _)| def.occurrences.len() + at);
            occurrences.chain(children).collect()
        })
        .collect()
}

/// The order finding for occurrence `index` of loop `id`, which comes after
/// `after`, the `(loop, occurrence index, position)` seen highest so far.
fn out_of_order(spec: &Spec, id: LoopId, index: usize, after: (LoopId, usize, usize)) -> Rule {
    let def = spec.get(id);
    let after_def = spec.get(after.0);
    let name = |def: &crate::spec::LoopDef, at: usize| {
        def.occurrences
            .get(at)
            .map(|occurrence| occurrence.name.clone())
            .unwrap_or_default()
    };
    Rule::OutOfOrder {
        loop_name: def.name.clone(),
        occurrence: name(def, index),
        pos: def
            .occurrences
            .get(index)
            .map_or(0, |occurrence| occurrence.pos),
        after_loop: after_def.name.clone(),
        after: name(after_def, after.1),
        after_pos: after.2,
    }
}
