//! The deserialization shapes of a spec's JSON entries that no declaration
//! emits: the element, the pick and the summary. The others are emitted with
//! their model in `model`, and re-exported here.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "an element object")]
pub(super) struct RawElement {
    pub(super) name: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) required: bool,
    pub(super) min: Option<usize>,
    pub(super) max: Option<usize>,
    pub(super) scale: Option<u8>,
    pub(super) codes: Option<Vec<String>>,
    #[serde(default)]
    pub(super) composite: BTreeMap<String, RawElement>,
}

// A pick is a name (`first`, `last`) or a 1-based position; which names are
// valid is checked when the column compiles, so the error can show the value.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged, expecting = "a pick")]
pub(super) enum RawPick {
    Nth(usize),
    Named(String),
}

impl RawPick {
    /// The pick as the spec writes it, in JSON.
    pub(super) fn written(&self) -> String {
        match self {
            RawPick::Nth(nth) => nth.to_string(),
            RawPick::Named(name) => serde_json::to_string(name).unwrap_or_default(),
        }
    }
}

// A summary holds exactly one operation; which ones it holds is checked when
// it compiles, so the error can list them.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary object")]
pub(super) struct RawSummary {
    pub(super) count: Option<RawSummaryTable>,
    pub(super) distinct: Option<RawSummaryColumn>,
    pub(super) sum: Option<RawSummaryColumn>,
    pub(super) row: Option<RawSummaryRow>,
}

pub(super) use super::model::{
    RawBalance, RawColumn, RawControl, RawLoop, RawOccurrence, RawQualifier, RawSegment, RawSpec,
    RawSummaryColumn, RawSummaryRow, RawSummaryTable, RawTable, RawTerm, RawVersion,
};
