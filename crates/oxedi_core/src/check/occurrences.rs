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
use crate::element::Element;
use crate::segment::Segment;
use crate::spec::{LoopId, Usage, render_selector, render_trigger};

use super::EnvelopeChecker;

/// What opening an instance raises, worked out before it is pushed and
/// reported once it is, so the path names it.
pub(super) struct Opening {
    over_max: Option<Rule>,
    out_of_order: Option<Rule>,
}

impl<'s> EnvelopeChecker<'s> {
    /// Counts a new instance of `id` under the innermost open instance (or
    /// the root) and, when its own trigger opened it, places it among the
    /// parent's occurrences.
    pub(super) fn count_instance(&mut self, id: LoopId, implicit: bool) -> Opening {
        let spec = self.spec;
        let def = spec.get(id);
        let (count, parent_name) = match self.open.last() {
            Some(parent) => {
                let parent_def = spec.get(parent.id);
                let slot = parent_def
                    .children
                    .iter()
                    .position(|&child| child == id)
                    .map(|at| parent.counts + parent_def.occurrences.len() + at);
                let count = slot
                    .and_then(|slot| self.counts.get_mut(slot))
                    .map(|count| {
                        *count = count.saturating_add(1);
                        *count
                    });
                (count, Some(parent_def.name.clone()))
            }
            None => {
                let slot = spec.roots().iter().position(|&root| root == id);
                let count = slot
                    .and_then(|slot| self.root_counts.get_mut(slot))
                    .map(|count| {
                        *count = count.saturating_add(1);
                        *count
                    });
                (count, None)
            }
        };
        let over_max = match (def.max, count) {
            (Some(max), Some(count)) if count > max => Some(Rule::LoopOverMax {
                loop_name: def.name.clone(),
                parent: parent_name,
                max,
                count,
            }),
            _ => None,
        };
        let out_of_order = if implicit { None } else { self.place_child(id) };
        Opening {
            over_max,
            out_of_order,
        }
    }

    /// Reports what opening the instance now on top of the stack raised.
    pub(super) fn report_opening(&mut self, opening: Opening, trigger: usize, datum: &[u8]) {
        for rule in [opening.over_max, opening.out_of_order]
            .into_iter()
            .flatten()
        {
            self.report(rule, Some(trigger), None, datum.to_vec());
        }
    }

    /// Records the trigger occurrence of child loop `id` as appearing in the
    /// innermost open instance, when the two share a position space, and
    /// returns the order finding it raises.
    fn place_child(&mut self, id: LoopId) -> Option<Rule> {
        let spec = self.spec;
        let def = spec.get(id);
        let parent = self.open.last_mut()?;
        let first = def.occurrences.first()?;
        let parent_first = spec.get(parent.id).occurrences.first()?;
        if first.pos <= parent_first.pos {
            return None;
        }
        if let Some((after_loop, after)) = parent.last {
            let after_def = spec.get(after_loop);
            if let Some(after) = after_def.occurrences.get(after)
                && after.pos > first.pos
            {
                return Some(Rule::OutOfOrder {
                    loop_name: def.name.clone(),
                    occurrence: first.name.clone(),
                    pos: first.pos,
                    after_loop: after_def.name.clone(),
                    after: after.name.clone(),
                    after_pos: after.pos,
                });
            }
        }
        parent.last = Some((id, 0));
        None
    }

    /// Matches a segment captured by the instance on top of the stack, of
    /// loop `id`, to its occurrence, and reports a segment that matches
    /// none, an occurrence past its maximum, and a position out of order.
    pub(super) fn occurrence_captured(&mut self, id: LoopId, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(id);
        if def.occurrences.is_empty() {
            return;
        }
        let Some(index) = def.occurrence_of(segment) else {
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
            let datum = qualifier_value(segment, element, component).to_vec();
            self.report_at(rule, Some(segment.index), Some(element), component, datum);
            return;
        };
        let Some(top) = self.open.last_mut().filter(|top| top.id == id) else {
            return;
        };
        let Some(occurrence) = def.occurrences.get(index) else {
            return;
        };
        let count = self.counts.get_mut(top.counts + index).map(|count| {
            *count = count.saturating_add(1);
            *count
        });
        let mut rules = Vec::new();
        if let (Some(max), Some(count)) = (occurrence.max, count)
            && count > max
        {
            rules.push(Rule::OccurrenceOverMax {
                loop_name: def.name.clone(),
                occurrence: occurrence.name.clone(),
                selector: render_selector(occurrence),
                max,
                count,
            });
        }
        let after = top.last.and_then(|(after_loop, after)| {
            let after_def = spec.get(after_loop);
            after_def
                .occurrences
                .get(after)
                .map(|after| (after_def, after))
        });
        match after {
            Some((after_def, after)) if after.pos > occurrence.pos => {
                rules.push(Rule::OutOfOrder {
                    loop_name: def.name.clone(),
                    occurrence: occurrence.name.clone(),
                    pos: occurrence.pos,
                    after_loop: after_def.name.clone(),
                    after: after.name.clone(),
                    after_pos: after.pos,
                });
            }
            _ => top.last = Some((id, index)),
        }
        for rule in rules {
            self.report(rule, Some(segment.index), None, segment.id.to_vec());
        }
    }

    /// Reports the required occurrences and child loops the instance on top
    /// of the stack never held; `at` is the segment whose arrival closes it.
    pub(super) fn occurrence_closed(&mut self, at: Option<&Segment<'_>>) {
        let spec = self.spec;
        let Some(top) = self.open.last() else {
            return;
        };
        let (start, implicit, opened_at) = (top.counts, top.implicit, top.opened_at);
        let def = spec.get(top.id);
        let mut rules = Vec::new();
        if !implicit {
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
        }
        for rule in rules {
            self.report(
                rule,
                at.map(|segment| segment.index),
                None,
                at.map(|segment| segment.id.to_vec()).unwrap_or_default(),
            );
        }
        self.counts.truncate(start);
    }
}

/// The value at a qualifier's place; empty when the segment has none.
fn qualifier_value<'a>(
    segment: &'a Segment<'_>,
    element: usize,
    component: Option<usize>,
) -> &'a [u8] {
    match (segment.element(element), component) {
        (Some(Element::Simple(value)), None | Some(1)) => value,
        (Some(Element::Composite(parts)), Some(component)) => component
            .checked_sub(1)
            .and_then(|index| parts.get(index))
            .map_or(&[], AsRef::as_ref),
        (Some(Element::Composite(parts)), None) => parts.first().map_or(&[], AsRef::as_ref),
        _ => &[],
    }
}
