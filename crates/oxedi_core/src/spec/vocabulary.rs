//! The words of the spec's own closed vocabularies and the codes key, one table
//! each: the word of each value and the value of each word, so the loader reads
//! every word through one place.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::{self, Serialize, Serializer};

use super::balance::Sign;
use super::loops::ControlCount;
use super::occurrences::Usage;
use super::raw::RawPick;
use super::segments::parse_position;
use super::table_error::TableDefError;
use super::tables::Pick;
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

impl Pick {
    /// The pick a written value names, or the value as JSON when it names
    /// none: a word that is not named, or the position 0.
    pub(super) fn read(raw: &RawPick) -> Result<Pick, String> {
        match raw {
            RawPick::Nth(nth) if *nth > 0 => Ok(Pick::Nth(*nth)),
            RawPick::Named(word) => Pick::from_word(word).ok_or_else(|| raw.written()),
            RawPick::Nth(_) => Err(raw.written()),
        }
    }
}

impl Serialize for Pick {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Pick::Nth(nth) => serializer.serialize_u64(u64::try_from(*nth).map_err(|_| {
                ser::Error::custom(format!("pick position {nth} does not fit in 64 bits"))
            })?),
            pick @ (Pick::First | Pick::Last) => {
                match Pick::NAMED.iter().find(|(_, named)| named == pick) {
                    Some((word, _)) => serializer.serialize_str(word),
                    None => Err(ser::Error::custom(format!("pick {pick:?} has no word"))),
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for Pick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawPick::deserialize(deserializer)?;
        Pick::read(&raw).map_err(|written| de::Error::custom(TableDefError::BadPick { written }))
    }
}

/// The position a `codes` key names: an element and maybe one of its
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
                "codes key {key:?} must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form"
            ))
        })
    }
}
