//! Why a kit manifest is refused once its keys read.

use std::fmt;

/// Why a kit manifest is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KitError {
    /// The kit's `format` is newer than the reader's.
    FormatTooNew {
        /// The format as written.
        found: u64,
        /// The newest format this reader takes.
        reader: u32,
    },
    /// The kit holds no case.
    NoCases,
    /// Two cases carry the same name.
    RepeatedCase {
        /// The name written twice under `cases`.
        case: String,
    },
    /// A case names no expected output.
    NoOutputs {
        /// The case, as named under `cases`.
        case: String,
    },
}

impl fmt::Display for KitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KitError::FormatTooNew { found, reader } => write!(
                f,
                "kit format {found} is newer than {reader}, the newest format this reader \
                 takes; read the kit with a newer oxedi, or write it in format {reader}"
            ),
            KitError::NoCases => {
                write!(f, "the kit has no case; add one under \"cases\"")
            }
            KitError::RepeatedCase { case } => write!(
                f,
                "case {case:?} is named twice under \"cases\"; rename one"
            ),
            KitError::NoOutputs { case } => write!(
                f,
                "case {case:?} expects no output; name a file under \"expect\" for \
                 \"tables\" or \"diagnostics\""
            ),
        }
    }
}

impl std::error::Error for KitError {}
