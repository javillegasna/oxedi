//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `loops` holds the loops
//! and their occurrences, `segments` the segments and their elements,
//! `tables` the tables and their columns, and `rules` the version
//! declaration, the balancing rules and the operations of a summary; which
//! read checks what is on [`SpecModel`]. `declare` holds the macro that
//! emits each object's model, its raw struct and its schema table.

mod declare;
mod loops;
mod rules;
mod segments;
mod tables;

use std::collections::BTreeMap;

use serde::de::{self, IgnoredAny};
use serde::{Deserialize, Deserializer};

use super::sections::{SUMMARY, TEXT};
use super::segments::{POSITION_RULE, parse_position};
use super::shape::Field;
use super::{Spec, SpecError};
use declare::spec_object;

pub use loops::{ControlModel, LoopModel, OccurrenceModel, QualifierModel, TriggerModel};
pub use rules::{
    BalanceModel, SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel,
    SummaryTableModel, TermModel, VersionModel,
};
pub use segments::{ElementModel, SegmentModel};
pub use tables::{ColumnModel, RepeatModel, TableModel};

pub(in crate::spec) use loops::{LOOP, RawControl, RawLoop, RawOccurrence, RawQualifier};
pub(in crate::spec) use rules::{
    BALANCE, RawBalance, RawSummaryColumn, RawSummaryRow, RawSummaryTable, RawTerm, RawVersion,
    SUMMARY_COLUMN, SUMMARY_ROW, SUMMARY_TABLE, VERSION,
};
// Read only by the sync test, which holds the list to the keys the model writes.
#[cfg(test)]
pub(in crate::spec) use segments::ELEMENT_MODEL_KEYS;
pub(in crate::spec) use segments::{RawSegment, SEGMENT};
pub(in crate::spec) use tables::{RawColumn, RawTable, TABLE};

spec_object! {
    /// A whole spec, as the spec writes it.
    ///
    /// Read it with `Spec::from_json(..)?.model()`, which checks every loader
    /// rule. Reading a model, or any `*Model`, with serde directly
    /// (`serde_json::from_value::<SummaryModel>(..)`) checks only its shape and
    /// its position keys: it accepts a table or a column that does not exist and
    /// an empty `fields` list. Load such a model with [`Spec::from_model`] before using it.
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
    SpecModel / RawSpec, ROOT, "a spec object with \"name\" and \"loops\"";
    /// The spec's name.
    name: String, TEXT, required;
    /// The element that declares the version a file is written in, when the
    /// spec declares one.
    version: Option<VersionModel>, Field::Object(&VERSION), optional;
    /// The loops, by name.
    // The loader checks the loops' outer shape and deserializes them one by
    // one, so a schema error can name the loop.
    loops: BTreeMap<String, LoopModel> | BTreeMap<String, IgnoredAny>,
        Field::Map(&Field::Object(&LOOP)), required;
    /// The tables, by name.
    tables: BTreeMap<String, TableModel> | #[allow(dead_code, reason = "each section is read from the source, entry by entry")] BTreeMap<String, IgnoredAny>,
        Field::Map(&Field::Object(&TABLE)), default;
    /// The balancing rules, by name.
    balancing: BTreeMap<String, BalanceModel> | #[allow(dead_code, reason = "each section is read from the source, entry by entry")] BTreeMap<String, IgnoredAny>,
        Field::Map(&Field::Object(&BALANCE)), default;
    /// The segments whose elements the spec names and types, by segment id.
    segments: BTreeMap<String, SegmentModel> | #[allow(dead_code, reason = "each section is read from the source, entry by entry")] BTreeMap<String, IgnoredAny>,
        Field::Map(&Field::Object(&SEGMENT)), default;
    /// The summaries, by name.
    summaries: BTreeMap<String, SummaryModel> | #[allow(dead_code, reason = "each section is read from the source, entry by entry")] BTreeMap<String, IgnoredAny>,
        Field::Map(&Field::Object(&SUMMARY)), default;
    /// The table a reader shows when none is named, when the spec names one.
    default_table: Option<String>, TEXT, optional;
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

// A map keyed by 1-based position, read through the loader's rule: the first
// key (in key order) that is not a canonical position refuses the whole map,
// naming it. The map is read whole first, a cold path.
fn positions<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
    deserializer: D,
    map: &str,
) -> Result<BTreeMap<usize, V>, D::Error> {
    BTreeMap::<String, V>::deserialize(deserializer)?
        .into_iter()
        .map(|(key, value)| match parse_position(&key) {
            Some(position) => Ok((position, value)),
            None => Err(de::Error::custom(format!(
                "{map} key {key:?} is not a position: {POSITION_RULE}"
            ))),
        })
        .collect()
}

// A key written `null` reads as its default, as an absent key does.
fn null_is_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    deserializer: D,
) -> Result<T, D::Error> {
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}
