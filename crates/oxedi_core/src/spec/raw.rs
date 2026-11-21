//! The deserialization shapes of a spec's JSON entries that no declaration
//! emits: the element and the summary. The others are emitted with
//! their model in `model`, and re-exported here.

use serde::Deserialize;

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
    RawBalance, RawSpec, RawSummaryColumn, RawSummaryRow, RawSummaryTable, RawTerm, RawVersion,
};
