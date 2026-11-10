//! The loops of a spec and their occurrences, as the spec writes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

use super::declare::spec_object;
use super::positions;
use crate::spec::sections::{COUNT, TEXT, TEXTS, nullable};
use crate::spec::shape::{Field, Leaf};
use crate::spec::{ControlCount, ElementPosition, Usage};

spec_object! {
    /// A loop: the segment that opens it, where it sits and what it holds.
    LoopModel / RawLoop, LOOP, "a loop object";
    /// The loop that holds this one, when it is not a top loop.
    parent: Option<String>, nullable(Leaf::Text), optional;
    /// The segment that closes an instance, when one does.
    end: Option<String>, nullable(Leaf::Text), optional;
    /// The most instances the parent holds, when the spec bounds them.
    max: Option<usize>, nullable(Leaf::Count), optional;
    /// Whether every instance of the parent holds one.
    usage: Usage | Option<String>, nullable(Leaf::Text), default_or_null;
    /// The occurrences an instance holds, by name.
    occurrences: BTreeMap<String, OccurrenceModel> | BTreeMap<String, RawOccurrence>,
        Field::Map(&Field::Object(&OCCURRENCE)), default;
    /// The segment, and the element values, that open an instance.
    trigger: TriggerModel | RawTrigger, Field::Object(&TRIGGER), required;
    /// The counts the end segment checks, when it checks any.
    control: Option<ControlModel> | Option<RawControl>, Field::Object(&CONTROL), optional;
}

spec_object! {
    /// The segment that opens a loop's instance and the element values it holds.
    TriggerModel / RawTrigger, TRIGGER, "a trigger object";
    /// The segment.
    segment: String, TEXT, required;
    /// The value each element must hold, by 1-based element position.
    #[serde(deserialize_with = "trigger_where")]
    conditions as "where": BTreeMap<usize, String> | BTreeMap<String, String>,
        Field::Map(&TEXT), default;
}

fn trigger_where<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, String>, D::Error> {
    positions(deserializer, "trigger \"where\"")
}

spec_object! {
    /// One place a segment takes inside a loop's instance.
    OccurrenceModel / RawOccurrence, OCCURRENCE, "an occurrence object";
    /// The segment.
    segment: String, TEXT, required;
    /// The order of the occurrence among those of its loop.
    pos: usize, COUNT, required;
    /// Whether every instance of the loop holds it.
    usage: Usage | Option<String>, nullable(Leaf::Text), default_or_null;
    /// The most times it repeats in one instance, when the spec bounds it.
    max: Option<usize>, nullable(Leaf::Count), optional;
    /// What tells it apart from the other occurrences of its segment.
    qualifier: Option<QualifierModel>, Field::Object(&QUALIFIER), optional;
    /// The codes an element or component may hold here, by position.
    codes: BTreeMap<ElementPosition, Vec<String>> | BTreeMap<String, Vec<String>>,
        Field::Map(&TEXTS), default;
}

spec_object! {
    /// The element (or component) and the codes that tell an occurrence apart
    /// from the other occurrences of its segment in the same loop.
    QualifierModel = RawQualifier, QUALIFIER, "a qualifier object";
    /// 1-based element position.
    element: usize, COUNT, required;
    /// 1-based component position inside the element, when the qualifier is one.
    component: Option<usize>, nullable(Leaf::Count), optional;
    /// The codes the element holds in this occurrence.
    codes: Vec<String>, TEXTS, required;
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
