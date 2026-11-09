//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `loops` holds the loops
//! and their occurrences, `segments` the segments and their elements,
//! `tables` the tables and their columns, and `rules` the version
//! declaration, the balancing rules and the operations of a summary; which
//! read checks what is on [`SpecModel`].

mod declare;
mod loops;
mod rules;
mod segments;
mod tables;

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::{Spec, SpecError};

pub use loops::{ControlModel, LoopModel, OccurrenceModel, QualifierModel, TriggerModel};
pub use rules::{
    BalanceModel, SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel,
    SummaryTableModel, TermModel, VersionModel,
};
pub use segments::{ElementModel, SegmentModel};
pub use tables::{ColumnModel, RepeatModel, TableModel};

pub(in crate::spec) use loops::{CONTROL, QUALIFIER, RawControl, RawQualifier};
pub(in crate::spec) use rules::{
    BALANCE, RawBalance, RawSummaryColumn, RawSummaryField, RawSummaryRow, RawSummaryTable,
    RawTerm, RawVersion, SUMMARY_COLUMN, SUMMARY_ROW, SUMMARY_TABLE, VERSION,
};
pub(in crate::spec) use tables::{REPEAT, RawRepeat};

/// A whole spec, as the spec writes it.
///
/// Read it with `Spec::from_json(..)?.model()`, which checks every loader
/// rule. Reading a model, or any `*Model`, with serde directly
/// (`serde_json::from_value::<SummaryModel>(..)`) checks only its shape: it
/// accepts a table or a column that does not exist and an empty `fields`
/// list. Load such a model with [`Spec::from_model`] before using it.
///
/// A model writes no default and no `null`: a key equal to its default and
/// an absent value are left out.
///
/// ```
/// # use oxedi_core::spec::{Spec, SummaryModel};
/// # use serde_json::json;
/// let mut model = Spec::builtin_835().model()?;
/// assert_eq!(Spec::from_model(&model)?.name(), "835");
/// let summary = serde_json::from_value::<SummaryModel>(json!({"count":{"table":"nope"}}))?;
/// model.summaries.insert("ghost".into(), summary);
/// assert!(Spec::from_model(&model).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a spec object with \"name\" and \"loops\""
)]
pub struct SpecModel {
    /// The spec's name.
    pub name: String,
    /// The element that declares the version a file is written in, when the
    /// spec declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionModel>,
    /// The loops, by name.
    pub loops: BTreeMap<String, LoopModel>,
    /// The segments whose elements the spec names and types, by segment id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub segments: BTreeMap<String, SegmentModel>,
    /// The tables, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tables: BTreeMap<String, TableModel>,
    /// The balancing rules, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub balancing: BTreeMap<String, BalanceModel>,
    /// The summaries, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub summaries: BTreeMap<String, SummaryModel>,
    /// The table a reader shows when none is named, when the spec names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_table: Option<String>,
}

impl SpecModel {
    /// The model as pretty-printed JSON text, as [`Spec::to_json`] prints a
    /// spec.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Spec {
    /// The spec as typed data, read from the JSON the loader accepted, so
    /// every loader rule holds for it.
    ///
    /// A failure means the loader and the model disagree on what a spec may
    /// hold, a bug in oxedi: [`SpecError::ModelMismatch`].
    pub fn model(&self) -> Result<SpecModel, SpecError> {
        SpecModel::deserialize(&self.source).map_err(|source| SpecError::ModelMismatch {
            spec_name: self.name.clone(),
            source,
        })
    }

    /// Loads a spec from its model: the model is written to JSON and loaded
    /// through the same steps as [`Spec::from_json`], so it fails with the
    /// same errors.
    pub fn from_model(model: &SpecModel) -> Result<Spec, SpecError> {
        let source = serde_json::to_value(model).map_err(SpecError::Json)?;
        Spec::from_value(source)
    }
}

// A key equal to its default is not written.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

// A key written `null` reads as its default, as an absent key does.
fn null_is_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    deserializer: D,
) -> Result<T, D::Error> {
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}
