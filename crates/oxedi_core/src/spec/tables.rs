//! Table definitions: repeats and column sources.

#[cfg(doc)]
use super::Spec;
use super::UnknownColumn;
use super::loops::LoopId;
use crate::vocabulary::{Phi, Pick};

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
    /// The kind of patient data each column of `columns` holds, parallel to
    /// it; read through [`TableDef::phi`].
    pub(super) phi: Vec<Option<Phi>>,
    /// The index in `columns` of the column whose value seeds the row's date
    /// shift; read through [`TableDef::phi_seed`].
    pub(super) phi_seed: Option<usize>,
}

impl TableDef {
    /// The source of the declared column `name`; `None` for a name the table
    /// does not declare, including the automatic `row`, `segment` and
    /// reference columns.
    pub fn column(&self, name: &str) -> Option<&ColumnSource> {
        self.columns
            .iter()
            .find_map(|(column, source)| (column == name).then_some(source))
    }

    /// The source of the declared column `name`; a name the table does not
    /// declare is an [`UnknownColumn`] that lists the declared columns, in the
    /// order [`Self::columns`] holds them.
    pub fn require_column(&self, name: &str) -> Result<&ColumnSource, UnknownColumn> {
        self.column(name).ok_or_else(|| UnknownColumn {
            table: self.name.clone(),
            column: name.to_owned(),
            columns: self
                .columns
                .iter()
                .map(|(column, _)| column.clone())
                .collect(),
        })
    }

    /// The kind of patient data column `column_index` of [`Self::columns`]
    /// holds, or `None` when it holds none.
    pub fn phi(&self, column_index: usize) -> Option<Phi> {
        self.phi.get(column_index).copied().flatten()
    }

    /// The index in [`Self::columns`] of the column whose value seeds the
    /// date shift of the table's rows.
    pub fn phi_seed(&self) -> Option<usize> {
        self.phi_seed
    }
}

/// What a summary computes over the tables of a parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Summary {
    /// The number of rows of the table.
    Count {
        /// The table.
        table: String,
    },
    /// The number of distinct non-null values of the column, compared as text.
    Distinct {
        /// The table.
        table: String,
        /// The column.
        column: String,
    },
    /// The sum of a decimal column; null cells are skipped.
    Sum {
        /// The table.
        table: String,
        /// The column, which reads a decimal (`R`) element.
        column: String,
    },
    /// Named fields of the one row of the table.
    Row {
        /// The table.
        table: String,
        /// `(name, column)` pairs, in the order written.
        fields: Vec<(String, String)>,
    },
}
