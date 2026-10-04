//! A whole EDI file held losslessly: the bytes plus one span per segment.
//!
//! Building a document only runs the framing pass and records where each
//! segment lives. Segments are parsed on demand and borrow from the document,
//! so the document can hold either a borrowed slice or an owned buffer without
//! two representations.

use std::borrow::Cow;
use std::ops::Range;

use crate::delimiters::Delimiters;
use crate::frame::{Frame, first_frame, next_frame};
use crate::segment::Segment;

mod error;

pub use error::{DocumentError, SizeError};

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

/// What a document stores per segment: where its body starts and ends.
///
/// The rest of a [`Span`] follows from the framing rules: a segment's `raw`
/// starts where the previous one's ends (at 0 for the first), every segment
/// but the last is terminated by the one byte after its body, and the last is
/// terminated only when its body ends before the input does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Body {
    start: u32,
    end: u32,
}

/// A whole file: its bytes and where each of its segments lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<'a> {
    bytes: Cow<'a, [u8]>,
    delims: Delimiters,
    bodies: Vec<Body>,
}

impl<'a> Document<'a> {
    /// Indexes `bytes`, reading the delimiters from the ISA segment (which may
    /// be preceded by a UTF-8 byte order mark and trivia, both kept in the
    /// first segment's span). Accepts a borrowed slice or an owned `Vec<u8>`.
    ///
    /// Fails when the ISA cannot be read or the input is longer than
    /// [`SizeError::LIMIT`] bytes.
    pub fn parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Self, DocumentError> {
        let bytes = bytes.into();
        let delims = Delimiters::from_isa_after_leading_trivia(&bytes)?;
        Self::with_delimiters(bytes, delims).map_err(DocumentError::Size)
    }

    /// Indexes `bytes` with caller-supplied delimiters.
    ///
    /// Fails only when the input is longer than [`SizeError::LIMIT`] bytes.
    pub fn with_delimiters(
        bytes: impl Into<Cow<'a, [u8]>>,
        delims: Delimiters,
    ) -> Result<Self, SizeError> {
        let bytes = bytes.into();
        SizeError::check(bytes.len())?;
        let bodies = index(&bytes, &delims);
        Ok(Self {
            bytes,
            delims,
            bodies,
        })
    }

    /// The file, byte for byte.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The delimiters in use.
    pub fn delimiters(&self) -> &Delimiters {
        &self.delims
    }

    /// Where the segment at `index` lives; `None` past the last segment.
    pub fn span(&self, index: usize) -> Option<Span> {
        let body = self.bodies.get(index)?;
        let raw_start = match index.checked_sub(1) {
            // The previous segment is not the last, so it is terminated by
            // the one byte after its body.
            Some(previous) => self.bodies.get(previous).map_or(0, |b| offset(b.end) + 1),
            None => 0,
        };
        let body = offset(body.start)..offset(body.end);
        let terminated = index + 1 < self.bodies.len() || body.end < self.bytes.len();
        Some(Span {
            raw: raw_start..body.end + usize::from(terminated),
            body,
            terminated,
        })
    }

    /// One span per segment, in order.
    pub fn spans(&self) -> Spans<'_, 'a> {
        Spans { doc: self, next: 0 }
    }

    /// Number of segments, including empty and trivia-only ones.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// `true` when the input had no bytes at all.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// The segment at `index` (0-based, same as [`Segment::index`]), parsed on demand.
    pub fn segment(&self, index: usize) -> Option<Segment<'_>> {
        let span = self.span(index)?;
        let frame = Frame {
            raw: self.bytes.get(span.raw)?,
            body: self.bytes.get(span.body)?,
            terminated: span.terminated,
        };
        Some(Segment::parse(index, frame, &self.delims))
    }

    /// Iterates every segment in order, parsing each on demand.
    pub fn segments(&self) -> Segments<'_, 'a> {
        Segments { doc: self, next: 0 }
    }

    /// Makes the document own its bytes, copying them only if they were borrowed.
    /// The index is reused as it is.
    pub fn into_owned(self) -> Document<'static> {
        Document {
            bytes: Cow::Owned(self.bytes.into_owned()),
            delims: self.delims,
            bodies: self.bodies,
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

/// Iterator over a document's spans, computed one at a time. `'d` is the
/// borrow of the document, `'a` the document's own buffer lifetime.
#[derive(Debug, Clone)]
pub struct Spans<'d, 'a> {
    doc: &'d Document<'a>,
    next: usize,
}

impl Iterator for Spans<'_, '_> {
    type Item = Span;

    fn next(&mut self) -> Option<Self::Item> {
        let span = self.doc.span(self.next)?;
        self.next += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.doc.len().saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Spans<'_, '_> {}

/// A stored offset as a buffer index. Offsets were taken from a buffer of at
/// most [`SizeError::LIMIT`] bytes, so they fit in `u32` and widen losslessly.
const fn offset(at: u32) -> usize {
    at as usize
}

/// Runs the framing pass and records where each frame's body starts and ends.
/// The caller has checked that `bytes.len()` fits in `u32`, so every offset
/// below, being at most `bytes.len()`, does too.
fn index(bytes: &[u8], delims: &Delimiters) -> Vec<Body> {
    let mut bodies = Vec::new();
    let mut rest = bytes;
    let mut raw_start = 0;
    loop {
        let split = if raw_start == 0 {
            first_frame(rest, delims)
        } else {
            next_frame(rest, delims)
        };
        let Some((frame, next)) = split else {
            break;
        };
        // raw = trivia + body + terminator, so the body starts after the
        // trivia, which is what remains of raw without the body and the
        // (0 or 1 byte) terminator.
        let trivia = frame.raw.len() - frame.body.len() - usize::from(frame.terminated);
        let start = raw_start + trivia;
        bodies.push(Body {
            start: start as u32,
            end: (start + frame.body.len()) as u32,
        });
        raw_start += frame.raw.len();
        rest = next;
    }
    bodies
}

#[cfg(test)]
mod tests;
