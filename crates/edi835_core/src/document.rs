//! A whole EDI file held losslessly: the bytes plus one span per segment.
//!
//! Building a document only runs the framing pass and records where each
//! segment lives. Segments are parsed on demand and borrow from the document,
//! so the document can hold either a borrowed slice or an owned buffer without
//! two representations.

use std::borrow::Cow;
use std::ops::Range;

use crate::delimiters::{Delimiters, IsaError};
use crate::frame::{Frame, first_frame, leading_trivia, next_frame};
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
        let rest = bytes.get(leading_trivia(&bytes)..).unwrap_or_default();
        let delims = Delimiters::from_isa(rest)?;
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
mod tests {
    use super::*;
    use crate::{Delimiters, Tokenizer};

    const ISA: &[u8] =
        b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

    fn plain() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    #[test]
    fn with_delimiters_indexes_every_frame() {
        let doc = Document::with_delimiters(&b"ST*835~\nSE*2~\n"[..], plain());
        assert_eq!(doc.len(), 3);
        assert_eq!(doc.segment(0).unwrap().id, b"ST");
        assert_eq!(doc.segment(1).unwrap().id, b"SE");
        assert!(doc.segment(2).unwrap().is_empty());
        assert_eq!(doc.segment(3), None);
    }

    #[test]
    fn spans_partition_the_bytes() {
        let input = &b"\nST*835~\n\nSE~X"[..];
        let doc = Document::with_delimiters(input, plain());
        let spans = doc.spans();
        assert_eq!(
            spans[0],
            Span {
                raw: 0..8,
                body: 1..7,
                terminated: true
            }
        );
        assert_eq!(
            spans[1],
            Span {
                raw: 8..13,
                body: 10..12,
                terminated: true
            }
        );
        assert_eq!(
            spans[2],
            Span {
                raw: 13..14,
                body: 13..14,
                terminated: false
            }
        );
        let mut next = 0;
        for span in spans {
            assert_eq!(span.raw.start, next, "spans must be contiguous");
            assert!(span.raw.start <= span.body.start && span.body.end <= span.raw.end);
            next = span.raw.end;
        }
        assert_eq!(next, input.len(), "spans must cover the whole input");

        let tail_only = Document::with_delimiters(&b"ST~\n"[..], plain());
        assert_eq!(
            tail_only.spans()[1],
            Span {
                raw: 3..4,
                body: 4..4,
                terminated: false
            }
        );
    }

    #[test]
    fn segment_equals_tokenizer_output() {
        let input = &b"ISA*00~\r\nSVC*HC:99213*100**12~CAS*CO*45~~"[..];
        let doc = Document::with_delimiters(input, plain());
        let expected: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
        assert_eq!(doc.len(), expected.len());
        for (i, segment) in expected.iter().enumerate() {
            assert_eq!(doc.segment(i).as_ref(), Some(segment), "segment {i}");
        }
    }

    #[test]
    fn parse_reads_delimiters_from_the_isa_and_tolerates_leading_trivia() {
        let mut input = b"\r\n".to_vec();
        input.extend_from_slice(ISA);
        input.extend_from_slice(b"GS*HP:X~");
        let doc = Document::parse(&input[..]).unwrap();
        assert_eq!(doc.delimiters().component, b':');
        assert_eq!(doc.segment(0).unwrap().id, b"ISA");
        assert!(doc.segment(0).unwrap().raw.starts_with(b"\r\n"));
        assert_eq!(
            doc.segment(1).unwrap().element(1).and_then(|e| e.simple()),
            None
        );
    }

