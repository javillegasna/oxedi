//! The segments of a spec and their elements, as the spec writes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::declare::spec_object;
use super::is_default;
use crate::spec::raw::RawElement;
use crate::spec::sections::ELEMENT;
use crate::spec::shape::Field;
use crate::value::ElementType;

spec_object! {
    /// The elements of a segment the spec defines.
    SegmentModel / RawSegment, SEGMENT, "a segment object";
    /// The elements, by 1-based element position.
    elements: BTreeMap<usize, ElementModel> | BTreeMap<String, RawElement>,
        Field::Map(&Field::Object(&ELEMENT)), default;
}

/// An element of a segment, or a component of a composite element.
///
/// It does not refuse an unknown key: serde does not support
/// `deny_unknown_fields` next to the flattened `kind`.
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub composite: BTreeMap<usize, ElementModel>,
}

// The `type` and `scale` keys, read through `ElementType::parse` so a pair
// the loader refuses is refused with its text; an `R` scale is written only
// when it is not the default.
mod type_keys {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    use crate::value::ElementType;

    #[derive(Serialize, Deserialize)]
    struct TypeKeys {
        #[serde(rename = "type")]
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<u8>,
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
        let keys = TypeKeys::deserialize(deserializer)?;
        ElementType::parse(&keys.code, keys.scale).map_err(de::Error::custom)
    }
}
