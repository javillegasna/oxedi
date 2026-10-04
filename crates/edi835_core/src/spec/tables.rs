//! Table definitions: repeats, column sources, anchor chains and their errors.

use std::fmt;

#[cfg(doc)]
use super::Spec;
use super::loops::LoopId;
use super::render::render_chain;

/// How a table's segment repeats a group of elements: one row per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// 1-based position of the first group's first element.
    pub from: usize,
    /// Elements per group.
    pub step: usize,
}

/// Where a column takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnSource {
    /// An element (or one of its components) of the first segment that
    /// matches `segment` and `conditions`, captured in the anchor loop
    /// instance, or in the first instance of `loop_id` inside it. In a table
    /// anchored on a segment, the anchor segment itself.
    Element {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
    /// The index of the first segment that matches, chosen as for `Element`.
    SegmentIndex {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
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

/// The tables above two anchor loops, outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorChains {
    /// The tables above the first anchor loop.
    pub first: Vec<String>,
    /// The tables above the second anchor loop.
    pub second: Vec<String>,
}

/// Why a table definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDefError {
    /// A table name, column name or `ref` is the empty string.
    EmptyName {
        /// Which one: `table name`, `column name` or `ref`.
        what: &'static str,
    },
    /// `loops` is empty.
    NoLoops,
    /// A loop name appears twice in `loops`.
    DuplicateLoop {
        /// The loop name as written.
        loop_name: String,
    },
    /// A loop name does not exist.
    UnknownLoop {
        /// The name as written.
        name: String,
    },
    /// A column name or `ref` is taken by an automatic column.
    ReservedName {
        /// The name as written.
        name: String,
    },
    /// Another table already uses this `ref`.
    RefTaken {
        /// The `ref` as written.
        name: String,
        /// The table that uses it first, in name order.
        table: String,
    },
    /// `repeat` was given without `segment`.
    RepeatWithoutSegment,
    /// `repeat.step` is 0.
    ZeroStep,
    /// A 1-based position holds 0.
    ZeroPosition {
        /// The key: `repeat.from`, `element` or `component`.
        key: &'static str,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The key as written.
        key: String,
    },
    /// `group_element` does not fit inside a group.
    OffsetBeyondStep {
        /// The offset as written.
        offset: usize,
        /// The group size.
        step: usize,
    },
    /// Two anchor loops of a table without `segment` nest.
    NestedAnchors {
        /// The enclosing loop.
        outer: String,
        /// The loop inside it.
        inner: String,
    },
    /// The loop already anchors another table without `segment`.
    SharedAnchor {
        /// The loop.
        loop_name: String,
        /// The table that anchors in it first, in name order.
        other: String,
    },
    /// Two anchor loops lead to tables above that are not one chain.
    UnrelatedAnchors {
        /// The anchor loop with the longest chain of tables above it.
        first: String,
        /// The anchor loop whose chain disagrees.
        second: String,
        /// The table names above each anchor loop.
        chains: Box<AnchorChains>,
    },
    /// A column's `loop` is not inside every anchor loop.
    NotADescendant {
        /// The column's loop.
        loop_name: String,
        /// The anchor loop it is not inside.
        anchor: String,
    },
    /// A segment is read from a loop that neither triggers on it nor holds it.
    SegmentNotHeld {
        /// The segment the column or table reads.
        segment: String,
        /// The loop that never holds it.
        loop_name: String,
    },
    /// A key that picks a segment is used in a table anchored on a segment.
    AnchorSegmentOnly {
        /// The key: `segment`, `loop` or `where`.
        key: &'static str,
        /// The segment the table is anchored on.
        anchor_segment: String,
        /// The key's value as the spec writes it, in JSON.
        written: String,
    },
    /// A column of a table without `segment` names no segment.
    NeedsSegment,
    /// `group_element` is used in a table without `repeat`.
    GroupWithoutRepeat,
    /// A column does not name exactly one value source.
    SourceCount {
        /// How many of `element`, `group_element` and `segment_index` it names.
        found: usize,
    },
    /// `component` is given with `segment_index`.
    ComponentOnIndex,
}

impl fmt::Display for TableDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableDefError::EmptyName { what } => write!(f, "the {what} is empty"),
            TableDefError::DuplicateLoop { loop_name } => {
                write!(
                    f,
                    "loop {loop_name:?} is listed more than once in \"loops\""
                )
            }
            TableDefError::NoLoops => {
                write!(
                    f,
                    "\"loops\" is empty; a table anchors in at least one loop"
                )
            }
            TableDefError::UnknownLoop { name } => write!(f, "loop {name:?} does not exist"),
            TableDefError::ReservedName { name } => {
                write!(f, "the name {name:?} is taken by an automatic column")
            }
            TableDefError::RefTaken { name, table } => {
                write!(f, "\"ref\" {name:?} is already used by table {table:?}")
            }
            TableDefError::RepeatWithoutSegment => write!(
                f,
                "\"repeat\" requires \"segment\": only a segment's elements repeat"
            ),
            TableDefError::ZeroStep => write!(f, "\"repeat.step\" is 0"),
            TableDefError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based position; found 0")
            }
            TableDefError::BadPosition { key } => write!(
                f,
                "\"where\" position {key:?} is not a 1-based integer in canonical form"
            ),
            TableDefError::OffsetBeyondStep { offset, step } => write!(
                f,
                "\"group_element\" {offset} is outside a group of {step} elements (offsets start at 0)"
            ),
            TableDefError::NestedAnchors { outer, inner } => write!(
                f,
                "anchor loops {outer:?} and {inner:?} nest; a table without \"segment\" anchors in loops that do not"
            ),
            TableDefError::SharedAnchor { loop_name, other } => write!(
                f,
                "loop {loop_name:?} already anchors table {other:?}; a loop anchors at most one table without \"segment\""
            ),
            TableDefError::UnrelatedAnchors {
                first,
                second,
                chains,
            } => write!(
                f,
                "anchor loops {first:?} and {second:?} sit under tables that are not one chain: {first:?} under {}, {second:?} under {}",
                render_chain(&chains.first),
                render_chain(&chains.second)
            ),
            TableDefError::NotADescendant { loop_name, anchor } => {
                write!(f, "loop {loop_name:?} is not inside anchor loop {anchor:?}")
            }
            TableDefError::SegmentNotHeld { segment, loop_name } => write!(
                f,
                "segment {segment:?} is neither the trigger nor a segment of loop {loop_name:?}, so it is never read there"
            ),
            TableDefError::AnchorSegmentOnly {
                key,
                anchor_segment,
                written,
            } => write!(
                f,
                "{key:?} ({written}) does not apply in a table anchored on segment {anchor_segment:?}: its columns read that segment"
            ),
            TableDefError::NeedsSegment => write!(f, "the column names no \"segment\" to read"),
            TableDefError::GroupWithoutRepeat => {
                write!(f, "\"group_element\" requires the table's \"repeat\"")
            }
            TableDefError::SourceCount { found } => write!(
                f,
                "a column takes exactly one of \"element\", \"group_element\" or \"segment_index\"; found {found}"
            ),
            TableDefError::ComponentOnIndex => {
                write!(f, "\"component\" does not apply to \"segment_index\"")
            }
        }
    }
}
