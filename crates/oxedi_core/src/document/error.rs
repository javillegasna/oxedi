//! Why a buffer could not become a [`Document`](super::Document).

use crate::delimiters::IsaError;

/// The input is too long to index: segment offsets are stored as `u32`, so a
/// document holds at most [`SizeError::LIMIT`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeError {
    /// Length of the rejected input, in bytes.
    pub len: usize,
}

impl SizeError {
    /// The largest input a document indexes, in bytes (`u32::MAX`).
    pub const LIMIT: usize = u32::MAX as usize;

    /// `Ok` when an input of `len` bytes can be indexed.
    pub(super) const fn check(len: usize) -> Result<(), SizeError> {
        // Compared as u64 so the check is meaningful where usize is 32 bits wide too.
        if len as u64 > Self::LIMIT as u64 {
            Err(SizeError { len })
        } else {
            Ok(())
        }
    }
}

impl std::error::Error for SizeError {}

/// Why [`Document::parse`](super::Document::parse) failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// The delimiters could not be read from the ISA segment.
    Isa(IsaError),
    /// The input is longer than a document can index.
    Size(SizeError),
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Both variants print their inner error in full, so the chain stops
        // here instead of printing it a second time.
        match self {
            DocumentError::Isa(_) | DocumentError::Size(_) => None,
        }
    }
}

impl From<IsaError> for DocumentError {
    fn from(e: IsaError) -> Self {
        DocumentError::Isa(e)
    }
}

impl From<SizeError> for DocumentError {
    fn from(e: SizeError) -> Self {
        DocumentError::Size(e)
    }
}
