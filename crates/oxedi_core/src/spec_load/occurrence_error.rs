//! Why an occurrence of a loop was rejected.

use super::segments::ElementDefError;
use crate::value::LengthRange;

/// Why an occurrence was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OccurrenceError {
    /// The occurrence name is the empty string.
    EmptyName,
    /// `max` is 0.
    ZeroMax,
    /// The qualifier or an `elements` key names an element or component the
    /// `segments` section does not define for the segment.
    UndefinedElement {
        /// Where the reference sits: `qualifier` or `elements.<key>`.
        key: String,
        /// The segment id.
        segment: String,
        /// The element position.
        element: usize,
        /// The component position, if any.
        component: Option<usize>,
    },
    /// A code list (the qualifier's or an entry's) is empty.
    EmptyCodes {
        /// Where the list sits: `qualifier.codes` or `elements.<key>.codes`.
        key: String,
    },
    /// A code list (the qualifier's or an entry's) is invalid for its element.
    BadCodes {
        /// Where the list sits: `qualifier.codes` or `elements.<key>.codes`.
        key: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// An entry's `usage` is `situational` where the element is required.
    UsageLooserThanElement {
        /// Where the usage sits: `elements.<key>.usage`.
        key: String,
        /// The element or component, e.g. `NM102`.
        place: String,
    },
    /// An entry's `min` and `max` are not valid lengths.
    BadLengths {
        /// Where the entry sits: `elements.<key>`.
        key: String,
        /// What is wrong with them.
        reason: ElementDefError,
    },
    /// An entry's lengths reach outside the element's.
    LengthsOutsideElement {
        /// Where the entry sits: `elements.<key>`.
        key: String,
        /// The element or component, e.g. `NM103`.
        place: String,
        /// The lengths the entry sets, with the element's bounds where it sets none.
        found: LengthRange,
        /// The element's own lengths.
        allowed: LengthRange,
    },
    /// An entry lists codes at the qualifier's own element or component.
    QualifierInCodes {
        /// Where the list sits: `elements.<key>.codes`.
        key: String,
    },
    /// An entry sets `min` or `max` at the qualifier's own element or component.
    QualifierLengths {
        /// Where the entry sits: `elements.<key>`.
        key: String,
        /// The lengths the entry sets, with the element's bounds where it sets none.
        found: LengthRange,
    },
    /// A code of a list (the qualifier's or an entry's) is not one the
    /// element's own `codes` list allows.
    CodeOutsideElement {
        /// Where the list sits: `qualifier.codes` or `elements.<key>.codes`.
        key: String,
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
    /// Another occurrence holds the same segment with a qualifier on a
    /// different element or component: every occurrence of one segment in a
    /// loop reads its qualifier at the same place.
    QualifierPlaces {
        /// The other occurrence.
        other: String,
        /// The place this occurrence's qualifier reads, e.g. `NM02`.
        place: String,
        /// The place the other's qualifier reads, e.g. `NM01`.
        other_place: String,
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
