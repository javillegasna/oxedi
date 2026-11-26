//! What selects a loop, an occurrence or a column's segment, as a message
//! names it.

use super::loops::Trigger;
use super::occurrences::{OccurrenceDef, Qualifier};

/// What selects a loop, an occurrence or a column's segment, as a message
/// names it. Its `Display` is the text a message writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// A loop's trigger: the segment and the element values that open it.
    Trigger(Trigger),
    /// An occurrence: its segment and the qualifier that tells it apart from
    /// the other occurrences of that segment.
    Occurrence {
        /// The segment id.
        segment: Vec<u8>,
        /// The qualifier, when the occurrence has one.
        qualifier: Option<Qualifier>,
    },
    /// A column's segment and the `where` values that pick the occurrence it
    /// writes.
    Where {
        /// The segment id.
        segment: Vec<u8>,
        /// The 1-based element position and the value it must hold, in the
        /// order the spec writes them.
        conditions: Vec<(usize, Vec<u8>)>,
    },
}

impl Selector {
    /// The selector of a loop's trigger.
    pub(crate) fn trigger(trigger: &Trigger) -> Selector {
        Selector::Trigger(trigger.clone())
    }

    /// The selector of a column's segment and its `where` values.
    pub(crate) fn conditioned(segment: &[u8], conditions: &[(usize, Vec<u8>)]) -> Selector {
        Selector::Where {
            segment: segment.to_vec(),
            conditions: conditions.to_vec(),
        }
    }

    /// The selector of an occurrence.
    pub(crate) fn occurrence(occurrence: &OccurrenceDef) -> Selector {
        Selector::Occurrence {
            segment: occurrence.segment.clone(),
            qualifier: occurrence.qualifier.clone(),
        }
    }
}
