//! Element and segment definitions, and the names of the automatic columns.

use std::collections::BTreeMap;

use crate::value::{ElementType, LengthRange};

/// One element of a segment, or one component of a composite element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDef {
    /// Name used for the element downstream, e.g. `claim_submitter_id`.
    pub name: String,
    /// Data type.
    pub kind: ElementType,
    /// `true` when the element must be present and non-empty.
    pub required: bool,
    /// Lengths the element allows, as the spec sets them; a bound the spec
    /// leaves out is not checked.
    pub length: LengthRange,
    /// Components by 1-based position; empty for a simple element.
    pub composite: BTreeMap<usize, ElementDef>,
    /// The values the element may hold, sorted by their bytes; empty when
    /// any value of its type is accepted.
    pub codes: Vec<String>,
}

impl ElementDef {
    /// `true` when the element has a code list and `value` is not in it.
    pub(crate) fn rejects_code(&self, value: &[u8]) -> bool {
        rejects_code(&self.codes, value)
    }
}

/// `true` when `value` is one of `codes`, a list sorted by bytes.
#[inline]
pub(crate) fn holds_code(codes: &[String], value: &[u8]) -> bool {
    codes
        .binary_search_by(|code| code.as_bytes().cmp(value))
        .is_ok()
}

/// `true` when `codes` (sorted by bytes) is a code list and `value` is not
/// in it; an empty list rejects nothing.
pub(crate) fn rejects_code(codes: &[String], value: &[u8]) -> bool {
    !codes.is_empty() && !holds_code(codes, value)
}

/// The elements of one segment id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// Elements by 1-based position. Positions with no entry are opaque.
    pub elements: BTreeMap<usize, ElementDef>,
}

/// Name of the automatic column that numbers a table's rows from 0, across
/// the whole stream.
pub const ROW_COLUMN: &str = "row";

/// Name of the automatic column that holds the index of a row's anchor
/// segment: the segment that opened the anchor loop instance, or the
/// anchored segment itself.
pub const SEGMENT_COLUMN: &str = "segment";
