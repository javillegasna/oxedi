//! The parts of a package id that have their own rule.

use std::fmt;

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
