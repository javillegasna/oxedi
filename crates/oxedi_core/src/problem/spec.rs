//! The text of the values that select a loop or an occurrence.

use std::fmt;

use crate::message::{Conditions, ElementRef, MAX_LISTED_CODES, render_names};
use crate::spec::Selector;

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selector::Trigger(trigger) => {
                let segment = String::from_utf8_lossy(&trigger.segment);
                if trigger.conditions.is_empty() {
                    return write!(f, "{segment:?} with no conditions");
                }
                write!(f, "{segment:?}{}", Conditions(&trigger.conditions))
            }
            Selector::Occurrence {
                segment,
                qualifier: None,
            } => write!(f, "{:?}", String::from_utf8_lossy(segment)),
            Selector::Occurrence {
                segment,
                qualifier: Some(qualifier),
            } => {
                let at = ElementRef::new(segment, qualifier.element, qualifier.component);
                write!(
                    f,
                    "{:?} where {at} {}",
                    String::from_utf8_lossy(segment),
                    Codes(&qualifier.codes)
                )
            }
        }
    }
}

/// The codes of a qualifier: one code, a short list or only the count.
struct Codes<'a>(&'a [String]);

impl fmt::Display for Codes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [code] => write!(f, "is {code:?}"),
            codes if codes.len() <= MAX_LISTED_CODES => {
                write!(f, "is one of {}", render_names(codes))
            }
            codes => write!(f, "is one of {} codes", codes.len()),
        }
    }
}
