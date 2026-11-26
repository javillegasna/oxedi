//! The text of triggers and occurrence selectors for the layers that hold a
//! message as a string; the text itself is written in `problem/`.

use super::loops::Trigger;
use super::occurrences::OccurrenceDef;
use super::selector::Selector;

/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
pub(crate) fn render_trigger(trigger: &Trigger) -> String {
    Selector::trigger(trigger).to_string()
}

/// What selects an occurrence, as a message names it.
pub(crate) fn render_selector(occurrence: &OccurrenceDef) -> String {
    Selector::occurrence(occurrence).to_string()
}
