//! Why a table definition is rejected, with the context its message needs.

use std::fmt;

use super::render::{render_chain, render_names};
use super::tables::Pick;

/// The tables above two anchor loops, outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorChains {
    /// The tables above the first anchor loop.
    pub first: Vec<String>,
    /// The tables above the second anchor loop.
    pub second: Vec<String>,
}

/// An occurrence's segment in two anchor loops of one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopSegments {
    /// The first anchor loop.
    pub first_loop: String,
    /// The occurrence's segment there.
    pub first_segment: String,
    /// The anchor loop where the segment differs.
    pub loop_name: String,
    /// The occurrence's segment there.
    pub segment: String,
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
    /// A column's `loop` is neither an anchor loop, a loop inside it nor a
    /// loop above it, for some anchor loop.
    UnrelatedLoop {
        /// The column's loop.
        loop_name: String,
        /// The anchor loop it is not in line with.
        anchor: String,
    },
    /// A column names an occurrence its reading loop does not declare.
    UnknownOccurrence {
        /// The reading loop.
        loop_name: String,
        /// The occurrence as written.
        occurrence: String,
        /// The loop's occurrences, in name order.
        known: Box<[String]>,
    },
    /// A column names an occurrence together with `segment` or `where`.
    OccurrenceAndSegment {
        /// The key: `segment` or `where`.
        key: &'static str,
        /// The key's value as the spec writes it, in JSON.
        written: String,
    },
    /// A column names an occurrence that is a different segment in two of
    /// the table's anchor loops.
    OccurrenceSegments {
        /// The occurrence as written.
        occurrence: String,
        /// The two anchor loops and the occurrence's segment in each.
        segments: Box<LoopSegments>,
    },
    /// `pick` is given without `occurrence`.
    PickNeedsOccurrence {
        /// The pick as the spec writes it, in JSON.
        written: String,
    },
    /// `pick` is neither `"first"`, `"last"` nor a position from 1.
    BadPick {
        /// The pick as the spec writes it, in JSON.
        written: String,
    },
    /// `pick` is given for an occurrence that appears at most once.
    PickOnSingle {
        /// The reading loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// The pick.
        pick: Pick,
    },
    /// `pick` names a position past the occurrence's maximum repeat.
    PickBeyondMax {
        /// The position.
        nth: usize,
        /// The reading loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// Its maximum repeat.
        max: usize,
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
        /// The key: `segment`, `loop`, `where`, `occurrence` or `pick`.
        key: &'static str,
        /// The segment the table is anchored on.
        anchor_segment: String,
        /// The key's value as the spec writes it, in JSON.
        written: String,
    },
    /// A column of a table without `segment` names no segment or occurrence.
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
            TableDefError::UnrelatedLoop { loop_name, anchor } => write!(
                f,
                "loop {loop_name:?} is neither anchor loop {anchor:?}, a loop inside it nor a loop above it"
            ),
            TableDefError::UnknownOccurrence {
                loop_name,
                occurrence,
                known,
            } => {
                write!(f, "loop {loop_name:?} has no occurrence {occurrence:?}; ")?;
                if known.is_empty() {
                    write!(f, "it declares none")
                } else {
                    write!(f, "it declares {}", render_names(known))
                }
            }
            TableDefError::OccurrenceAndSegment { key, written } => write!(
                f,
                "\"occurrence\" already names the segment and its qualifier; {key:?} ({written}) does not go with it"
            ),
            TableDefError::OccurrenceSegments {
                occurrence,
                segments,
            } => write!(
                f,
                "occurrence {occurrence:?} is segment {:?} in anchor loop {:?} but segment {:?} in anchor loop {:?}; name the loop to read with \"loop\"",
                segments.first_segment, segments.first_loop, segments.segment, segments.loop_name
            ),
            TableDefError::PickNeedsOccurrence { written } => write!(
                f,
                "\"pick\" ({written}) requires \"occurrence\": only a named occurrence has a known repeat"
            ),
            TableDefError::BadPick { written } => write!(
                f,
                "\"pick\" must be \"first\", \"last\" or a 1-based position; found {written}"
            ),
            TableDefError::PickOnSingle {
                loop_name,
                occurrence,
                pick,
            } => write!(
                f,
                "occurrence {occurrence:?} of loop {loop_name:?} appears at most once, so \"pick\" ({pick}) has nothing to choose from"
            ),
            TableDefError::PickBeyondMax {
                nth,
                loop_name,
                occurrence,
                max,
            } => write!(
                f,
                "\"pick\" {nth} is past occurrence {occurrence:?} of loop {loop_name:?}, which repeats at most {max} times"
            ),
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
            TableDefError::NeedsSegment => write!(
                f,
                "the column names no \"segment\" or \"occurrence\" to read"
            ),
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
