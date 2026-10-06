//! Envelope and structure checks over the engine's events.
//!
//! The checker follows the loops the engine opens and closes and reports what
//! the events alone reveal: segments no loop holds, loops opened without their
//! trigger, loops that close without their end segment, and end segments whose
//! count or control number disagrees with the loop they close. Which loops are
//! envelopes, and which elements carry the count and the control number, is
//! read from each loop's `control` in the spec.
//!
//! It also checks each loop instance against its loop's occurrences
//! (`occurrences.rs`): required occurrences and child loops, repeat limits,
//! position order and segments that match no occurrence.

use crate::delimiters::Delimiters;
use crate::diagnostic::{Diagnostic, LoopRef, Rule};
use crate::engine::Event;
use crate::frame::BYTE_ORDER_MARK;
use crate::segment::Segment;
use crate::spec::{ControlCount, LoopId, Spec, render_trigger};

/// One loop instance the checker is inside of.
#[derive(Debug, Clone)]
struct Open {
    id: LoopId,
    ordinal: usize,
    implicit: bool,
    /// Index of the trigger that opened the instance; `None` when implicit.
    opened_at: Option<usize>,
    /// Non-empty segments consumed before the trigger of this instance.
    start: usize,
    /// Child instances opened by their own trigger.
    children: usize,
    /// The trigger's control number, for an envelope opened by its trigger.
    control_number: Option<Vec<u8>>,
    /// `true` once the loop's end segment has been captured.
    ended: bool,
    /// Where the instance's counts start in the checker's `counts`: one per
    /// occurrence of its loop, then one per child loop.
    counts: usize,
    /// The occurrence with the highest position seen in the instance, as
    /// `(loop, index in its occurrences, position)`.
    last: Option<(LoopId, usize, usize)>,
}

