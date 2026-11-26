//! The text of an occurrence that was rejected.

use std::fmt;

use crate::message::place;
use crate::spec_load::OccurrenceError;

impl fmt::Display for OccurrenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OccurrenceError::EmptyName => write!(f, "the occurrence name is empty"),
            OccurrenceError::ZeroMax => write!(
                f,
                "\"max\" is 0; an occurrence that may appear appears at least once"
            ),
            OccurrenceError::UndefinedElement {
                key,
                segment,
                element,
                component,
            } => write!(
                f,
                "{key} names {}, which the \"segments\" section does not define for {segment:?}",
                place(segment, *element, *component)
            ),
            OccurrenceError::EmptyCodes { key } => {
                write!(f, "{key} is empty; list at least one code")
            }
            OccurrenceError::BadCodes { key, reason } => write!(f, "{key}: {reason}"),
            OccurrenceError::UsageLooserThanElement { key, place } => write!(
                f,
                "{key}: \"situational\" is looser than {place}, which the segment requires; \
                 an occurrence only narrows its element"
            ),
            OccurrenceError::BadLengths { key, reason } => write!(f, "{key}: {reason}"),
            OccurrenceError::LengthsOutsideElement {
                key,
                place,
                found,
                allowed,
            } => write!(
                f,
                "{key}: lengths {found} are not inside {allowed}, which {place} allows; \
                 an occurrence only narrows its element"
            ),
            OccurrenceError::QualifierInCodes { key } => write!(
                f,
                "{key} sits at the qualifier's own place; its codes are the \
                 qualifier's \"codes\""
            ),
            OccurrenceError::QualifierLengths { key, found } => write!(
                f,
                "{key}: lengths {found} sit at the qualifier's own place; its codes are \
                 checked against the element's own lengths, so only the element sets them"
            ),
            OccurrenceError::CodeOutsideElement {
                key,
                code,
                place,
                allowed,
            } => write!(
                f,
                "{key}: code {code:?} is not among the codes {place} allows ({})",
                allowed.join(", ")
            ),
            OccurrenceError::Indistinct { other, segment } => write!(
                f,
                "holds segment {segment:?} like occurrence {other:?}, and the two have no \
                 qualifier on one shared element to tell them apart"
            ),
            OccurrenceError::QualifierPlaces {
                other,
                place,
                other_place,
            } => write!(
                f,
                "reads its qualifier at {place} but occurrence {other:?} of the same segment \
                 reads it at {other_place}; every occurrence of one segment in a loop reads \
                 its qualifier at the same place"
            ),
            OccurrenceError::SharedCode {
                other,
                segment,
                code,
            } => write!(
                f,
                "holds segment {segment:?} like occurrence {other:?}, and both qualifiers \
                 accept code {code:?}"
            ),
        }
    }
}
