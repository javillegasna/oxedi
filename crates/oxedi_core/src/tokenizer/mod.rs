//! The tokenizer: a lazy iterator of segments over a byte buffer.
//!
//! It has no per-segment errors. Every frame becomes a [`Segment`], including
//! empty ones (`~~`) and the file's trailing trivia, which carry an empty `id`.
//! Nothing is dropped, nothing panics, and the consumer decides what an empty
//! id means. Between two calls to `next` the tokenizer is simply paused.

use crate::delimiters::{Delimiters, IsaError};
use crate::frame::{first_frame, next_frame};
use crate::segment::Segment;

/// Iterator of segments over `input`.
#[derive(Debug, Clone)]
pub struct Tokenizer<'a> {
    rest: &'a [u8],
    delims: Delimiters,
    next_index: usize,
}

impl<'a> Tokenizer<'a> {
    /// Reads the delimiters from the ISA segment, which may be preceded by a
    /// UTF-8 byte order mark and trivia; both stay in the first segment's `raw`.
    ///
    /// `release` is never read from the file. To use one, read the delimiters
    /// with [`Delimiters::from_isa`], add it, and call [`Tokenizer::with_delimiters`].
    pub fn new(input: &'a [u8]) -> Result<Self, IsaError> {
        let delims = Delimiters::from_isa_after_leading_trivia(input)?;
        Ok(Self::with_delimiters(input, delims))
    }

    /// Tokenizes with caller-supplied delimiters: fragments without an ISA, or
    /// an ISA-derived set extended with a release byte.
    pub const fn with_delimiters(input: &'a [u8], delims: Delimiters) -> Self {
        Self {
            rest: input,
            delims,
            next_index: 0,
        }
    }

    /// The delimiters in use.
    pub const fn delimiters(&self) -> &Delimiters {
        &self.delims
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (frame, rest) = if self.next_index == 0 {
            first_frame(self.rest, &self.delims)?
        } else {
            next_frame(self.rest, &self.delims)?
        };
        self.rest = rest;
        let segment = Segment::parse(self.next_index, frame, &self.delims);
        self.next_index += 1;
        Some(segment)
    }
}

#[cfg(test)]
mod tests;
