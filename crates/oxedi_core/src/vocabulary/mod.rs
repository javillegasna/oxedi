//! Closed vocabularies: typed values with a fixed set of words, each word read
//! and written through one place.
//!
//! [`Phi`] (what a patient-data column holds) and [`DateMode`] (what masking
//! does to a date) are defined here, so the spec that flags columns and the
//! mask that replaces them both use this module and neither uses the other.
//! The spec's own words (usage, control counts, signs) implement `Vocabulary`
//! next to their types in the spec; reading a date mode, which fails with a
//! mask error, is written in the mask.

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::Serializer;

/// A closed set of words a typed value is written as.
pub(crate) trait Vocabulary: Copy + Eq + 'static {
    /// The key the word is written under, quoted in the error of an unknown
    /// word.
    const KEY: &'static str;

    /// Every value, in the order its words are listed.
    const ALL: &'static [Self];

    /// The word that names the value.
    fn word(self) -> &'static str;

    /// The value a spec's word names, or `None` when the word is not in
    /// the vocabulary; matching is exact.
    fn from_word(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|value| value.word() == word)
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

pub(crate) fn write_word<T: Vocabulary, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(value.word())
}

pub(crate) fn read_word<'de, T: Vocabulary, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
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
        impl ::serde::Serialize for $vocabulary {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                $crate::vocabulary::write_word(self, serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $vocabulary {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::vocabulary::read_word(deserializer)
            }
        }
    )+};
}

pub(crate) use serde_by_word;

/// What a flagged column holds, which decides how it is masked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phi {
    /// A person's name.
    Name,
    /// An identifier of a person or of a claim.
    Id,
    /// A date.
    Date,
}

impl Phi {
    /// Every kind, in the order a spec lists them.
    pub const ALL: [Phi; 3] = [Phi::Name, Phi::Id, Phi::Date];

    /// The word a spec writes for the kind, also mixed into the digest so the
    /// same value masks differently per kind.
    pub fn word(self) -> &'static str {
        match self {
            Phi::Name => "name",
            Phi::Id => "id",
            Phi::Date => "date",
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

serde_by_word!(Phi);

/// What happens to a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateMode {
    /// January 1 of the same year.
    Year,
    /// The whole row moves by the same number of days.
    Shift,
    /// Unchanged.
    Keep,
}

impl DateMode {
    /// The word that names the mode: `year`, `shift` or `keep`.
    pub fn word(self) -> &'static str {
        match self {
            DateMode::Year => "year",
            DateMode::Shift => "shift",
            DateMode::Keep => "keep",
        }
    }
}

impl Vocabulary for DateMode {
    const KEY: &'static str = "dates";
    const ALL: &'static [DateMode] = &[DateMode::Year, DateMode::Shift, DateMode::Keep];

    fn word(self) -> &'static str {
        DateMode::word(self)
    }
}
