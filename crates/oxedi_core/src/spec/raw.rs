//! The deserialization shapes of a spec's JSON entries that no declaration
//! emits: the element, the pick and the summary. The others are emitted with
//! their model in `model`, and re-exported here.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::table_error::TableDefError;

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
// It is read through one visitor so every value that is not a word or a whole
// number fails with the `BadPick` text.
#[derive(Debug, Clone)]
pub(super) enum RawPick {
    Nth(usize),
    Named(String),
}

impl<'de> Deserialize<'de> for RawPick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PickVisitor)
    }
}

struct PickVisitor;

impl PickVisitor {
    fn refuse<E: de::Error>(written: impl Into<String>) -> E {
        E::custom(TableDefError::BadPick {
            written: written.into(),
        })
    }
}

impl<'de> Visitor<'de> for PickVisitor {
    type Value = RawPick;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"first\", \"last\" or a 1-based position")
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<RawPick, E> {
        usize::try_from(v)
            .map(RawPick::Nth)
            .map_err(|_| Self::refuse(v.to_string()))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<RawPick, E> {
        usize::try_from(v)
            .map(RawPick::Nth)
            .map_err(|_| Self::refuse(v.to_string()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<RawPick, E> {
        Err(Self::refuse(format!("{v}")))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<RawPick, E> {
        Err(Self::refuse(v.to_string()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<RawPick, E> {
        Err(Self::refuse("null"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<RawPick, E> {
        Ok(RawPick::Named(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<RawPick, E> {
        Ok(RawPick::Named(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<RawPick, A::Error> {
        Err(Self::refuse("an array"))
    }

    fn visit_map<A: MapAccess<'de>>(self, _: A) -> Result<RawPick, A::Error> {
        Err(Self::refuse("an object"))
    }
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
