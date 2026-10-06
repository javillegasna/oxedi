//! Table definitions: repeats, picks and column sources.

use std::fmt;

#[cfg(doc)]
use super::Spec;
use super::loops::LoopId;

/// How a table's segment repeats a group of elements: one row per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// 1-based position of the first group's first element.
    pub from: usize,
    /// Elements per group.
    pub step: usize,
}

/// Which of the segments that match a column's source gives its value.
///
/// The segments considered are, in file order: for a source in the anchor
/// loop or a loop inside it, those captured while the row's anchor instance
/// is open (in that instance, or in any instance of the inner loop inside
/// it); for a source in a loop above the anchor, those the enclosing
/// instance of that loop captured before the row's anchor instance opened
/// (an instance captures nothing while a loop inside it is open, so these
/// are all it captures until the row is appended). A table anchored on a
/// segment reads that segment only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pick {
    /// The first segment that matches; written `"first"`, and the default.
    #[default]
    First,
    /// The last segment that matches; written `"last"`.
    Last,
    /// The n-th segment that matches, counting from 1; written as the
    /// number. The value is null when fewer segments match.
    Nth(usize),
}

impl fmt::Display for Pick {
    /// The pick as a spec writes it: `"first"`, `"last"` or the position.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pick::First => write!(f, "\"first\""),
            Pick::Last => write!(f, "\"last\""),
            Pick::Nth(nth) => write!(f, "{nth}"),
        }
    }
}

/// Where a column takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnSource {
    /// An element (or one of its components) of the segment that `pick`
    /// chooses among those that match `segment` and `conditions` (or the
    /// named `occurrence`), read in the anchor loop instance, in the loops
    /// `loop_id` names inside it, or in the instance of `loop_id` above it;
    /// see [`Pick`]. In a table anchored on a segment, the anchor segment
    /// itself.
    Element {
        /// The loop to read from: one inside the anchor loop, or one above
        /// it; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id; for an occurrence, the occurrence's segment.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position;
        /// empty for an occurrence.
        conditions: Vec<(usize, Vec<u8>)>,
        /// The occurrence of the reading loop the segment must match, by
        /// name, instead of `segment` and `conditions`.
        occurrence: Option<String>,
        /// Which matching segment gives the value.
        pick: Pick,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
    /// The index of the segment chosen as for `Element`.
    SegmentIndex {
        /// The loop to read from: one inside the anchor loop, or one above
        /// it; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id; for an occurrence, the occurrence's segment.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position;
        /// empty for an occurrence.
        conditions: Vec<(usize, Vec<u8>)>,
        /// The occurrence of the reading loop the segment must match, by
        /// name, instead of `segment` and `conditions`.
        occurrence: Option<String>,
        /// Which matching segment gives the index.
        pick: Pick,
    },
    /// An element of the row's group, in a table whose segment repeats a
    /// group: position `from + k * step + offset` for group `k`.
    GroupElement {
        /// 0-based position inside the group.
        offset: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
}

/// One table of the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    /// Name used in the JSON, e.g. `claims`.
    pub name: String,
    /// Name of the column that refers to a row of this table from the
    /// tables below it, e.g. `claim`; the table's name unless the spec says.
    pub reference: String,
    /// The loops whose instances (or whose segments) give rows.
    pub loops: Vec<LoopId>,
    /// With a segment, one row per occurrence of it in an anchor loop
    /// instead of one row per loop instance.
    pub segment: Option<Vec<u8>>,
    /// With a repeat, one row per element group of the segment.
    pub repeat: Option<Repeat>,
    /// The declared columns by name, in name order.
    pub columns: Vec<(String, ColumnSource)>,
    /// The nearest table above this one, as an index into [`Spec::tables`].
    pub parent: Option<usize>,
    /// Every table above this one, outermost first; the last is `parent`.
    /// Each one gets an automatic column named after its `reference`.
    pub ancestors: Vec<usize>,
}
