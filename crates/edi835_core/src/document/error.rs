//! Why a buffer could not become a [`Document`](super::Document).

use std::fmt;

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
        if len > Self::LIMIT {
            Err(SizeError { len })
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for SizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "input of {} bytes exceeds the document size limit of {} bytes \
             (segment offsets are stored as 32-bit integers)",
            self.len,
            Self::LIMIT
        )
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

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Isa(e) => write!(f, "{e}"),
            DocumentError::Size(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DocumentError::Isa(e) => Some(e),
            DocumentError::Size(e) => Some(e),
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
