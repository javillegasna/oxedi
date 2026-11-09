//! The closed words and the codes key a spec writes, one table each: the list
//! of words of a vocabulary, the word of each value and the value of each
//! word, so the loader reads every word through one place.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::{self, Serialize, Serializer};

use super::balance::Sign;
use super::loops::ControlCount;
use super::occurrences::Usage;
use super::raw::RawPick;
use super::segments::parse_position;
use super::tables::Pick;
use crate::mask::Phi;

/// A closed set of words a spec writes for a typed value.
pub(crate) trait Vocabulary: Copy + Eq + 'static {
    /// The key a spec writes the word under, quoted in the error of an
    /// unknown word.
    const KEY: &'static str;

    /// Every value, in the order a spec lists them.
    const ALL: &'static [Self];

    /// The word a spec writes for the value.
    fn word(self) -> &'static str;

    /// The value a spec's word names, or `None` when the word is not in
    /// the vocabulary; matching is exact.
    fn from_word(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|value| value.word() == word)
    }
}

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

impl Vocabulary for Phi {
    const KEY: &'static str = "phi";
    const ALL: &'static [Phi] = &Phi::ALL;

    fn word(self) -> &'static str {
        Phi::word(self)
    }
}

/// Every word of the vocabulary, quoted: `"a" or "b"`, or `"a", "b" or "c"`.
pub(crate) fn words_text<T: Vocabulary>() -> String {
    let quoted: Vec<String> = T::ALL
        .iter()
        .map(|value| format!("{:?}", value.word()))
        .collect();
    match quoted.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

fn write_word<T: Vocabulary, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(value.word())
}

fn read_word<'de, T: Vocabulary, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
    let found = String::deserialize(deserializer)?;
    T::from_word(&found).ok_or_else(|| {
        de::Error::custom(format!(
            "{:?} must be {}; found {found:?}",
            T::KEY,
            words_text::<T>()
        ))
    })
}

// A vocabulary is a JSON string, its word.
macro_rules! serde_by_word {
    ($($vocabulary:ty),+) => {$(
        impl Serialize for $vocabulary {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                write_word(self, serializer)
            }
        }

        impl<'de> Deserialize<'de> for $vocabulary {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                read_word(deserializer)
            }
        }
    )+};
}

serde_by_word!(Usage, ControlCount, Sign, Phi);

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
            Pick::Nth(nth) => serializer.serialize_u64(
                u64::try_from(*nth).map_err(|_| ser::Error::custom("pick position over u64"))?,
            ),
            pick => match Pick::NAMED.iter().find(|(_, named)| named == pick) {
                Some((word, _)) => serializer.serialize_str(word),
                None => Err(ser::Error::custom(format!("pick {pick:?} has no word"))),
            },
        }
    }
}

impl<'de> Deserialize<'de> for Pick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawPick::deserialize(deserializer)?;
        Pick::read(&raw).map_err(|found| {
            de::Error::custom(format!(
                "\"pick\" must be \"first\", \"last\" or a 1-based position; found {found}"
            ))
        })
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
