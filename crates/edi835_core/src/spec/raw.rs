//! The deserialization shapes of a spec's JSON entries.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::IgnoredAny;

// The loop, segment and table sections are only checked for their outer shape
// here; each entry is then deserialized on its own, straight from the spec's
// JSON, so a schema error can name the entry it came from.
#[derive(Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a spec object with \"name\" and \"loops\""
)]
pub(super) struct RawSpec {
    pub(super) name: String,
    pub(super) loops: BTreeMap<String, IgnoredAny>,
    #[serde(default, rename = "segments")]
    pub(super) _segments: BTreeMap<String, IgnoredAny>,
    #[serde(default, rename = "tables")]
    pub(super) _tables: BTreeMap<String, IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a loop object")]
pub(super) struct RawLoop {
    pub(super) parent: Option<String>,
    pub(super) trigger: RawTrigger,
    #[serde(default)]
    pub(super) segments: Vec<String>,
    pub(super) end: Option<String>,
    pub(super) control: Option<RawControl>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a control object")]
pub(super) struct RawControl {
    pub(super) opener_element: usize,
    pub(super) closer_element: usize,
    pub(super) count_element: usize,
    pub(super) count: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a trigger object")]
pub(super) struct RawTrigger {
    pub(super) segment: String,
    #[serde(default, rename = "where")]
    pub(super) conditions: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a segment object")]
pub(super) struct RawSegment {
    #[serde(default)]
    pub(super) elements: BTreeMap<String, RawElement>,
}

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
    #[serde(default)]
    pub(super) composite: BTreeMap<String, RawElement>,
}

// Columns are checked for their outer shape here and deserialized one by one,
// so a schema error can name the column.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a table object")]
pub(super) struct RawTable {
    pub(super) loops: Vec<String>,
    #[serde(rename = "ref")]
    pub(super) reference: Option<String>,
    pub(super) segment: Option<String>,
    pub(super) repeat: Option<RawRepeat>,
    #[serde(default)]
    pub(super) columns: BTreeMap<String, IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a repeat object")]
pub(super) struct RawRepeat {
    pub(super) from: usize,
    pub(super) step: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a column object")]
pub(super) struct RawColumn {
    #[serde(rename = "loop")]
    pub(super) loop_name: Option<String>,
    pub(super) segment: Option<String>,
    #[serde(default, rename = "where")]
    pub(super) conditions: BTreeMap<String, String>,
    pub(super) element: Option<usize>,
    pub(super) component: Option<usize>,
    pub(super) group_element: Option<usize>,
    #[serde(default)]
    pub(super) segment_index: bool,
}
