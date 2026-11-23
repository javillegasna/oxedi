//! Loading a spec: the typed model, its compilation into the schema of
//! [`crate::spec`], patches, the built-in specs and why a spec is rejected.
//! The schema never uses this module.
//!
//! Loading reads a spec's JSON whole into its typed model, then compiles the
//! model into a [`Spec`](crate::spec::Spec). The loader keeps the model
//! ([`LoadedSpec`]), patches it and compiles it again. Patches follow RFC
//! 7386: objects merge key by key, while arrays and scalars replace wholesale;
//! see [`LoadedSpec::merge_patch`] for what that means for a loop's
//! occurrences.
//!
//! Loading is split by responsibility: `model` the spec as typed data and
//! `strict` the deserializer that reads it; `build` the construction of a
//! [`Spec`](crate::spec::Spec) as a pipeline of steps, from JSON with
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
//! `vocabulary` the spec's closed words and the codes key, one table each;
//! `package` the layers of a package and its identity.

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
mod package;
mod patch;
mod phi;
mod segments;
mod strict;
mod summaries;
mod summary_error;
mod table_chain;
mod table_error;
#[cfg(test)]
mod tests;
mod vocabulary;

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
pub use package::{Guide, IdError, IdPart, Layer, PackageId, Release, Scope, TransactionSet};
pub use patch::merge_patch;
pub use segments::ElementDefError;
pub use summary_error::SummaryDefError;
pub use table_error::{AnchorChains, AnchorPositions, LoopSegments, TableDefError};
pub use vocabulary::ElementPosition;
