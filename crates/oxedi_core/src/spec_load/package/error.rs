//! Why a package's identity is rejected.

use std::fmt;

use super::id::IdPart;
use super::layer::Layer;
use crate::vocabulary::words_text;

/// Why an id does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The id has fewer parts than its layer's form, or more.
    Parts {
        /// The id as written.
        id: String,
        /// The form the id should have, e.g. `<author>/envelope/<release>`.
        form: &'static str,
    },
    /// The second part is not a layer word.
    Layer {
        /// The id as written.
        id: String,
        /// The second part as written.
        layer: String,
    },
    /// A part breaks its own rule.
    Part {
        /// The id as written.
        id: String,
        /// Which part.
        part: IdPart,
        /// The part as written.
        text: String,
    },
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Parts { id, form } => {
                write!(f, "package id \"{id}\" is not written {form}")
            }
            IdError::Layer { id, layer } => write!(
                f,
                "package id \"{id}\": layer \"{layer}\" must be {}",
                words_text::<Layer>()
            ),
            IdError::Part { id, part, text } => write!(
                f,
                "package id \"{id}\": {} \"{text}\" must be {}",
                part.name(),
                part.rule()
            ),
        }
    }
}

impl std::error::Error for IdError {}
