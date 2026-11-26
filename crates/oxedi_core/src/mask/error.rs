//! The error of building a [`Mask`](super::Mask) or reading a date mode.

use std::fmt;

use super::Mask;
use crate::vocabulary::{DateMode, words_text};

/// Why a mask or a date mode could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskError {
    /// The key is shorter than [`Mask::MIN_KEY`](super::Mask::MIN_KEY) bytes.
    ShortKey {
        /// The number of bytes given.
        length: usize,
    },
    /// The date mode is not `year`, `shift` or `keep`.
    UnknownDateMode {
        /// The text given.
        given: String,
    },
}

impl fmt::Display for MaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaskError::ShortKey { length } => write!(
                f,
                "the mask key must be at least {} bytes long; got {length}",
                Mask::MIN_KEY
            ),
            MaskError::UnknownDateMode { given } => write!(
                f,
                "the mask date mode must be {}; got {given:?}",
                words_text::<DateMode>()
            ),
        }
    }
}

impl std::error::Error for MaskError {}
