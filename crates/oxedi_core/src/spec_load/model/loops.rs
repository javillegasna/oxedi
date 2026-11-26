//! The loops of a spec and their occurrences, as the spec writes them.

use std::collections::BTreeMap;

use serde::Deserializer;

use super::declare::spec_object;
use super::{positions, present};
use crate::spec::{ControlCount, EnvelopeField, Usage};
use crate::spec_load::ElementPosition;

spec_object! {
    /// A loop: the segment that opens it, where it sits and what it holds.
    LoopModel, "a loop object";
    /// The loop that holds this one, when it is not a top loop.
    parent: Option<String>, optional;
    /// The segment that closes an instance, when one does.
    end: Option<String>, optional;
    /// The most instances the parent holds, when the spec bounds them.
    max: Option<usize>, optional;
    /// Whether every instance of the parent holds one.
    usage: Usage, default_or_null;
    /// The occurrences an instance holds, by name.
    occurrences: BTreeMap<String, OccurrenceModel>, default;
    /// The segment, and the element values, that open an instance.
    trigger: TriggerModel, required;
    /// The counts the end segment checks, when it checks any.
    #[serde(default, deserialize_with = "present")]
    control: Option<ControlModel>, optional;
}

spec_object! {
    /// The segment that opens a loop's instance and the element values it holds.
    TriggerModel, "a trigger object";
    /// The segment.
    segment: String, required;
    /// The value each element must hold, by 1-based element position.
    #[serde(deserialize_with = "trigger_where")]
    conditions as "where": BTreeMap<usize, String>, default;
}

fn trigger_where<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, String>, D::Error> {
    positions(deserializer, "trigger \"where\"")
}

spec_object! {
    /// One place a segment takes inside a loop's instance.
    OccurrenceModel, "an occurrence object";
    /// The segment.
    segment: String, required;
    /// The order of the occurrence among those of its loop.
    pos: usize, required;
    /// Whether every instance of the loop holds it.
    usage: Usage, default_or_null;
    /// The most times it repeats in one instance, when the spec bounds it.
    max: Option<usize>, optional;
    /// What tells it apart from the other occurrences of its segment.
    #[serde(default, deserialize_with = "present")]
    qualifier: Option<QualifierModel>, optional;
    /// How the occurrence narrows its segment's elements and components, by
    /// position.
    elements: BTreeMap<ElementPosition, ElementEntryModel>, default;
}

spec_object! {
    /// How one occurrence narrows an element or component of its segment. An
    /// empty entry `{}` is written back as it is.
    ElementEntryModel, "an element entry object";
    /// Whether the place must hold a value here; the element's when absent.
    usage: Option<Usage>, optional;
    /// The shortest length here; the element's when absent.
    min: Option<usize>, optional;
    /// The longest length here; the element's when absent.
    max: Option<usize>, optional;
    /// The codes the place may hold here; the element's list when absent.
    #[serde(default, deserialize_with = "present")]
    codes: Option<Vec<String>>, optional;
}

spec_object! {
    /// The element (or component) and the codes that tell an occurrence apart
    /// from the other occurrences of its segment in the same loop.
    QualifierModel, "a qualifier object";
    /// 1-based element position.
    element: usize, required;
    /// 1-based component position inside the element, when the qualifier is one.
    component: Option<usize>, optional;
    /// The codes the element holds in this occurrence.
    codes: Vec<String>, required;
}

spec_object! {
    /// The elements of a loop's end segment that check the instance it closes.
    ControlModel, "a control object";
    /// 1-based element position of the control number in the opening segment.
    opener_element: usize, required;
    /// 1-based element position of the control number in the end segment.
    closer_element: usize, required;
    /// 1-based element position of the count in the end segment.
    count_element: usize, required;
    /// What the count counts.
    count: ControlCount, required;
    /// The caller's field each element of the trigger carries, by 1-based
    /// element position.
    #[serde(deserialize_with = "control_fields")]
    fields: BTreeMap<usize, EnvelopeField>, default;
}

fn control_fields<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, EnvelopeField>, D::Error> {
    positions(deserializer, "control \"fields\"")
}
