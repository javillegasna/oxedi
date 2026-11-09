//! The loops of a spec and their occurrences, as the spec writes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::declare::spec_object;
use super::{is_default, null_is_default};
use crate::spec::sections::{COUNT, TEXT};
use crate::spec::{ControlCount, ElementPosition, Usage};

/// A loop: the segment that opens it, where it sits and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a loop object")]
pub struct LoopModel {
    /// The loop that holds this one, when it is not a top loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The segment, and the element values, that open an instance.
    pub trigger: TriggerModel,
    /// The occurrences an instance holds, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub occurrences: BTreeMap<String, OccurrenceModel>,
    /// The most instances the parent holds, when the spec bounds them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// Whether every instance of the parent holds one.
    #[serde(default, deserialize_with = "null_is_default")]
    #[serde(skip_serializing_if = "is_default")]
    pub usage: Usage,
    /// The segment that closes an instance, when one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// The counts the end segment checks, when it checks any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlModel>,
}

/// The segment that opens a loop's instance and the element values it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a trigger object")]
pub struct TriggerModel {
    /// The segment.
    pub segment: String,
    /// The value each element must hold, by 1-based element position.
    #[serde(default, rename = "where", skip_serializing_if = "BTreeMap::is_empty")]
    pub conditions: BTreeMap<usize, String>,
}

/// One place a segment takes inside a loop's instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "an occurrence object")]
pub struct OccurrenceModel {
    /// The segment.
    pub segment: String,
    /// The order of the occurrence among those of its loop.
    pub pos: usize,
    /// Whether every instance of the loop holds it.
    #[serde(default, deserialize_with = "null_is_default")]
    #[serde(skip_serializing_if = "is_default")]
    pub usage: Usage,
    /// The most times it repeats in one instance, when the spec bounds it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// What tells it apart from the other occurrences of its segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<QualifierModel>,
    /// The codes an element or component may hold here, by position.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub codes: BTreeMap<ElementPosition, Vec<String>>,
}

/// The element (or component) and the codes that tell an occurrence apart
/// from the other occurrences of its segment in the same loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a qualifier object")]
pub struct QualifierModel {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position inside the element, when the qualifier is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<usize>,
    /// The codes the element holds in this occurrence.
    pub codes: Vec<String>,
}

spec_object! {
    /// The elements of a loop's end segment that check the instance it closes.
    ControlModel / RawControl, CONTROL, "a control object";
    /// 1-based element position of the control number in the opening segment.
    opener_element: usize, COUNT, required;
    /// 1-based element position of the control number in the end segment.
    closer_element: usize, COUNT, required;
    /// 1-based element position of the count in the end segment.
    count_element: usize, COUNT, required;
    /// What the count counts.
    count: ControlCount | String, TEXT, required;
}
