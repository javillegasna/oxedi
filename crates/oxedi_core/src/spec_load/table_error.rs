//! Why a table definition is rejected, with the context its message needs.

use std::fmt;

use super::table_chain::{Declared, render_chain};
use crate::message::ElementRef;
use crate::spec::ColumnKind;
use crate::vocabulary::Pick;

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

/// Where a column's occurrence above the anchor sits, and where the anchor
/// opens, in one position space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorPositions {
    /// The loop above the anchor.
    pub loop_name: String,
    /// The occurrence's position.
    pub pos: usize,
    /// The anchor loop.
    pub anchor: String,
    /// The position of the anchor's trigger occurrence.
    pub anchor_pos: usize,
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
    /// A column reads a loop above the anchor at segments that all come
    /// after the position where the anchor opens, so they arrive only after
    /// the anchor's rows are appended.
    AfterAnchor {
        /// The earliest occurrence the column can read.
        occurrence: String,
        /// Its loop and position, and the anchor's.
        positions: Box<AnchorPositions>,
    },
    /// `pick` is given without `occurrence`.
    PickNeedsOccurrence {
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
    /// `phi` is `name` or `id` on a column that is not text.
    PhiNeedsText {
        /// The kind given: `name` or `id`.
        kind: &'static str,
        /// The column's type.
        found: ColumnKind,
    },
    /// `phi` is `date` on a column that is not a date.
    PhiNeedsDate {
        /// The column's type.
        found: ColumnKind,
    },
    /// `phi` is set on an element an envelope loop uses as a control number
    /// or count.
    PhiOnControlElement {
        /// The segment the column reads, e.g. `ST`.
        segment: String,
        /// The 1-based element position.
        element: usize,
    },
    /// `phi_seed` names a column the table does not have.
    UnknownPhiSeed {
        /// The column named.
        name: String,
    },
    /// `phi_seed` names a column that is not flagged `id`.
    PhiSeedNotId {
        /// The column named.
        name: String,
    },
}
