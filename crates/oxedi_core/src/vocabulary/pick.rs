//! Which matching segment a column reads: a word (`first`, `last`) or a 1-based position, read
//! and written through one place.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, Serializer};

/// Which of the segments that match a column's source gives its value.
///
/// The segments considered are, in file order: for a source in the anchor
/// loop or a loop inside it, those captured while the row's anchor instance
/// is open (in that instance, or in any instance of the inner loop inside
/// it); for a source in a loop above the anchor, those the enclosing
/// instance of that loop captured before the row's anchor instance opened
/// (an instance captures nothing while a loop inside it is open, so these
/// are all it captures until the row is appended). A table anchored on a
/// segment reads that segment only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pick {
    /// The first segment that matches; written `"first"`, and the default.
    #[default]
    First,
    /// The last segment that matches; written `"last"`.
    Last,
    /// The n-th segment that matches, counting from 1; written as the
    /// number. The value is null when fewer segments match.
    Nth(usize),
}

/// How a spec writes a pick: a word or a position.
enum Written {
    Word(&'static str),
    Position(usize),
}

impl Pick {
    /// The named picks, in the order the refusal lists their words.
    pub(crate) const NAMED: [Pick; 2] = [Pick::First, Pick::Last];
    /// What a spec may write for a pick; the shape check and the refusal both
    /// say it.
    pub(crate) const EXPECTED: &'static str = "\"first\", \"last\" or a 1-based position";

    /// The one place a pick's word or position is written.
    fn written(self) -> Written {
        match self {
            Pick::First => Written::Word("first"),
            Pick::Last => Written::Word("last"),
            Pick::Nth(nth) => Written::Position(nth),
        }
    }

    /// The named pick a spec's word names, or `None`.
    pub(crate) fn from_word(word: &str) -> Option<Pick> {
        Pick::NAMED
            .into_iter()
            .find(|pick| matches!(pick.written(), Written::Word(named) if named == word))
    }
}

impl fmt::Display for Pick {
    /// The pick as a spec writes it: `"first"`, `"last"` or the position.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.written() {
            Written::Word(word) => write!(f, "\"{word}\""),
            Written::Position(nth) => write!(f, "{nth}"),
        }
    }
}

impl Serialize for Pick {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.written() {
            Written::Word(word) => serializer.serialize_str(word),
            Written::Position(nth) => {
                serializer.serialize_u64(u64::try_from(nth).map_err(|_| {
                    ser::Error::custom(format!("pick position {nth} does not fit in 64 bits"))
                })?)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Pick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PickVisitor)
    }
}

/// Reads a word or a whole number from 1 into a `Pick`; any other JSON value
/// is refused with the same text.
struct PickVisitor;

impl PickVisitor {
    fn refuse<E: de::Error>(written: impl fmt::Display) -> E {
        E::custom(format!(
            "\"pick\" must be {}; found {written}",
            Pick::EXPECTED
        ))
    }
}

impl<'de> Visitor<'de> for PickVisitor {
    type Value = Pick;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Pick::EXPECTED)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Pick, E> {
        usize::try_from(v)
            .ok()
            .filter(|nth| *nth > 0)
            .map(Pick::Nth)
            .ok_or_else(|| Self::refuse(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Pick, E> {
        match u64::try_from(v) {
            Ok(positive) => self.visit_u64(positive),
            Err(_) => Err(Self::refuse(v)),
        }
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Pick, E> {
        Err(Self::refuse(v))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Pick, E> {
        Err(Self::refuse(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Pick, E> {
        Err(Self::refuse("null"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Pick, E> {
        Pick::from_word(v).ok_or_else(|| Self::refuse(serde_json::Value::from(v)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<Pick, A::Error> {
        Err(Self::refuse("an array"))
    }

    fn visit_map<A: MapAccess<'de>>(self, _: A) -> Result<Pick, A::Error> {
        Err(Self::refuse("an object"))
    }
}
