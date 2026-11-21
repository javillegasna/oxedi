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
//! position order and segments that match no occurrence; and each instance
//! against the spec's balancing rules (`balance.rs`): amounts that must add up.
//! The envelope control checks, the control number and the count at an end
//! segment, are in `control.rs`.

use crate::delimiters::Delimiters;
use crate::diagnostic::{Diagnostic, Rule, locate};
use crate::engine::{Event, Instance, InstanceId, lineage};
use crate::segment::Segment;
use crate::spec::{OccurrenceId, Spec};
use crate::trivia::BYTE_ORDER_MARK;

/// One loop instance the checker is inside of.
#[derive(Debug, Clone)]
struct Open {
    instance: InstanceId,
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
    /// Where the counts of its child loops start in the checker's `counts`.
    child_counts: usize,
    /// The occurrence with the highest position seen in the instance, as
    /// `(loop, index in its occurrences, position)`.
    last: Option<Seen>,
    /// The highest position reached inside the closed child instances that
    /// share the instance's position space: an occurrence of the instance
    /// itself must not come before it.
    inner: Option<Seen>,
}

/// Turns the engine's events into structural diagnostics, one segment at a time.
#[derive(Debug, Clone)]
pub struct StructureChecker<'s> {
    spec: &'s Spec,
    /// Joins the components of a control value the file split.
    separator: u8,
    open: Vec<Open>,
    /// Non-empty segments consumed so far.
    seen: usize,
    /// The occurrence and child loop counts of every open instance.
    counts: Vec<usize>,
    /// Instances of each root loop so far.
    root_counts: Vec<usize>,
    /// Position and maximum (`None` for no limit) of every occurrence of
    /// every loop, loop after loop.
    limits: Vec<(usize, Option<usize>)>,
    /// Per loop: where its occurrences start in `limits`.
    first: Vec<usize>,
    /// Per loop: what the occurrence checks need of it.
    layouts: Vec<occurrences::Layout>,
    /// Per loop: the count slots that must not stay 0.
    required: Vec<Vec<usize>>,
    /// The balancing rules and the totals of the open instances they check.
    balances: Balances,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> StructureChecker<'s> {
    /// A checker at the root, with nothing open. `delimiters` gives the
    /// component separator, which a control value read as one text keeps.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        let (limits, first) = occurrences::limits(spec);
        let balances = Balances::new(spec, &first);
        Self {
            spec,
            separator: delimiters.component,
            open: Vec::new(),
            seen: 0,
            counts: Vec::new(),
            root_counts: vec![0; spec.roots().len()],
            limits,
            first,
            layouts: occurrences::layouts(spec),
            required: occurrences::required(spec),
            balances,
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics they raise. `open` is `LoopEngine::path()` after the step;
    /// with the events it gives the loop path of each diagnostic. The slice is
    /// valid until the next call. The first segment of a stream whose `raw`
    /// starts with a UTF-8 byte order mark also raises [`Rule::ByteOrderMark`],
    /// before anything else, at the root.
    pub fn on(
        &mut self,
        segment: &Segment<'_>,
        events: &[Event],
        open: &[Instance],
    ) -> &[Diagnostic] {
        self.diagnostics.clear();
        self.check_byte_order_mark(segment);
        self.handle(Some(segment), events, open);
        &self.diagnostics
    }

    /// Handles each event, then gives the diagnostics it raised the loop path
    /// of its instance. `segment` is the one the events were returned for,
    /// `None` at the end of the stream.
    fn handle(&mut self, segment: Option<&Segment<'_>>, events: &[Event], open: &[Instance]) {
        for event in events {
            let before = self.diagnostics.len();
            self.step(event, segment);
            if self.diagnostics.len() > before {
                self.locate(before, event.instance(), open, events);
            }
        }
    }

    fn step(&mut self, event: &Event, segment: Option<&Segment<'_>>) {
        match *event {
            Event::LoopClosed { instance } => self.closed(instance, segment),
            Event::LoopOpened { .. } | Event::Captured { .. } | Event::Unmatched { .. } => {
                // The engine's `finish` only closes loops, so only `on` has a segment.
                if let Some(segment) = segment {
                    self.arrived(event, segment);
                }
            }
            Event::Empty { .. } => {}
        }
    }

    /// Handles an event that comes with the segment that caused it.
    fn arrived(&mut self, event: &Event, segment: &Segment<'_>) {
        match *event {
            Event::LoopOpened {
                instance,
                implicit,
                segment: trigger,
            } => self.opened(instance, implicit, trigger, segment),
            Event::Captured {
                instance,
                occurrence,
                ..
            } => {
                self.seen = self.seen.saturating_add(1);
                self.captured(instance, segment, occurrence.map(OccurrenceId::index));
            }
            Event::Unmatched { segment: index, .. } => {
                self.seen = self.seen.saturating_add(1);
                self.report(
                    Rule::UnknownSegment {
                        segment_id: segment.id.to_vec(),
                    },
                    Some(index),
                    None,
                    segment.id.to_vec(),
                );
            }
            Event::LoopClosed { .. } | Event::Empty { .. } => {}
        }
    }

    /// Writes the loop path of `instance` on the diagnostics from `first` on;
    /// an event no loop owns leaves the root path.
    #[cold]
    fn locate(
        &mut self,
        first: usize,
        instance: Option<Instance>,
        open: &[Instance],
        events: &[Event],
    ) {
        let Some(instance) = instance else {
            return;
        };
        let spec = self.spec;
        let path = lineage(instance, open, events);
        let names = path
            .iter()
            .map(|at| (spec.loop_name(at.loop_id), at.ordinal_in_stream));
        locate(&mut self.diagnostics[first..], names);
    }

    /// Reports a UTF-8 byte order mark that starts the first segment.
    fn check_byte_order_mark(&mut self, segment: &Segment<'_>) {
        if segment.index == 0 && segment.raw.starts_with(BYTE_ORDER_MARK) {
            self.report(Rule::ByteOrderMark, Some(0), None, BYTE_ORDER_MARK.to_vec());
        }
    }

    /// Consumes the events `LoopEngine::finish` returned, which close every
    /// loop still open, and returns the diagnostics that raises. The checker
    /// is then back at the root: feeding it again behaves like a fresh checker.
    pub fn finish(&mut self, events: &[Event]) -> &[Diagnostic] {
        self.diagnostics.clear();
        self.handle(None, events, &[]);
        self.open.clear();
        self.counts.clear();
        self.balances.clear_pending();
        self.seen = 0;
        self.root_counts.iter_mut().for_each(|count| *count = 0);
        &self.diagnostics
    }

    fn opened(
        &mut self,
        instance: Instance,
        implicit: bool,
        trigger: usize,
        segment: &Segment<'_>,
    ) {
        let id = instance.loop_id;
        if !implicit && let Some(parent) = self.open.last_mut() {
            parent.children = parent.children.saturating_add(1);
        }
        let opening = self.count_instance(id, implicit);
        let (counts, child_counts) = self.reserve_counts(id);
        let number = self.opener_number(id, implicit, segment);
        let missing_opener = number
            .as_ref()
            .and_then(|number| number.as_ref().err().copied());
        self.open.push(Open {
            instance: instance.id,
            implicit,
            opened_at: (!implicit).then_some(trigger),
            start: self.seen,
            children: 0,
            control_number: number.and_then(Result::ok),
            ended: false,
            counts,
            child_counts,
            last: None,
            inner: None,
        });
        self.balance_opened(id);
        self.report_opening(id, opening, trigger, segment.id);
        if let Some(element) = missing_opener {
            self.report_missing_opener(element, trigger, segment);
        }
        if implicit {
            self.report_implicit(id, trigger, segment);
        }
    }

    fn captured(&mut self, instance: Instance, segment: &Segment<'_>, matched: Option<usize>) {
        self.occurrence_captured(instance, segment, matched);
        self.balance_captured(instance.loop_id, segment, matched);
        self.end_captured(instance, segment);
    }

    /// Closes the innermost open loop when it is `instance`; `at` is the
    /// segment whose arrival closed it, or `None` at the end of the stream.
    fn closed(&mut self, instance: Instance, at: Option<&Segment<'_>>) {
        let spec = self.spec;
        let Some(top) = self.open.last().filter(|top| top.instance == instance.id) else {
            return;
        };
        let def = spec.get(instance.loop_id);
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
        self.occurrence_closed(instance.loop_id, at);
        self.balance_closed();
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
        // `handle` writes the path once the event is done.
        self.diagnostics.push(Diagnostic::new(
            rule,
            segment,
            element,
            component,
            Vec::new(),
            datum,
        ));
    }
}

mod balance;
mod control;
mod occurrences;
use balance::Balances;
use occurrences::Seen;
#[cfg(test)]
mod tests;
