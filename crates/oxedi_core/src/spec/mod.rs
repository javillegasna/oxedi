//! Loop specifications: the data that tells the engine how a file is structured.
//!
//! A spec is plain JSON: a map of loops, each naming its parent, the segment
//! (and optional element conditions) that opens it, the occurrences of
//! segments it may hold (named, with position, usage, maximum repeat,
//! qualifier and own code lists) and an optional segment that closes it. An
//! optional `segments` section names and types the elements of each segment
//! id, wherever the segment appears. An optional `balancing` section lists
//! amounts that must add up inside each instance of a loop. An optional
//! `summaries` section names figures computed over the tables, and an
//! optional `default_table` names the table a reader shows when none is
//! named. The compiled spec holds index-based definitions, so the engine
//! never compares strings, and keeps no JSON.
//!
//! The schema is split by responsibility: `loops`, `occurrences`, `elements`
//! (an element, a segment, the automatic column names), `tables` and
//! `balance_rule` (a balancing rule and its text) hold the definitions;
//! `occurrence_index` the occurrence a segment takes in a loop; `id_map` the
//! map keyed by segment id; `schema` the columns of each table, known from the
//! spec alone; `unknown` the errors for a table or a column the spec does not
//! define; `render` the text of triggers and occurrence selectors in messages;
//! `version` the version a compiled spec declares and the choice of a spec by
//! it; `test_support` (test builds only) the small spec shared by the spec and
//! projector tests and the nested-loop spec shared by the engine and tree
//! tests.
//!
//! Loading reads a spec's JSON whole into its typed model, then compiles the
//! model into a [`Spec`]. The loader keeps the model
//! ([`LoadedSpec`]), patches it and compiles it again. Patches follow RFC 7386:
//! objects merge key by key, while arrays and scalars replace wholesale; see
//! [`LoadedSpec::merge_patch`] for what that means for a loop's occurrences.
//!
//! Loading is split by responsibility: `model` the spec as typed data and
//! `strict` the deserializer that reads it; `build` the construction of a
//! [`Spec`] as a pipeline of steps, from JSON with
//! [`Spec::from_json`](crate::spec::Spec::from_json), and why a version
//! declaration is rejected; `build_loops` the compilation of one loop and why
//! its control is rejected, `build_occurrences` that of its occurrences,
//! `segments` that of the segments, `balance` that of the balancing rules,
//! `summaries` that of the summaries and the default table, `compile` that of
//! the tables, `columns` that of one table column and `column_occurrence` the
//! reading of the occurrence a column names; `phi` the check of the columns
//! flagged as patient data and of the seed column; `error` the load error and
//! `error_text` its `source` and the parts of its text built from lists;
//! `occurrence_error`, `table_error`, `balance_error` and `summary_error` why
//! an occurrence, a table, a balancing rule or a summary is rejected, and
//! `table_chain` the table chains and declared occurrences a table error
//! quotes; `loaded` the spec as a loader keeps it, its model beside its
//! compiled form; `patch` the merge patch; `builtin` the registry of shipped
//! specs by transaction and version, and the built-in 835 shortcuts;
//! `vocabulary` the spec's closed words and the codes key, one table each.

mod balance_rule;
mod elements;
mod id_map;
mod loops;
mod occurrence_index;
mod occurrences;
mod render;
mod schema;
mod tables;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
mod unknown;
mod version;

mod balance;
mod balance_error;
mod build;
mod build_loops;
mod build_occurrences;
mod builtin;
mod column_occurrence;
mod columns;
mod compile;
mod error;
mod error_text;
mod loaded;
mod model;
mod occurrence_error;
mod patch;
mod phi;
mod segments;
mod strict;
mod summaries;
mod summary_error;
mod table_chain;
mod table_error;
mod vocabulary;

