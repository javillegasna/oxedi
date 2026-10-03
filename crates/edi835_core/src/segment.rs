//! A generic segment, borrowed from the input buffer.
//!
//! A segment is an id plus elements. It carries its own `raw` bytes and its
//! position in the stream so that every later layer can point back at the
//! exact input it came from.

use crate::delimiters::Delimiters;
use crate::element::{Element, split_raw};
use crate::frame::Frame;

/// One segment of the input. Everything borrows from the buffer except
/// values that needed unescaping (see [`Element`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment<'a> {
    /// 0-based position in the token stream.
    pub index: usize,
    /// Exact bytes this segment accounts for: leading trivia, body, terminator.
    pub raw: &'a [u8],
    /// Segment identifier (`ISA`, `CLP`, …). Empty for an empty or trivia-only frame.
    pub id: &'a [u8],
    /// Elements after the identifier, in X12 order: `elements[0]` is `XX01`.
    pub elements: Vec<Element<'a>>,
    /// `false` only when the input ended before this segment's terminator.
    pub terminated: bool,
}

impl<'a> Segment<'a> {
    /// Parses a frame's body into id and elements.
    pub fn parse(index: usize, frame: Frame<'a>, delims: &Delimiters) -> Self {
        let mut pieces = split_raw(frame.body, delims.element, delims.release).into_iter();
        let id = pieces.next().unwrap_or_default();
        let elements = pieces
            .map(|piece| Element::parse(piece, delims.component, delims.release))
            .collect();
        Self {
            index,
            raw: frame.raw,
            id,
            elements,
            terminated: frame.terminated,
        }
    }

    /// `true` when the frame had no content: `~~`, or trailing trivia.
    pub fn is_empty(&self) -> bool {
        self.id.is_empty() && self.elements.is_empty()
    }

    /// Element by its 1-based X12 position: `element(1)` is `XX01`.
    pub fn element(&self, position: usize) -> Option<&Element<'a>> {
        self.elements.get(position.checked_sub(1)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Frame};
    use std::borrow::Cow;

    fn frame(body: &[u8]) -> Frame<'_> {
        Frame {
            raw: body,
            body,
            terminated: true,
        }
    }

    #[test]
    fn parse_splits_id_and_elements() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(7, frame(b"ST*835*1234"), &delims);
        assert_eq!(segment.index, 7);
        assert_eq!(segment.id, b"ST");
        assert_eq!(
            segment.elements,
            vec![
                Element::Simple(Cow::Borrowed(b"835")),
                Element::Simple(Cow::Borrowed(b"1234"))
            ]
        );
        assert!(segment.terminated);
    }

    #[test]
    fn parse_of_empty_body_is_an_empty_segment() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b""), &delims);
        assert_eq!(segment.id, b"");
        assert!(segment.elements.is_empty());
        assert!(segment.is_empty());
    }

    #[test]
    fn parse_keeps_trailing_empty_elements() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"BPR*I**C"), &delims);
        assert_eq!(segment.elements.len(), 3);
        assert_eq!(segment.element(2).and_then(Element::simple), Some(&b""[..]));
    }

    #[test]
    fn element_is_one_based_like_x12() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"CLP*123*1"), &delims);
        assert_eq!(
            segment.element(1).and_then(Element::simple),
            Some(&b"123"[..])
        );
        assert_eq!(
            segment.element(2).and_then(Element::simple),
            Some(&b"1"[..])
        );
        assert_eq!(segment.element(0), None);
        assert_eq!(segment.element(3), None);
    }
}
