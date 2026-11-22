//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `loops` holds the loops
//! and their occurrences, `segments` the segments and their elements,
//! `tables` the tables and their columns, and `rules` the version
//! declaration, the balancing rules and the operations of a summary; which
//! read checks what is on [`SpecModel`]. `declare` holds the macro that
//! emits each object's model.

mod declare;
mod loops;
mod rules;
mod segments;
mod tables;

use std::collections::BTreeMap;

use serde::de;
use serde::{Deserialize, Deserializer};

use super::SpecError;
use super::segments::{POSITION_RULE, parse_position};
use super::strict::StrictValue;
use crate::message::render_key;
use crate::spec::Spec;
use declare::spec_object;

pub use loops::{ControlModel, LoopModel, OccurrenceModel, QualifierModel, TriggerModel};
pub use rules::{
    BalanceModel, SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel,
    SummaryTableModel, TermModel, VersionModel,
};
pub use segments::{ElementModel, SegmentModel};
pub use tables::{ColumnModel, RepeatModel, TableModel};

// Read only by the model test that holds the list to the keys the model writes.
#[cfg(test)]
pub(crate) use segments::ELEMENT_MODEL_KEYS;

spec_object! {
    /// A whole spec, as the spec writes it.
    ///
    /// Read it with [`SpecModel::from_json`], which reads the whole spec strictly,
    /// and check the rest with [`SpecModel::compile`]. Reading a model,
    /// or any `*Model`, with serde directly
    /// (`serde_json::from_value::<SummaryModel>(..)`) checks only its shape, its
    /// position keys and a summary's one operation: it accepts a table or a column
    /// that does not exist and an empty `fields` list. Compile such a model
    /// before using it.
    ///
    /// A model writes no default and no `null`: a key equal to its default and
    /// an absent value are left out.
    ///
    /// ```
    /// # use oxedi_core::spec::Spec;
    /// # use oxedi_core::spec_load::{SpecModel, SummaryModel};
    /// # use serde_json::json;
    /// let mut model = SpecModel::from_json(Spec::BUILTIN_835_JSON)?;
    /// assert_eq!(model.compile()?.name(), "835");
    /// let summary = serde_json::from_value::<SummaryModel>(json!({"count":{"table":"nope"}}))?;
    /// model.summaries.insert("ghost".into(), summary);
    /// assert!(model.compile().is_err());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    SpecModel, "a spec object with \"name\" and \"loops\"";
    /// The spec's name.
    name: String, required;
    /// The element that declares the version a file is written in, when the
    /// spec declares one.
    #[serde(default, deserialize_with = "present")]
    version: Option<VersionModel>, optional;
    /// The loops, by name.
    loops: BTreeMap<String, LoopModel>, required;
    /// The tables, by name.
    tables: BTreeMap<String, TableModel>, default;
    /// The balancing rules, by name.
    balancing: BTreeMap<String, BalanceModel>, default;
    /// The segments whose elements the spec names and types, by segment id.
    segments: BTreeMap<String, SegmentModel>, default;
    /// The summaries, by name.
    summaries: BTreeMap<String, SummaryModel>, default;
    /// The table a reader shows when none is named, when the spec names one.
    #[serde(default, deserialize_with = "present")]
    default_table: Option<String>, optional;
}

impl SpecModel {
    /// Reads a spec's JSON text into its model, one strict read of the whole
    /// spec; it does not check the rules that compile checks. Unparsable
    /// text is [`SpecError::Json`], a value the model refuses
    /// [`SpecError::Format`] at its path.
    pub fn from_json(json: &str) -> Result<SpecModel, SpecError> {
        let source: serde_json::Value = serde_json::from_str(json).map_err(SpecError::Json)?;
        SpecModel::from_value(&source)
    }

    /// Compiles the model into the spec the engine runs; it fails with the
    /// errors [`Spec::from_json`] reports for the same spec.
    pub fn compile(&self) -> Result<Spec, SpecError> {
        Spec::build(self)
    }

    /// Reads a parsed spec into its model in one pass; the first value the
    /// model refuses, in key order, fails with [`SpecError::Format`] at its
    /// path from the spec's root (`.` for the root itself). An array where it
    /// has an object is refused.
    pub(in crate::spec_load) fn from_value(
        source: &serde_json::Value,
    ) -> Result<SpecModel, SpecError> {
        serde_path_to_error::deserialize(StrictValue(source)).map_err(|err| SpecError::Format {
            path: root_path(err.path()),
            source: err.into_inner(),
        })
    }

    /// The model as pretty-printed JSON text, as
    /// [`crate::spec_load::LoadedSpec::to_json`] prints a spec.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// A path as an error writes it: keys joined by `.`, each through `render_key`
// so a key holding `.` stays one key; a list item as `[i]`.
fn render_path(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut text = String::new();
    for segment in path {
        match segment {
            Segment::Map { key } | Segment::Enum { variant: key } => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(&render_key(key));
            }
            Segment::Seq { index } => text.push_str(&format!("[{index}]")),
            Segment::Unknown => text.push_str(".?"),
        }
    }
    text
}

// The path of a refused value from the spec's root; `.` for the root itself.
fn root_path(path: &serde_path_to_error::Path) -> String {
    match render_path(path) {
        path if path.is_empty() => ".".to_owned(),
        path => path,
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

// A key that holds a value when written: `null` is refused as any other
// value of the wrong type, with the text of `T`'s own read, instead of
// reading as an absent key.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
