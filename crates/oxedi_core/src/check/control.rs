//! Envelope control checks: the control number an envelope's trigger carries,
//! and, at the loop's end segment, its count and the number it repeats.
//!
//! The checks are pure functions that return their finding; the checker
//! reports them in order.

use crate::diagnostic::Rule;
use crate::engine::Instance;
use crate::segment::Segment;
use crate::spec::{Control, ControlCount, LoopDef, LoopId, render_trigger};
use crate::value::parse_unsigned;

use super::{Open, StructureChecker};

/// A control finding: the rule, the 1-based element it is about and the datum.
type Finding = (Rule, usize, Vec<u8>);

/// A count written as ASCII digits (leading zeros allowed); `None` for
/// anything else, including the empty value and a count past `usize`.
pub(super) fn parse_count(value: &[u8]) -> Option<usize> {
    parse_unsigned(value).and_then(|count| usize::try_from(count).ok())
}

/// The finding for an element the end segment does not have.
fn missing_finding(segment: &Segment<'_>, element: usize) -> Finding {
    (
        Rule::ControlElementMissing {
            segment_id: segment.id.to_vec(),
            element,
        },
        element,
        Vec::new(),
    )
}

/// What the end segment's count element says against `counted`: missing, or
/// not the number of what the loop counted.
fn count_finding(
    control: &Control,
    counted: usize,
    segment: &Segment<'_>,
    separator: u8,
) -> Option<Finding> {
    let element = control.count_element;
    let mut joined = Vec::new();
    match segment.text(element, separator, &mut joined) {
        None => Some(missing_finding(segment, element)),
        Some(found) if parse_count(found) != Some(counted) => Some((
            Rule::ControlCountMismatch {
                segment_id: segment.id.to_vec(),
                element,
                expected: counted,
                found: found.to_vec(),
            },
            element,
            found.to_vec(),
        )),
        Some(_) => None,
    }
}

/// What the end segment's number element says against the number the trigger
/// carried: missing, or different.
fn number_finding(
    def: &LoopDef,
    control: &Control,
    opener_value: Vec<u8>,
    opened_at: Option<usize>,
    segment: &Segment<'_>,
    separator: u8,
) -> Option<Finding> {
    let element = control.closer_element;
    let mut joined = Vec::new();
    match segment.text(element, separator, &mut joined) {
        None => Some(missing_finding(segment, element)),
        Some(closer_value) if closer_value != opener_value => Some((
            Rule::ControlNumberMismatch {
                opener: def.trigger.segment.clone(),
                opener_element: control.opener_element,
                closer: segment.id.to_vec(),
                closer_element: element,
                opener_value,
                closer_value: closer_value.to_vec(),
                opened_at,
            },
            element,
            closer_value.to_vec(),
        )),
        Some(_) => None,
    }
}

/// What an instance's control counts, at its end segment.
fn counted(control: &Control, seen: usize, top: &Open) -> usize {
    match control.count {
        ControlCount::Segments => seen.saturating_sub(top.start),
        ControlCount::Children => top.children,
    }
}

impl StructureChecker<'_> {
    /// The control number an envelope's trigger carries: `Ok` with its text,
    /// `Err` with the 1-based element that is missing. `None` for a loop with
    /// no control or an instance opened implicitly.
    pub(super) fn opener_number(
        &self,
        id: LoopId,
        implicit: bool,
        segment: &Segment<'_>,
    ) -> Option<Result<Vec<u8>, usize>> {
        let control = self.spec.get(id).control.filter(|_| !implicit)?;
        let mut joined = Vec::new();
        let value = segment.text(control.opener_element, self.separator, &mut joined);
        Some(value.map(<[u8]>::to_vec).ok_or(control.opener_element))
    }

    /// Reports a trigger that lacks its control number element.
    #[cold]
    pub(super) fn report_missing_opener(
        &mut self,
        element: usize,
        trigger: usize,
        segment: &Segment<'_>,
    ) {
        let (rule, element, datum) = missing_finding(segment, element);
        self.report(rule, Some(trigger), Some(element), datum);
    }

    /// Reports an instance opened by a segment other than its trigger.
    #[cold]
    pub(super) fn report_implicit(&mut self, id: LoopId, trigger: usize, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(id);
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

    /// Marks the instance on top as ended when `segment` is the end segment of
    /// its loop, and checks the count and the control number it carries. A
    /// segment that is not the loop's end, or an entry of another loop on top,
    /// changes nothing.
    pub(super) fn end_captured(&mut self, instance: Instance, segment: &Segment<'_>) {
        let spec = self.spec;
        let def = spec.get(instance.loop_id);
        if def.end.as_deref() != Some(segment.id) {
            return;
        }
        let seen = self.seen;
        let separator = self.separator;
        let Some(top) = self
            .open
            .last_mut()
            .filter(|top| top.instance == instance.id)
        else {
            return;
        };
        top.ended = true;
        let Some(control) = def.control else {
            return;
        };
        let counted = counted(&control, seen, top);
        let opened_at = top.opened_at;
        // The instance closes right after its end segment, so its control
        // number is no longer needed.
        let opener_value = top.control_number.take();
        let findings = [
            count_finding(&control, counted, segment, separator),
            opener_value.and_then(|opener_value| {
                number_finding(def, &control, opener_value, opened_at, segment, separator)
            }),
        ];
        for (rule, element, datum) in findings.into_iter().flatten() {
            self.report(rule, Some(segment.index), Some(element), datum);
        }
    }
}
