//! Why a balancing rule was rejected.

use crate::message::ElementRef;

/// Why a balancing rule was rejected. Every variant that points inside the
/// rule names the key it sits at, e.g. `per`, `target[0].loop` or
/// `sum[1].elements[2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceError {
    /// The rule name is the empty string.
    EmptyName,
    /// `per` or a value's `loop` names a loop the spec does not declare.
    UnknownLoop {
        /// Where the name sits.
        key: String,
        /// The loop name as written.
        name: String,
    },
    /// `target` or `sum` lists no value.
    NoTerms {
        /// `target` or `sum`.
        key: &'static str,
    },
    /// A value reads a loop that is neither the rule's `per` loop nor one
    /// below it, so no single instance of `per` holds its segments.
    LoopOutsidePer {
        /// Where the loop is named (`target[0].loop`), or the value itself
        /// when it reads `per` by default.
        key: String,
        /// The loop the value reads.
        loop_name: String,
        /// The rule's `per` loop.
        per: String,
    },
    /// A value names an occurrence its loop does not declare.
    UnknownOccurrence {
        /// Where the name sits.
        key: String,
        /// The loop.
        loop_name: String,
        /// The occurrence name as written.
        occurrence: String,
    },
    /// A value lists no element.
    NoElements {
        /// Where the list sits.
        key: String,
    },
    /// A value names an element or component the `segments` section does not
    /// define for the occurrence's segment.
    UndefinedElement {
        /// Where the position sits.
        key: String,
        /// The element or component, e.g. `CLP03` or `SVC01-2`.
        place: ElementRef,
    },
    /// A value names an element or component that is not a decimal amount.
    NotDecimal {
        /// Where the position sits.
        key: String,
        /// The element or component, e.g. `CLP01`.
        place: ElementRef,
        /// Its declared type code, e.g. `AN`.
        found: String,
    },
    /// The same element of the same occurrence is counted twice in the rule.
    RepeatedValue {
        /// Where the second mention sits.
        key: String,
        /// Where the first mention sits.
        first: String,
        /// The element or component, e.g. `CAS03`.
        place: ElementRef,
    },
}
