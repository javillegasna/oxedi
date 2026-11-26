//! The five delimiters by role, and the check that a caller's text for one
//! is a single byte.

use super::Delimiters;

/// One of the five delimiters of an interchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterRole {
    /// Separates elements.
    Element,
    /// Separates components.
    Component,
    /// Ends a segment.
    Segment,
    /// Separates repetitions.
    Repetition,
    /// Makes the following byte literal.
    Release,
}

impl DelimiterRole {
    /// Every role, in the order the delimiters are written.
    pub const ALL: [DelimiterRole; 5] = [
        DelimiterRole::Element,
        DelimiterRole::Component,
        DelimiterRole::Segment,
        DelimiterRole::Repetition,
        DelimiterRole::Release,
    ];

    /// The role's name as a word.
    pub const fn word(self) -> &'static str {
        match self {
            DelimiterRole::Element => "element",
            DelimiterRole::Component => "component",
            DelimiterRole::Segment => "segment",
            DelimiterRole::Repetition => "repetition",
            DelimiterRole::Release => "release",
        }
    }
}

/// A delimiter given as other than one byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimiterLength {
    /// The delimiter it was given for.
    pub role: DelimiterRole,
    /// The bytes given.
    pub bytes: Vec<u8>,
}

impl std::error::Error for DelimiterLength {}

impl Delimiters {
    /// The one byte `value` holds, for the delimiter `role`.
    pub fn byte(role: DelimiterRole, value: &[u8]) -> Result<u8, DelimiterLength> {
        match value {
            [byte] => Ok(*byte),
            _ => Err(DelimiterLength {
                role,
                bytes: value.to_vec(),
            }),
        }
    }
}