    #[test]
    fn parse_keeps_a_byte_order_mark_in_the_first_span() {
        let mut input = b"\xEF\xBB\xBF".to_vec();
        input.extend_from_slice(ISA);
        input.extend_from_slice(b"GS*HP~");
        let doc = Document::parse(&input[..]).unwrap();
        assert_eq!(doc.len(), 2);
        assert_eq!(
            doc.spans()[0],
            Span {
                raw: 0..3 + ISA.len(),
                body: 3..3 + ISA.len() - 1,
                terminated: true
            }
        );
        assert_eq!(doc.segment(0).unwrap().id, b"ISA");
        assert_eq!(doc.segment(1).unwrap().id, b"GS");
        assert_eq!(doc.as_bytes(), &input[..]);
        let expected: Vec<_> = Tokenizer::new(&input).unwrap().collect();
        assert_eq!(doc.segments().collect::<Vec<_>>(), expected);
    }

    #[test]
    fn parse_fails_without_an_isa() {
        assert_eq!(
            Document::parse(&b"ST*835~"[..]).err(),
            Some(IsaError::NotIsa {
                found: b"ST*835~".to_vec()
            })
        );
        assert_eq!(
            Document::parse(&b""[..]).err(),
            Some(IsaError::NotIsa { found: Vec::new() })
        );
    }

    #[test]
    fn empty_input_is_an_empty_document() {
        let doc = Document::with_delimiters(&b""[..], plain());
        assert!(doc.is_empty());
        assert_eq!(doc.len(), 0);
        assert_eq!(doc.as_bytes(), b"");
        assert_eq!(doc.segment(0), None);
    }

    #[test]
    fn as_bytes_is_the_input_unchanged() {
        let input = &b"ST*835~\n"[..];
        assert_eq!(Document::with_delimiters(input, plain()).as_bytes(), input);
    }

    #[test]
    fn segments_iterator_yields_every_segment_with_consecutive_indices() {
        let doc = Document::with_delimiters(&b"ST*835~SE*2~\n"[..], plain());
        let segments: Vec<_> = doc.segments().collect();
        assert_eq!(segments.len(), 3);
        assert_eq!(doc.segments().len(), 3, "ExactSizeIterator");
        for (i, segment) in segments.iter().enumerate() {
            assert_eq!(segment.index, i);
            assert_eq!(doc.segment(i).as_ref(), Some(segment));
        }
    }

    #[test]
    fn a_document_reference_can_be_iterated_with_for() {
        let doc = Document::with_delimiters(&b"ST*835~SE*2~"[..], plain());
        let mut ids = Vec::new();
        for segment in &doc {
            ids.push(segment.id);
        }
        assert_eq!(ids, vec![&b"ST"[..], b"SE"]);
    }

    #[test]
    fn parse_accepts_owned_bytes() {
        fn assert_static(_: &Document<'static>) {}
        let mut input = ISA.to_vec();
        input.extend_from_slice(b"GS*HP~");
        let doc = Document::parse(input).unwrap();
        assert_static(&doc);
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.segment(1).unwrap().id, b"GS");
    }

    #[test]
    fn into_owned_detaches_from_the_buffer() {
        fn assert_static(_: &Document<'static>) {}
        // A `Segment` borrows from the document, so the snapshot taken before
        // the move must own its bytes: otherwise `into_owned` and `drop` would
        // not compile while it is alive.
        fn snapshot(doc: &Document<'_>) -> Vec<(Vec<u8>, Vec<u8>, usize)> {
            doc.segments()
                .map(|s| (s.raw.to_vec(), s.id.to_vec(), s.elements.len()))
                .collect()
        }
        let buffer = b"ST*835~SE*2~".to_vec();
        let borrowed = Document::with_delimiters(&buffer[..], plain());
        let before = snapshot(&borrowed);
        let owned = borrowed.into_owned();
        drop(buffer);
        assert_static(&owned);
        assert_eq!(snapshot(&owned), before);
        assert_eq!(owned.as_bytes(), b"ST*835~SE*2~");
    }

    #[test]
    fn into_owned_of_an_owned_document_does_not_change_it() {
        let doc = Document::with_delimiters(b"ST*835~".to_vec(), plain());
        let owned = doc.clone().into_owned();
        assert_eq!(owned, doc);
    }
}
