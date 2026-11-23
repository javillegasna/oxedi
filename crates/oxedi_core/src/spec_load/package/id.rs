//! The parts of a package id that have their own rule.

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
