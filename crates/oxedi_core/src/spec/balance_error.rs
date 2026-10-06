//! Why a balancing rule was rejected.

use std::fmt;

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
        place: String,
    },
    /// A value names an element or component that is not a decimal amount.
    NotDecimal {
        /// Where the position sits.
        key: String,
        /// The element or component, e.g. `CLP01`.
        place: String,
        /// Its declared type code, e.g. `AN`.
        found: String,
    },
    /// `sign` is not `+` or `-`.
    UnknownSign {
        /// Where the sign sits.
        key: String,
        /// The value as written.
        found: String,
    },
    /// The same element of the same occurrence is counted twice in the rule.
    RepeatedValue {
        /// Where the second mention sits.
        key: String,
        /// Where the first mention sits.
        first: String,
        /// The element or component, e.g. `CAS03`.
        place: String,
    },
}

impl fmt::Display for BalanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BalanceError::EmptyName => write!(f, "the rule name is empty"),
            BalanceError::UnknownLoop { key, name } => {
                write!(
                    f,
                    "{key} names loop {name:?}, which the spec does not declare"
                )
            }
            BalanceError::NoTerms { key } => {
                write!(f, "{key} is empty; list at least one value")
            }
            BalanceError::LoopOutsidePer {
                key,
                loop_name,
                per,
            } => write!(
                f,
                "{key} reads loop {loop_name:?}, which is neither the rule's \"per\" loop \
                 {per:?} nor a loop below it"
            ),
            BalanceError::UnknownOccurrence {
                key,
                loop_name,
                occurrence,
            } => write!(
                f,
                "{key} names occurrence {occurrence:?}, which loop {loop_name:?} does not declare"
            ),
            BalanceError::NoElements { key } => {
                write!(f, "{key} is empty; list at least one element position")
            }
            BalanceError::UndefinedElement { key, place } => write!(
                f,
                "{key} names {place}, which the \"segments\" section does not define"
            ),
            BalanceError::NotDecimal { key, place, found } => write!(
                f,
                "{key} names {place}, of type {found}; a balancing value must be of type R"
            ),
            BalanceError::UnknownSign { key, found } => {
                write!(f, "{key} must be \"+\" or \"-\"; found {found:?}")
            }
            BalanceError::RepeatedValue { key, first, place } => write!(
                f,
                "{key} counts {place} of the same occurrence that {first} already counts"
            ),
        }
    }
}
