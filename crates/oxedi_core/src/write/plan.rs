//! The write plan: for every loop, where its instances come from, and for
//! every segment it writes, which column or code gives each element.

#[cfg(doc)]
use crate::spec::{Spec, TableDef};

/// Where the instances of a loop come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instances {
    /// The loop is not written.
    Absent,
    /// One instance per envelope: the caller's envelope gives it.
    Envelope,
    /// One instance per row of the table anchored on the loop (an index into
    /// [`Spec::tables`]).
    Rows {
        /// The table.
        table: usize,
    },
    /// At most one instance per row of `table`, inside the row's anchor
    /// instance: the table's columns read the loop below its anchor.
    Inside {
        /// The table.
        table: usize,
    },
    /// One instance per run of consecutive rows of `table` (anchored below
    /// the loop) whose columns that read the loop hold the same values.
    Groups {
        /// The table.
        table: usize,
    },
    /// One instance around each run of instances of the written loops inside
    /// it; no column gives it values of its own.
    Implied,
}

/// Where one segment's values come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentSource {
    /// An envelope segment: the caller's envelope and the writer's counts.
    Envelope,
    /// The `nth` (from 1) segment of the occurrence, filled from one row of
    /// `table`.
    Row {
        /// The table.
        table: usize,
        /// Which repeat of the occurrence, counting from 1.
        nth: usize,
    },
    /// One segment per segment of a table anchored on the segment: the rows
    /// that share a segment index are its element groups.
    Repeat {
        /// The table.
        table: usize,
    },
}

/// Where one element (or component) value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSource {
    /// A column of the segment's table, by index into its
    /// [`TableDef::columns`].
    Column {
        /// The column.
        column: usize,
    },
    /// A column of a table with a repeat, written at `from + k * step +
    /// offset` for the `k`-th row of the segment.
    Group {
        /// The column.
        column: usize,
        /// 0-based position inside the group.
        offset: usize,
    },
    /// A code the spec fixes: the occurrence's single qualifier code, a
    /// `where` value, or the one code a required element allows.
    Code(Vec<u8>),
}

/// One element (or component) a segment writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementPlan {
    /// 1-based element position; for a group, the position in the first group.
    pub element: usize,
    /// 1-based component position, for one component of a composite.
    pub component: Option<usize>,
    /// Where its value comes from.
    pub value: ValueSource,
}

/// One segment a loop instance writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPlan {
    /// Index of the occurrence in its loop's occurrences.
    pub occurrence: usize,
    /// Where its values come from.
    pub source: SegmentSource,
    /// Its elements, by position then component.
    pub elements: Vec<ElementPlan>,
}

/// How one loop is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopPlan {
    /// Where its instances come from.
    pub instances: Instances,
    /// The segments each instance may write, by occurrence position (then
    /// occurrence, codes and repeat).
    pub segments: Vec<SegmentPlan>,
}

/// How a spec's tables are written back into a file: one [`LoopPlan`] per
/// loop, by [`LoopId`](crate::spec::LoopId) index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritePlan {
    /// The plan of every loop, in [`Spec::loops`] order.
    pub loops: Vec<LoopPlan>,
}
