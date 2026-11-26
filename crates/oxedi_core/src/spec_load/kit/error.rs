//! Why a kit manifest is refused once its keys read.

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

impl std::error::Error for KitError {}