/// Turns the engine's events into structural diagnostics, one segment at a time.
#[derive(Debug, Clone)]
pub struct EnvelopeChecker<'s> {
    spec: &'s Spec,
    /// Joins the components of a control value the file split.
    separator: u8,
    open: Vec<Open>,
    /// Instances opened so far, per loop index.
    ordinals: Vec<usize>,
    /// Non-empty segments consumed so far.
    seen: usize,
    /// The occurrence and child loop counts of every open instance.
    counts: Vec<usize>,
    /// Instances of each root loop so far.
    root_counts: Vec<usize>,
    /// Position and maximum (`usize::MAX` for none) of every occurrence of
    /// every loop, loop after loop.
    limits: Vec<(usize, usize)>,
    /// Per loop: where its occurrences start in `limits`.
    first: Vec<usize>,
    /// Per loop: what the occurrence checks need of it.
    layouts: Vec<occurrences::Layout>,
    /// Per loop: the count slots that must not stay 0.
    required: Vec<Vec<usize>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> EnvelopeChecker<'s> {
    /// A checker at the root, with nothing open. `delimiters` gives the
    /// component separator, which a control value read as one text keeps.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        let (limits, first) = occurrences::limits(spec);
        Self {
            spec,
            separator: delimiters.component,
            open: Vec::new(),
            ordinals: vec![0; spec.loops().len()],
            seen: 0,
            counts: Vec::new(),
            root_counts: vec![0; spec.roots().len()],
            limits,
            first,
            layouts: occurrences::layouts(spec),
            required: occurrences::required(spec),
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics they raise. The slice is valid until the next call. The
    /// first segment of a stream whose `raw` starts with a UTF-8 byte order
    /// mark also raises [`Rule::ByteOrderMark`], before anything else.
    pub fn on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic] {
        let spec = self.spec;
        let matched = events.iter().find_map(|event| match *event {
            Event::Captured { id, .. } => spec.get(id).occurrence_of(segment),
            _ => None,
        });
        self.on_matched(segment, events, matched)
    }

    /// [`on`](Self::on) with the occurrence the captured segment matched in
    /// its loop already resolved, as the index in the loop's occurrences.
    pub(crate) fn on_matched(
        &mut self,
        segment: &Segment<'_>,
        events: &[Event],
        matched: Option<usize>,
    ) -> &[Diagnostic] {
        self.diagnostics.clear();
        if segment.index == 0 && segment.raw.starts_with(BYTE_ORDER_MARK) {
            self.report(Rule::ByteOrderMark, Some(0), None, BYTE_ORDER_MARK.to_vec());
        }
        for &event in events {
            match event {
                Event::LoopOpened {
                    id,
                    implicit,
                    segment: trigger,
                } => self.opened(id, implicit, trigger, segment),
                Event::Captured { id, .. } => {
                    self.seen = self.seen.saturating_add(1);
                    self.captured(id, segment, matched);
                }
                Event::Unmatched { segment: index } => {
                    self.seen = self.seen.saturating_add(1);
                    self.report(
                        Rule::UnknownSegment {
                            id: segment.id.to_vec(),
                        },
                        Some(index),
                        None,
                        segment.id.to_vec(),
                    );
                }
                Event::LoopClosed { .. } => self.closed(Some(segment)),
                Event::Empty { .. } => {}
            }
        }
        &self.diagnostics
    }

    /// Closes every loop still open, as the engine's `finish` does, and
    /// returns the diagnostics that raises. The checker is then back at the
    /// root: feeding it again behaves like a fresh checker.
    pub fn finish(&mut self) -> &[Diagnostic] {
        self.diagnostics.clear();
        while !self.open.is_empty() {
            self.closed(None);
        }
        self.seen = 0;
        self.ordinals.iter_mut().for_each(|count| *count = 0);
        self.root_counts.iter_mut().for_each(|count| *count = 0);
        &self.diagnostics
    }

    fn opened(&mut self, id: LoopId, implicit: bool, trigger: usize, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(id);
        self.ordinals[id.index()] = self.ordinals[id.index()].saturating_add(1);
        let ordinal = self.ordinals[id.index()];
        if !implicit && let Some(parent) = self.open.last_mut() {
            parent.children = parent.children.saturating_add(1);
        }
        let opening = self.count_instance(id, implicit);
        let counts = self.counts.len();
        let slots = self
            .layouts
            .get(id.index())
            .map_or(0, |layout| layout.occurrences + layout.children);
        self.counts.resize(counts + slots, 0);
        let mut missing_opener = None;
        let control_number = match def.control {
            Some(control) if !implicit => {
                let value = text_at(segment, control.opener_element, self.separator);
                if value.is_none() {
                    missing_opener = Some(control.opener_element);
                }
                value
            }
            _ => None,
        };
        self.open.push(Open {
            id,
            ordinal,
            implicit,
            opened_at: (!implicit).then_some(trigger),
            start: self.seen,
            children: 0,
            control_number,
            ended: false,
            counts,
            last: None,
        });
        self.report_opening(id, opening, trigger, segment.id);
        if let Some(element) = missing_opener {
            self.report(
                Rule::ControlElementMissing {
                    segment_id: segment.id.to_vec(),
                    element,
                },
                Some(trigger),
                Some(element),
                Vec::new(),
            );
        }
        if implicit {
            self.report(
                Rule::ImplicitLoop {
                    loop_name: def.name.clone(),
                    expected_trigger: render_trigger(&def.trigger),
                    caused_by: segment.id.to_vec(),
                },
                Some(trigger),
                None,
                segment.id.to_vec(),
            );
        }
    }

    fn captured(&mut self, id: LoopId, segment: &Segment<'_>, matched: Option<usize>) {
        self.occurrence_captured(id, segment, matched);
        let spec = self.spec;
        let def = spec.get(id);
        if def.end.as_deref() != Some(segment.id) {
            return;
        }
        let seen = self.seen;
        let Some(top) = self.open.last_mut().filter(|top| top.id == id) else {
            return;
        };
        top.ended = true;
        let opened_at = top.opened_at;
        let Some(control) = def.control else {
            return;
        };
        let counted = match control.count {
            ControlCount::Segments => seen.saturating_sub(top.start),
            ControlCount::Children => top.children,
        };
        // The instance closes right after its end segment, so its control
        // number is no longer needed.
        let opener_value = top.control_number.take();

        match text_at(segment, control.count_element, self.separator) {
            None => self.report(
                Rule::ControlElementMissing {
                    segment_id: segment.id.to_vec(),
                    element: control.count_element,
                },
                Some(segment.index),
                Some(control.count_element),
                Vec::new(),
            ),
            Some(found) if parse_count(&found) != Some(counted) => self.report(
                Rule::ControlCountMismatch {
                    segment_id: segment.id.to_vec(),
                    element: control.count_element,
                    expected: counted,
                    found: found.to_vec(),
                },
                Some(segment.index),
                Some(control.count_element),
                found,
            ),
            Some(_) => {}
        }
        if let Some(opener_value) = opener_value {
            match text_at(segment, control.closer_element, self.separator) {
                None => self.report(
                    Rule::ControlElementMissing {
                        segment_id: segment.id.to_vec(),
                        element: control.closer_element,
                    },
                    Some(segment.index),
                    Some(control.closer_element),
                    Vec::new(),
                ),
                Some(closer_value) if closer_value != opener_value => self.report(
                    Rule::ControlNumberMismatch {
                        opener: def.trigger.segment.clone(),
                        opener_element: control.opener_element,
                        closer: segment.id.to_vec(),
                        closer_element: control.closer_element,
                        opener_value,
                        closer_value: closer_value.to_vec(),
                        opened_at,
                    },
                    Some(segment.index),
                    Some(control.closer_element),
                    closer_value,
                ),
                Some(_) => {}
            }
        }
    }

    /// Closes the innermost open loop; `at` is the segment whose arrival
    /// closed it, or `None` at the end of the stream.
    fn closed(&mut self, at: Option<&Segment<'_>>) {
        let spec = self.spec;
        let Some(top) = self.open.last() else {
            return;
        };
        let def = spec.get(top.id);
        // An implicit loop never saw its trigger; its missing end is part of
        // the same gap and is already reported by its opening.
        if let Some(end) = &def.end
            && !top.ended
            && !top.implicit
        {
            self.report(
                Rule::UnterminatedLoop {
                    loop_name: def.name.clone(),
                    expected_end: end.clone(),
                    opened_at: top.opened_at,
                },
                at.map(|segment| segment.index),
                None,
                at.map(|segment| segment.id.to_vec()).unwrap_or_default(),
            );
        }
        self.occurrence_closed(at);
        self.open.pop();
    }

    fn report(
        &mut self,
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        datum: Vec<u8>,
    ) {
        self.report_at(rule, segment, element, None, datum);
    }

    fn report_at(
        &mut self,
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        component: Option<usize>,
        datum: Vec<u8>,
    ) {
        let spec = self.spec;
        let path = self
            .open
            .iter()
            .map(|open| LoopRef {
                name: spec.loop_name(open.id).to_string(),
                ordinal: open.ordinal,
            })
            .collect();
        self.diagnostics.push(Diagnostic::new(
            rule, segment, element, component, path, datum,
        ));
    }
}

/// The whole text of the element at a 1-based position, components re-joined
/// with `separator`; `None` when the segment has no such element.
fn text_at(segment: &Segment<'_>, position: usize, separator: u8) -> Option<Vec<u8>> {
    let mut joined = Vec::new();
    segment
        .text(position, separator, &mut joined)
        .map(<[u8]>::to_vec)
}

/// A count written as ASCII digits (leading zeros allowed); `None` for
/// anything else, including the empty value and overflow.
fn parse_count(value: &[u8]) -> Option<usize> {
    if value.is_empty() {
        return None;
    }
    value.iter().try_fold(0usize, |count, &byte| {
        if byte.is_ascii_digit() {
            count.checked_mul(10)?.checked_add(usize::from(byte - b'0'))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod occurrence_tests;
mod occurrences;
#[cfg(test)]
mod tests;
