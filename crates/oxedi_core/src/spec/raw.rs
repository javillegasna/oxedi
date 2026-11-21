//! The deserialization shapes of a spec's JSON entries that no declaration
//! emits: the element and the summary. The others are emitted with
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
    RawBalance, RawSegment, RawSpec, RawSummaryColumn, RawSummaryRow, RawSummaryTable, RawTerm,
    RawVersion,
};
