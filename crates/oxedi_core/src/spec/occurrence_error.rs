//! Why an occurrence of a loop was rejected.

use std::fmt;

use super::segments::ElementDefError;

/// Why an occurrence was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OccurrenceError {
    /// The occurrence name is the empty string.
    EmptyName,
    /// `usage` is not `required` or `situational`.
    UnknownUsage {
        /// The value as written.
        found: String,
    },
    /// `max` is 0.
    ZeroMax,
    /// The qualifier or a `codes` key names an element or component the
    /// `segments` section does not define for the segment.
    UndefinedElement {
        /// Where the reference sits: `qualifier` or `codes.<key>`.
        key: String,
        /// The segment id.
        segment: String,
        /// The element position.
        element: usize,
        /// The component position, if any.
        component: Option<usize>,
    },
    /// A `codes` key is not `<element>` or `<element>-<component>`, 1-based
    /// in canonical form.
    BadCodesKey {
        /// The key as written.
        key: String,
    },
    /// A code list (the qualifier's or one under `codes`) is empty.
    EmptyCodes {
        /// Where the list sits: `qualifier.codes` or `codes.<key>`.
        key: String,
    },
    /// A code list (the qualifier's or one under `codes`) is invalid for its element.
    BadCodes {
        /// Where the list sits: `qualifier.codes` or `codes.<key>`.
        key: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// A `codes` key names the qualifier's own element or component.
    QualifierInCodes {
        /// The key as written.
        key: String,
    },
    /// A qualifier code is not one the element's own `codes` list allows.
    QualifierCodeOutsideElement {
        /// The code as written.
        code: String,
        /// The element or component, e.g. `NM101`.
        place: String,
        /// The element's own codes, sorted.
        allowed: Vec<String>,
    },
    /// Another occurrence holds the same segment and the two have no
    /// qualifier on one shared element or component to tell them apart.
    Indistinct {
        /// The other occurrence.
        other: String,
        /// The segment id both hold.
        segment: String,
    },
    /// Another occurrence holds the same segment and both qualifiers accept a code.
    SharedCode {
        /// The other occurrence.
        other: String,
        /// The segment id both hold.
        segment: String,
        /// The code both accept.
        code: String,
    },
}

pub(super) fn place(segment: &str, element: usize, component: Option<usize>) -> String {
    match component {
        Some(component) => format!("{segment}{element:02}-{component}"),
        None => format!("{segment}{element:02}"),
    }
}

impl fmt::Display for OccurrenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OccurrenceError::EmptyName => write!(f, "the occurrence name is empty"),
            OccurrenceError::UnknownUsage { found } => write!(
                f,
                "\"usage\" must be \"required\" or \"situational\"; found {found:?}"
            ),
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
            OccurrenceError::BadCodesKey { key } => write!(
                f,
                "codes key {key:?} must be \"<element>\" or \"<element>-<component>\", \
                 1-based integers in canonical form"
            ),
            OccurrenceError::EmptyCodes { key } => {
                write!(f, "{key} is empty; list at least one code")
            }
            OccurrenceError::BadCodes { key, reason } => write!(f, "{key}: {reason}"),
            OccurrenceError::QualifierInCodes { key } => write!(
                f,
                "codes key {key:?} names the qualifier's own place; its codes are the \
                 qualifier's \"codes\""
            ),
            OccurrenceError::QualifierCodeOutsideElement {
                code,
                place,
                allowed,
            } => write!(
                f,
                "qualifier.codes: code {code:?} is not among the codes {place} allows ({})",
                allowed.join(", ")
            ),
            OccurrenceError::Indistinct { other, segment } => write!(
                f,
                "holds segment {segment:?} like occurrence {other:?}, and the two have no \
                 qualifier on one shared element to tell them apart"
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
