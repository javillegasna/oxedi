//! A package's identity and the grammar it is read by.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer};

use super::error::IdError;
use super::layer::Layer;

/// The form every id shares, quoted when the id is too short to name its layer.
const GENERAL_FORM: &str = "<author>/<layer>/<release>[/<transaction set>[/<guide>]]";

/// A part of an id with its own rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdPart {
    /// The first part.
    Author,
    /// The release, the third part.
    Release,
    /// The transaction set, the fourth part.
    TransactionSet,
    /// The guide, the fifth part.
    Guide,
}

/// `text` is 1 to `max` characters, its first one `first` and every other `rest`.
fn starts_then(text: &str, max: usize, first: fn(char) -> bool, rest: fn(char) -> bool) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(first) && text.len() <= max && chars.all(rest)
}

/// `text` is exactly `len` ASCII digits.
fn digits(text: &str, len: usize) -> bool {
    text.len() == len && text.chars().all(|c| c.is_ascii_digit())
}

impl IdPart {
    /// What the part is called in a message.
    pub const fn name(self) -> &'static str {
        match self {
            IdPart::Author => "author",
            IdPart::Release => "release",
            IdPart::TransactionSet => "transaction set",
            IdPart::Guide => "guide",
        }
    }

    /// The part's rule, written as a message quotes it.
    pub const fn rule(self) -> &'static str {
        match self {
            IdPart::Author => {
                "lowercase ASCII letters, digits and \"-\", starting with a letter, at most 32, e.g. \"oxedi\""
            }
            IdPart::Release => {
                "six ASCII digits, the X12 release as GS08 starts it, e.g. \"005010\""
            }
            IdPart::TransactionSet => "three ASCII digits, e.g. \"835\"",
            IdPart::Guide => {
                "uppercase ASCII letters and digits, starting with a letter, at most 16, e.g. \"X221A1\""
            }
        }
    }

    /// Whether `text` follows the part's rule.
    fn accepts(self, text: &str) -> bool {
        match self {
            IdPart::Author => starts_then(
                text,
                32,
                |c| c.is_ascii_lowercase(),
                |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-',
            ),
            IdPart::Release => digits(text, 6),
            IdPart::TransactionSet => digits(text, 3),
            IdPart::Guide => starts_then(
                text,
                16,
                |c| c.is_ascii_uppercase(),
                |c| c.is_ascii_uppercase() || c.is_ascii_digit(),
            ),
        }
    }
}

/// The X12 release an id names, six digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Release(String);

impl Release {
    /// The six digits.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The transaction set an id names, three digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionSet(String);

impl TransactionSet {
    /// The three digits.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TransactionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The implementation guide an id names, e.g. `X221A1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Guide(String);

impl Guide {
    /// The guide's name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Guide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What an id names beyond its release.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    /// The release alone.
    Release,
    /// One transaction set of the release.
    Set(TransactionSet),
    /// One implementation guide of a transaction set.
    Guide {
        /// The transaction set the guide is for.
        set: TransactionSet,
        /// The guide the id names.
        guide: Guide,
    },
}

/// A package's identity, read from its canonical text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageId {
    text: String,
    author: String,
    layer: Layer,
    release: Release,
    scope: Scope,
}

impl PackageId {
    /// Reads an id. The text is canonical: any other spelling is refused.
    pub fn parse(text: &str) -> Result<PackageId, IdError> {
        let parts_error = |form| IdError::Parts {
            id: text.to_string(),
            form,
        };
        let fields: Vec<&str> = text.split('/').collect();
        let [author, word, rest @ ..] = fields.as_slice() else {
            return Err(parts_error(GENERAL_FORM));
        };
        let layer = Layer::parse(word).ok_or_else(|| IdError::Layer {
            id: text.to_string(),
            layer: word.to_string(),
        })?;
        let [release, tail @ ..] = rest else {
            return Err(parts_error(layer.form()));
        };
        if fields.len() != layer.parts() {
            return Err(parts_error(layer.form()));
        }
        let named = [
            (IdPart::Author, Some(author)),
            (IdPart::Release, Some(release)),
        ];
        let tail_parts = [IdPart::TransactionSet, IdPart::Guide]
            .into_iter()
            .zip(tail.iter().map(Some));
        for (part, found) in named.into_iter().chain(tail_parts) {
            if let Some(found) = found.filter(|found| !part.accepts(found)) {
                return Err(IdError::Part {
                    id: text.to_string(),
                    part,
                    text: found.to_string(),
                });
            }
        }
        Ok(PackageId {
            text: text.to_string(),
            author: author.to_string(),
            layer,
            release: Release(release.to_string()),
            scope: scope_of(tail),
        })
    }

    /// The canonical text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The first part.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// The layer the id names.
    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// The release.
    pub fn release(&self) -> &Release {
        &self.release
    }

    /// What the id names beyond its release.
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// The transaction set, when the id names one.
    pub fn transaction_set(&self) -> Option<&TransactionSet> {
        match &self.scope {
            Scope::Release => None,
            Scope::Set(set) | Scope::Guide { set, .. } => Some(set),
        }
    }

    /// The guide, when the id names one.
    pub fn guide(&self) -> Option<&Guide> {
        match &self.scope {
            Scope::Guide { guide, .. } => Some(guide),
            Scope::Release | Scope::Set(_) => None,
        }
    }

    /// The name a stack topped by this package takes: its transaction set, else
    /// its release.
    pub fn spec_name(&self) -> &str {
        self.transaction_set()
            .map_or(self.release.as_str(), TransactionSet::as_str)
    }
}

/// The scope the parts after the release name; `tail` holds the set, then the
/// guide, as many as the layer's form has.
fn scope_of(tail: &[&str]) -> Scope {
    match tail {
        [set, guide, ..] => Scope::Guide {
            set: TransactionSet(set.to_string()),
            guide: Guide(guide.to_string()),
        },
        [set] => Scope::Set(TransactionSet(set.to_string())),
        [] => Scope::Release,
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl<'de> Deserialize<'de> for PackageId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        PackageId::parse(&text).map_err(de::Error::custom)
    }
}
