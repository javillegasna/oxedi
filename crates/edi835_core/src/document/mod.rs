//! A whole EDI file held losslessly: the bytes plus one span per segment.
//!
//! Building a document only runs the framing pass and records where each
//! segment lives. Segments are parsed on demand and borrow from the document,
//! so the document can hold either a borrowed slice or an owned buffer without
//! two representations.

use std::borrow::Cow;
use std::ops::Range;

use crate::delimiters::{Delimiters, IsaError};
use crate::frame::{Frame, first_frame, next_frame};
use crate::segment::Segment;

/// Where one segment lives inside [`Document::as_bytes`].
///
/// `raw` ranges of consecutive spans are contiguous and together cover the
/// whole buffer; `body` lies inside `raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Leading trivia, body and terminator.
    pub raw: Range<usize>,
    /// The segment text without trivia or terminator.
    pub body: Range<usize>,
    /// `false` only for a final segment with no terminator.
    pub terminated: bool,
}

/// A whole file: its bytes and the spans of its segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<'a> {
    bytes: Cow<'a, [u8]>,
    delims: Delimiters,
    spans: Vec<Span>,
}

impl<'a> Document<'a> {
    /// Indexes `bytes`, reading the delimiters from the ISA segment (which may
    /// be preceded by a UTF-8 byte order mark and trivia, both kept in the
    /// first segment's span). Accepts a borrowed slice or an owned `Vec<u8>`.
    pub fn parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Self, IsaError> {
        let bytes = bytes.into();
        let delims = Delimiters::from_isa_after_leading_trivia(&bytes)?;
        Ok(Self::with_delimiters(bytes, delims))
    }

    /// Indexes `bytes` with caller-supplied delimiters.
    pub fn with_delimiters(bytes: impl Into<Cow<'a, [u8]>>, delims: Delimiters) -> Self {
        let bytes = bytes.into();
        let spans = index(&bytes, &delims);
        Self {
            bytes,
            delims,
            spans,
        }
    }

    /// The file, byte for byte.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The delimiters in use.
    pub fn delimiters(&self) -> &Delimiters {
        &self.delims
    }

    /// One span per segment, in order.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Number of segments, including empty and trivia-only ones.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// `true` when the input had no bytes at all.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// The segment at `index` (0-based, same as [`Segment::index`]), parsed on demand.
    pub fn segment(&self, index: usize) -> Option<Segment<'_>> {
        let span = self.spans.get(index)?;
        Some(self.segment_from(index, span))
    }

    fn segment_from(&self, index: usize, span: &Span) -> Segment<'_> {
        let frame = Frame {
            raw: &self.bytes[span.raw.clone()],
            body: &self.bytes[span.body.clone()],
            terminated: span.terminated,
        };
        Segment::parse(index, frame, &self.delims)
    }

    /// Iterates every segment in order, parsing each on demand.
    pub fn segments(&self) -> Segments<'_, 'a> {
        Segments { doc: self, next: 0 }
    }

    /// Makes the document own its bytes, copying them only if they were borrowed.
    /// Spans are reused as they are.
    pub fn into_owned(self) -> Document<'static> {
        Document {
            bytes: Cow::Owned(self.bytes.into_owned()),
            delims: self.delims,
            spans: self.spans,
        }
    }
}

impl<'d, 'a> IntoIterator for &'d Document<'a> {
    type Item = Segment<'d>;
    type IntoIter = Segments<'d, 'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments()
    }
}

/// Iterator over a document's segments. `'d` is the borrow of the document,
/// `'a` the document's own buffer lifetime; items borrow for `'d`.
#[derive(Debug, Clone)]
pub struct Segments<'d, 'a> {
    doc: &'d Document<'a>,
    next: usize,
}

impl<'d> Iterator for Segments<'d, '_> {
    type Item = Segment<'d>;

    fn next(&mut self) -> Option<Self::Item> {
        let segment = self.doc.segment(self.next)?;
        self.next += 1;
        Some(segment)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.doc.len().saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Segments<'_, '_> {}

/// Runs the framing pass and records each frame as byte ranges.
fn index(bytes: &[u8], delims: &Delimiters) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut rest = bytes;
    let mut offset = 0;
    loop {
        let split = if offset == 0 {
            first_frame(rest, delims)
        } else {
            next_frame(rest, delims)
        };
        let Some((frame, next)) = split else {
            break;
        };
        // raw = trivia + body + terminator, so the body offset is what is left
        // after removing the body and the (0 or 1 byte) terminator from raw.
        let trivia = frame.raw.len() - frame.body.len() - usize::from(frame.terminated);
        let raw = offset..offset + frame.raw.len();
        let body = raw.start + trivia..raw.start + trivia + frame.body.len();
        spans.push(Span {
            raw: raw.clone(),
            body,
            terminated: frame.terminated,
        });
        offset = raw.end;
        rest = next;
    }
    spans
}

#[cfg(test)]
mod tests;
