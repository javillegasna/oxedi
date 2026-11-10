//! The segments of a spec and their elements, as the spec writes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::declare::spec_object;
use super::{is_default, positions};
use crate::spec::raw::RawElement;
use crate::spec::sections::ELEMENT;
use crate::spec::shape::Field;
use crate::value::ElementType;

fn segment_elements<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, ElementModel>, D::Error> {
    positions(deserializer, "segment \"elements\"")
}

fn element_composite<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, ElementModel>, D::Error> {
    positions(deserializer, "element \"composite\"")
}

spec_object! {
    /// The elements of a segment the spec defines.
    SegmentModel / RawSegment, SEGMENT, "a segment object";
    /// The elements, by 1-based element position.
    #[serde(deserialize_with = "segment_elements")]
    elements: BTreeMap<usize, ElementModel> | BTreeMap<String, RawElement>,
        Field::Map(&Field::Object(&ELEMENT)), default;
}

/// The keys an element object takes, as an unknown-key error lists them.
pub(in crate::spec) const ELEMENT_MODEL_KEYS: &[&str] = &[
    "name",
    "type",
    "scale",
    "required",
    "min",
    "max",
    "codes",
    "composite",
];

/// An element of a segment, or a component of a composite element.
///
/// A key it does not take is refused by the flattened kind, which reads every
/// key the other fields leave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(expecting = "an element object")]
pub struct ElementModel {
    /// The name its column takes.
    pub name: String,
    /// The data type, written `type`, and for `R` its `scale`.
    #[serde(flatten, with = "type_keys")]
    pub kind: ElementType,
    /// Whether a segment must hold it.
    #[serde(default, skip_serializing_if = "is_default")]
    pub required: bool,
    /// The shortest length, when the spec bounds it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<usize>,
    /// The longest length, when the spec bounds it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// The codes it may hold, when the spec lists them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codes: Option<Vec<String>>,
    /// The components, by 1-based component position.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "element_composite"
    )]
    pub composite: BTreeMap<usize, ElementModel>,
}

// The `type` and `scale` keys, read through `ElementType::parse` so a pair
// the loader refuses is refused with its text; an `R` scale is written only
// when it is not the default.
mod type_keys {
    use std::collections::BTreeMap;

    use serde::de::IgnoredAny;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    use crate::value::ElementType;

    #[derive(Serialize, Deserialize)]
    struct TypeKeys {
        #[serde(rename = "type")]
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<u8>,
    }

    // What the element's other fields leave: the kind's keys, and any key
    // nothing takes.
    #[derive(Deserialize)]
    struct ReadKeys {
        #[serde(flatten)]
        keys: TypeKeys,
        #[serde(flatten)]
        unknown: BTreeMap<String, IgnoredAny>,
    }

    pub(super) fn serialize<S: Serializer>(
        kind: &ElementType,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let scale = kind
            .scale()
            .filter(|scale| *scale != ElementType::DEFAULT_SCALE);
        TypeKeys {
            code: kind.code(),
            scale,
        }
        .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ElementType, D::Error> {
        let ReadKeys { keys, unknown } = ReadKeys::deserialize(deserializer)?;
        if let Some(key) = unknown.keys().next() {
            return Err(de::Error::unknown_field(key, super::ELEMENT_MODEL_KEYS));
        }
        ElementType::parse(&keys.code, keys.scale).map_err(de::Error::custom)
    }
}
