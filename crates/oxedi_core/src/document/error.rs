//! Why a buffer could not become a [`Document`](super::Document).

use std::num::NonZeroUsize;

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

/// Where to count a segment from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Index {
    /// 0-based position from the first segment.
    FromStart(usize),
    /// 1-based position from the last segment: 1 is the last.
    FromEnd(NonZeroUsize),
}

/// A segment index past the ends of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentOutOfRange {
    /// The index asked for.
    pub index: Index,
    /// The number of segments the document has.
    pub len: usize,
}

impl std::error::Error for SegmentOutOfRange {}

/// Why [`Document::parse`](super::Document::parse) failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// The delimiters could not be read from the ISA segment.
    Isa(IsaError),
    /// The input is longer than a document can index.
    Size(SizeError),
}

impl DocumentError {
    /// The bytes the error is about in `input`, the bytes it was read from:
    /// [`IsaError::datum`] for an ISA error, nothing for an input too long to
    /// index.
    pub fn datum<'a>(&'a self, input: &'a [u8]) -> &'a [u8] {
        match self {
            DocumentError::Isa(error) => error.datum(input),
            DocumentError::Size(_) => &[],
        }
    }
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
