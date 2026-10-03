//! A generic segment, borrowed from the input buffer.
//!
//! A segment is an id plus elements. It carries its own `raw` bytes and its
//! position in the stream so that every later layer can point back at the
//! exact input it came from.

use std::fmt;
use std::io;

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

/// Why a segment could not be written.
#[derive(Debug)]
pub enum WriteError {
    /// The sink failed.
    Io(io::Error),
    /// A value contains a delimiter and no release byte is configured to escape it.
    DelimiterInValue {
        /// The offending byte.
        byte: u8,
        /// `0` for the segment id, otherwise the 1-based element position.
        element: usize,
        /// 1-based component position when the element is a composite.
        component: Option<usize>,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Io(e) => write!(f, "write failed: {e}"),
            WriteError::DelimiterInValue {
                byte,
                element,
                component,
            } => {
                match (element, component) {
                    (0, _) => write!(f, "segment id")?,
                    (element, None) => write!(f, "element {element}")?,
                    (element, Some(component)) => {
                        write!(f, "element {element} component {component}")?;
                    }
                }
                write!(
                    f,
                    " contains delimiter byte 0x{byte:02X} and no release byte is configured"
                )
            }
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WriteError::Io(e) => Some(e),
            WriteError::DelimiterInValue { .. } => None,
        }
    }
}

impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        WriteError::Io(e)
    }
}

impl Segment<'_> {
    /// Writes `id`, `elements` and the terminator using `delims`. Delimiter
    /// bytes inside values are escaped with the release byte; without one they
    /// are an error. The id is written verbatim, so an id holding a delimiter
    /// or the release byte is an error too: escaping it would change the id
    /// that reads back. Trivia in `raw` is not written: this rebuilds from data.
    pub fn write_to<W: io::Write>(
        &self,
        delims: &Delimiters,
        out: &mut W,
    ) -> Result<(), WriteError> {
        if let Some(&byte) = self.id.iter().find(|&&byte| delims.is_special(byte)) {
            return Err(WriteError::DelimiterInValue {
                byte,
                element: 0,
                component: None,
            });
        }
        out.write_all(self.id)?;
        for (position, element) in (1..).zip(&self.elements) {
            out.write_all(&[delims.element])?;
            match element {
                Element::Simple(value) => write_value(value, delims, out, position, None)?,
                Element::Composite(values) => {
                    for (component, value) in (1..).zip(values) {
                        if component > 1 {
                            out.write_all(&[delims.component])?;
                        }
                        write_value(value, delims, out, position, Some(component))?;
                    }
                }
            }
        }
        out.write_all(&[delims.segment])?;
        Ok(())
    }
}

/// Writes one value; `element` and `component` locate it for the error.
fn write_value<W: io::Write>(
    value: &[u8],
    delims: &Delimiters,
    out: &mut W,
    element: usize,
    component: Option<usize>,
) -> Result<(), WriteError> {
    if !value.iter().any(|&byte| delims.is_special(byte)) {
        return Ok(out.write_all(value)?);
    }
    let Some(release) = delims.release else {
        let byte = value
            .iter()
            .copied()
            .find(|&byte| delims.is_special(byte))
            .unwrap_or_default();
        return Err(WriteError::DelimiterInValue {
            byte,
            element,
            component,
        });
    };
    for &byte in value {
        if delims.is_special(byte) {
            out.write_all(&[release, byte])?;
        } else {
            out.write_all(&[byte])?;
        }
    }
    Ok(())
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

    fn written(segment: &Segment<'_>, delims: &Delimiters) -> Result<Vec<u8>, WriteError> {
        let mut out = Vec::new();
        segment.write_to(delims, &mut out)?;
        Ok(out)
    }

    #[test]
    fn write_to_rebuilds_id_elements_and_terminator() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"SVC*HC:99213*100**12"), &delims);
        assert_eq!(
            written(&segment, &delims).unwrap(),
            b"SVC*HC:99213*100**12~"
        );
    }

    #[test]
    fn write_to_escapes_delimiters_inside_values_when_release_is_set() {
        let delims = Delimiters::new(b'*', b':', b'~').with_release(b'?');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"N1",
            elements: vec![Element::Simple(Cow::Owned(b"A*B~C?D".to_vec()))],
            terminated: true,
        };
        assert_eq!(written(&segment, &delims).unwrap(), b"N1*A?*B?~C??D~");
    }

    #[test]
    fn write_to_without_release_rejects_a_delimiter_in_a_value() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"N1",
            elements: vec![Element::Simple(Cow::Borrowed(b"A*B"))],
            terminated: true,
        };
        assert!(matches!(
            written(&segment, &delims),
            Err(WriteError::DelimiterInValue {
                byte: b'*',
                element: 1,
                component: None
            })
        ));
    }

    #[test]
    fn write_to_reports_the_element_and_component_holding_the_delimiter() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"SVC",
            elements: vec![
                Element::Simple(Cow::Borrowed(b"1")),
                Element::Simple(Cow::Borrowed(b"2")),
                Element::Composite(vec![Cow::Borrowed(b"HC"), Cow::Borrowed(b"A~B")]),
            ],
            terminated: true,
        };
        assert!(matches!(
            written(&segment, &delims),
            Err(WriteError::DelimiterInValue {
                byte: b'~',
                element: 3,
                component: Some(2)
            })
        ));
    }

    #[test]
    fn write_to_does_not_write_trivia_from_raw() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(
            0,
            Frame {
                raw: b"\nSE*2*1~",
                body: b"SE*2*1",
                terminated: true,
            },
            &delims,
        );
        assert_eq!(written(&segment, &delims).unwrap(), b"SE*2*1~");
    }

    #[test]
    fn delimiter_in_value_displays_where_the_byte_is() {
        assert_eq!(
            WriteError::DelimiterInValue {
                byte: b'*',
                element: 3,
                component: Some(2)
            }
            .to_string(),
            "element 3 component 2 contains delimiter byte 0x2A and no release byte is configured"
        );
        assert_eq!(
            WriteError::DelimiterInValue {
                byte: b'*',
                element: 1,
                component: None
            }
            .to_string(),
            "element 1 contains delimiter byte 0x2A and no release byte is configured"
        );
        assert_eq!(
            WriteError::DelimiterInValue {
                byte: b'*',
                element: 0,
                component: None
            }
            .to_string(),
            "segment id contains delimiter byte 0x2A and no release byte is configured"
        );
    }

    #[test]
    fn io_error_displays_the_sink_failure() {
        let err = WriteError::Io(io::Error::other("disk full"));
        assert_eq!(err.to_string(), "write failed: disk full");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn write_to_rejects_an_id_containing_a_delimiter_or_release_byte() {
        let delims = Delimiters::new(b'*', b':', b'~');
        for (id, byte) in [(&b"A*B"[..], b'*'), (b"A~B", b'~'), (b"A:B", b':')] {
            let segment = Segment {
                index: 0,
                raw: b"",
                id,
                elements: vec![],
                terminated: true,
            };
            assert!(
                matches!(written(&segment, &delims), Err(WriteError::DelimiterInValue { byte: b, element: 0, component: None }) if b == byte),
                "id {:?}",
                id
            );
        }
        let with_release = delims.with_release(b'?');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"AB?",
            elements: vec![],
            terminated: true,
        };
        assert!(matches!(
            written(&segment, &with_release),
            Err(WriteError::DelimiterInValue {
                byte: b'?',
                element: 0,
                component: None
            })
        ));
    }
}
