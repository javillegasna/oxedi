//! Why a package's identity is rejected.

use std::fmt;

use super::id::{IdPart, PackageId};
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

/// Why a package header is refused once its keys read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    /// The header's `format` is newer than the reader's.
    FormatTooNew {
        /// The format as written.
        found: u64,
        /// The newest format this reader takes.
        reader: u32,
    },
    /// The id names another layer than the top one `layers` lists.
    IdLayerNotTop {
        /// The package's id, boxed to keep the error small.
        id: Box<PackageId>,
        /// The top layer `layers` lists.
        top: Layer,
    },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::FormatTooNew { found, reader } => write!(
                f,
                "package format {found} is newer than {reader}, the newest format this reader \
                 takes; read the package with a newer oxedi, or write it in format {reader}"
            ),
            HeaderError::IdLayerNotTop { id, top } => write!(
                f,
                "package \"{id}\" names layer \"{}\" in its id, but the top layer \"layers\" \
                 lists is \"{top}\"; the id names the package's top layer",
                id.layer()
            ),
        }
    }
}

impl std::error::Error for HeaderError {}