pub use balance_rule::{BalanceRule, BalanceTerm, Sign};
pub use elements::{ElementDef, ROW_COLUMN, SEGMENT_COLUMN, SegmentDef};
pub use loops::{Control, ControlCount, LoopDef, LoopId, Trigger};
pub use occurrence_index::OccurrenceId;
pub use occurrences::{OccurrenceDef, Qualifier, Usage};
pub use schema::{
    ColumnKind, ColumnSchema, TableSchema, require_column, spec_schema, table_columns, table_schema,
};
pub use tables::{ColumnSource, Repeat, Summary, TableDef};
pub use unknown::{UnknownColumn, UnknownTable};
pub use version::DeclaredVersion;

pub use balance_error::BalanceError;
pub use build::VersionError;
pub use build_loops::ControlError;
pub use builtin::{Builtin, UnknownVersion, Version};
pub use error::SpecError;
pub use loaded::LoadedSpec;
pub use model::{
    BalanceModel, ColumnModel, ControlModel, ElementModel, LoopModel, OccurrenceModel,
    QualifierModel, RepeatModel, SegmentModel, SpecModel, SummaryColumnModel, SummaryFieldModel,
    SummaryModel, SummaryRowModel, SummaryTableModel, TableModel, TermModel, TriggerModel,
    VersionModel,
};
pub use occurrence_error::OccurrenceError;
pub use patch::merge_patch;
pub use segments::ElementDefError;
pub use summary_error::SummaryDefError;
pub use table_error::{AnchorChains, AnchorPositions, LoopSegments, TableDefError};
pub use vocabulary::ElementPosition;

pub(crate) use balance_rule::render_terms;
pub(crate) use elements::rejects_code;
pub(crate) use id_map::IdMap;
pub(crate) use occurrence_index::OccurrenceIndex;
pub(crate) use occurrences::element_def;
pub(crate) use render::{render_selector, render_trigger};
pub(crate) use schema::column_type;
pub(crate) use version::chosen;

use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::segment::Segment;

/// A loaded, validated loop structure.
#[derive(Debug, Clone)]
pub struct Spec {
    pub(crate) name: String,
    pub(crate) loops: Vec<LoopDef>,
    pub(crate) occurrence_index: OccurrenceIndex,
    pub(crate) roots: Vec<LoopId>,
    pub(crate) segments: BTreeMap<Vec<u8>, SegmentDef>,
    pub(crate) tables: Vec<TableDef>,
    pub(crate) balancing: Vec<BalanceRule>,
    pub(crate) summaries: Vec<(String, Summary)>,
    pub(crate) default_table: Option<String>,
    pub(crate) version: Option<DeclaredVersion>,
}

impl Spec {
    /// The spec's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every loop, in spec order. Spec order is alphabetical by loop name
    /// (the JSON map is read into a `BTreeMap`), and it breaks ties between
    /// equally specific triggers.
    pub fn loops(&self) -> &[LoopDef] {
        &self.loops
    }

    /// The loop behind an id.
    pub fn get(&self, id: LoopId) -> &LoopDef {
        &self.loops[id.0]
    }

    /// The id of a loop by name.
    pub fn loop_id(&self, name: &str) -> Option<LoopId> {
        self.loops
            .iter()
            .position(|def| def.name == name)
            .map(LoopId)
    }

    /// The name of a loop by id.
    pub fn loop_name(&self, id: LoopId) -> &str {
        &self.loops[id.0].name
    }

    /// Top-level loops (no parent), in spec order.
    pub fn roots(&self) -> &[LoopId] {
        &self.roots
    }

    /// The element definitions of a segment id, if the spec has any.
    pub fn segment(&self, id: &[u8]) -> Option<&SegmentDef> {
        self.segments.get(id)
    }

    /// Every defined segment id with its definition, ordered by id.
    pub fn segments(&self) -> impl Iterator<Item = (&[u8], &SegmentDef)> {
        self.segments.iter().map(|(id, def)| (id.as_slice(), def))
    }

