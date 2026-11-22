//! Rendering of triggers and occurrence selectors for error messages.

use super::loops::Trigger;
use super::occurrences::OccurrenceDef;
use crate::message::{Conditions, ElementRef, MAX_LISTED_CODES, render_names};

/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
pub(crate) fn render_trigger(trigger: &Trigger) -> String {
    let segment = String::from_utf8_lossy(&trigger.segment);
    if trigger.conditions.is_empty() {
        return format!("{segment:?} with no conditions");
    }
    format!("{segment:?}{}", Conditions(&trigger.conditions))
}

/// What selects an occurrence: `"N3"`, `"PER" where PER01 is "BL"`,
/// `"DTM" where DTM01 is one of "232", "233"`, or for a longer list
/// `"REF" where REF01 is one of 14 codes`.
pub(crate) fn render_selector(occurrence: &OccurrenceDef) -> String {
    let segment = String::from_utf8_lossy(&occurrence.segment);
    let Some(qualifier) = &occurrence.qualifier else {
        return format!("{segment:?}");
    };
    let at = ElementRef::new(&occurrence.segment, qualifier.element, qualifier.component);
    let codes = match qualifier.codes.as_slice() {
        [code] => format!("is {code:?}"),
        codes if codes.len() <= MAX_LISTED_CODES => format!("is one of {}", render_names(codes)),
        codes => format!("is one of {} codes", codes.len()),
    };
    format!("{segment:?} where {at} {codes}")
}
