//! The words of the spec's own closed vocabularies and the codes key, one table
//! each: the word of each value and the value of each word, so the loader reads
//! every word through one place.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

use super::segments::parse_position;
use crate::spec::{ControlCount, Sign, Usage};
use crate::vocabulary::{Vocabulary, serde_by_word};

impl Vocabulary for Usage {
    const KEY: &'static str = "usage";
    const ALL: &'static [Usage] = &[Usage::Required, Usage::Situational];

    fn word(self) -> &'static str {
        match self {
            Usage::Required => "required",
            Usage::Situational => "situational",
        }
    }
}

impl Vocabulary for ControlCount {
    const KEY: &'static str = "count";
    const ALL: &'static [ControlCount] = &[ControlCount::Segments, ControlCount::Children];

    fn word(self) -> &'static str {
        match self {
            ControlCount::Segments => "segments",
            ControlCount::Children => "children",
        }
    }
}

impl Vocabulary for Sign {
    const KEY: &'static str = "sign";
    const ALL: &'static [Sign] = &[Sign::Plus, Sign::Minus];

    fn word(self) -> &'static str {
        match self {
            Sign::Plus => "+",
            Sign::Minus => "-",
        }
    }
}

serde_by_word!(Usage, ControlCount, Sign);

/// The position an `elements` key names: an element and maybe one of its
/// components, written `<element>` or `<element>-<component>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementPosition {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position inside the element, when the key names one.
    pub component: Option<usize>,
}

impl ElementPosition {
    /// Reads a key in canonical form (no sign, no leading zero, no position
    /// 0); `None` for anything else.
    pub fn parse(key: &str) -> Option<ElementPosition> {
        let (element, component) = match key.split_once('-') {
            None => (key, None),
            Some((element, component)) => (element, Some(parse_position(component)?)),
        };
        Some(ElementPosition {
            element: parse_position(element)?,
            component,
        })
    }
}

impl fmt::Display for ElementPosition {
    /// The key as a spec writes it: `3` or `3-2`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.component {
            None => write!(f, "{}", self.element),
            Some(component) => write!(f, "{}-{component}", self.element),
        }
    }
}

// A key is a string, so a position works as a map key.
impl Serialize for ElementPosition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ElementPosition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        ElementPosition::parse(&key).ok_or_else(|| {
            de::Error::custom(format!(
                "elements key {key:?} must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form"
            ))
        })
    }
}