    /// The definition of an element, or of one of its components; `None`
    /// when the spec does not define it.
    pub fn element_def(
        &self,
        segment: &[u8],
        element: usize,
        component: Option<usize>,
    ) -> Option<&ElementDef> {
        occurrences::element_def(&self.segments, segment, element, component)
    }

    /// Whether any column of any table is flagged as patient data.
    pub fn masks_anything(&self) -> bool {
        self.tables
            .iter()
            .any(|table| table.phi.iter().any(Option::is_some))
    }

    /// Every table, in name order.
    pub fn tables(&self) -> &[TableDef] {
        &self.tables
    }

    /// The names of the tables, in the order of [`Spec::tables`].
    pub(crate) fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|table| table.name.clone()).collect()
    }

    /// The tables and loops, editable past the loader's checks, to build
    /// specs the loader refuses. It edits the loader's output without
    /// updating anything derived from it (loop children and segment lists,
    /// table parents and ancestors, the occurrence order): use it only for
    /// write plan tests, which read the edited parts directly.
    #[cfg(test)]
    pub(crate) fn parts_mut(&mut self) -> (&mut Vec<TableDef>, &mut Vec<LoopDef>) {
        (&mut self.tables, &mut self.loops)
    }

    /// A table by name.
    pub fn table(&self, name: &str) -> Option<&TableDef> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// A table by name; a name the spec does not define is an
    /// [`UnknownTable`] that lists the spec's tables.
    pub fn require_table(&self, name: &str) -> Result<&TableDef, UnknownTable> {
        self.table(name).ok_or_else(|| UnknownTable {
            name: name.to_owned(),
            tables: self.table_names(),
        })
    }

    /// Every balancing rule, in name order.
    pub fn balancing(&self) -> &[BalanceRule] {
        &self.balancing
    }

    /// Every summary with its name, in name order.
    pub fn summaries(&self) -> &[(String, Summary)] {
        &self.summaries
    }

    /// The summary `name`, when the spec declares it.
    pub fn summary(&self, name: &str) -> Option<&Summary> {
        self.summaries
            .iter()
            .find_map(|(declared, summary)| (declared == name).then_some(summary))
    }

    /// The table a reader shows when the caller names none, when the spec
    /// declares one (`default_table`); it is always one of [`Self::tables`].
    pub fn default_table(&self) -> Option<&str> {
        self.default_table.as_deref()
    }

    /// Children of a loop, or the top-level loops for `None`.
    pub fn children(&self, parent: Option<LoopId>) -> &[LoopId] {
        match parent {
            Some(id) => &self.loops[id.0].children,
            None => &self.roots,
        }
    }

    /// Ancestors of a loop, root-most first, excluding the loop itself.
    pub fn ancestors(&self, id: LoopId) -> Vec<LoopId> {
        let mut chain = Vec::new();
        self.ancestors_into(id, &mut chain);
        chain
    }

    /// Like [`ancestors`](Spec::ancestors), writing the chain into `chain`
    /// after clearing it, so a caller can reuse one buffer.
    pub fn ancestors_into(&self, id: LoopId, chain: &mut Vec<LoopId>) {
        chain.clear();
        let mut current = self.loops[id.0].parent;
        while let Some(parent) = current {
            chain.push(parent);
            current = self.loops[parent.0].parent;
        }
        chain.reverse();
    }

    /// The child of `parent` that `segment` triggers, preferring the trigger
    /// with the most conditions; ties go to spec order.
    pub fn matching_child(&self, parent: Option<LoopId>, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match(self.children(parent).iter().copied(), segment)
    }

    /// Any loop that `segment` triggers, wherever it sits in the structure.
    pub fn matching_any(&self, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match((0..self.loops.len()).map(LoopId), segment)
    }

    fn best_match(
        &self,
        candidates: impl Iterator<Item = LoopId>,
        segment: &Segment<'_>,
    ) -> Option<LoopId> {
        candidates
            .filter(|&id| self.loops[id.0].trigger.matches(segment))
            .min_by_key(|&id| Reverse(self.loops[id.0].trigger.conditions.len()))
    }
}
