//! A generic segment, borrowed from the input buffer.
//!
//! A segment is an id plus elements. It carries its own `raw` bytes and its
//! position in the stream so that every later layer can point back at the
//! exact input it came from.

use std::fmt;
use std::io;

use crate::delimiters::Delimiters;
use crate::element::{Element, Pieces};
use crate::frame::Frame;

mod serializer;

pub(crate) use serializer::{Output, Refused, SegmentSerializer, Sink, Syntax, Trailing};

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
        let mut segment = Segment::empty();
        segment.reparse(index, frame, delims);
        segment
    }

    /// A segment with no id, no elements and no bytes, to be filled by
    /// [`Segment::reparse`].
    pub(crate) const fn empty() -> Self {
        Self {
            index: 0,
            raw: &[],
            id: &[],
            elements: Vec::new(),
            terminated: false,
        }
    }

    /// Overwrites this segment with the parse of `frame`, reusing the element
    /// vector and the composite vectors already in it. Every field is
    /// replaced, and elements past the new count are dropped.
    pub(crate) fn reparse(&mut self, index: usize, frame: Frame<'a>, delims: &Delimiters) {
        let mut pieces = Pieces::new(frame.body, delims.element, delims.release);
        self.index = index;
        self.raw = frame.raw;
        self.id = pieces.next().unwrap_or_default();
        self.terminated = frame.terminated;
        let mut count = 0;
        for piece in pieces {
            match self.elements.get_mut(count) {
                Some(slot) => slot.reparse(piece, delims.component, delims.release),
                None => self
                    .elements
                    .push(Element::parse(piece, delims.component, delims.release)),
            }
            count += 1;
        }
        self.elements.truncate(count);
    }

    /// `true` when the frame had no content: `~~`, or trailing trivia.
    pub fn is_empty(&self) -> bool {
        self.id.is_empty() && self.elements.is_empty()
    }

    /// Element by its 1-based X12 position: `element(1)` is `XX01`.
    pub fn element(&self, position: usize) -> Option<&Element<'a>> {
        self.elements.get(position.checked_sub(1)?)
    }

    /// The value of an element, or of one of its components, as written.
    /// Component 1 of a simple element is the element itself, and a
    /// composite addressed without a component gives its first component.
    /// `None` when the segment does not have it.
    #[inline]
    pub(crate) fn leaf(&self, element: usize, component: Option<usize>) -> Option<&[u8]> {
        match (self.element(element)?, component) {
            (Element::Simple(value), None | Some(1)) => Some(value),
            (Element::Simple(_), Some(_)) => None,
            (Element::Composite(parts), component) => component
                .unwrap_or(1)
                .checked_sub(1)
                .and_then(|at| parts.get(at))
                .map(AsRef::as_ref),
        }
    }

    /// The bytes of one component of an element, as written. `element` and
    /// `component` are 1-based; component 1 of a simple element is the
    /// element itself. `None` when the segment does not have that place.
    pub fn component(&self, element: usize, component: usize) -> Option<&[u8]> {
        self.leaf(element, Some(component))
    }

    /// The whole text of an element: a composite's components are written
    /// back into `joined` with `separator` between them. `None` when the
    /// segment does not have the element.
    pub(crate) fn text<'s>(
        &'s self,
        element: usize,
        separator: u8,
        joined: &'s mut Vec<u8>,
    ) -> Option<&'s [u8]> {
        match self.element(element)? {
            Element::Simple(value) => Some(value),
            Element::Composite(parts) => {
                joined.clear();
                for (at, part) in parts.iter().enumerate() {
                    if at > 0 {
                        joined.push(separator);
                    }
                    joined.extend_from_slice(part);
                }
                Some(joined)
            }
        }
    }

    /// `true` when every `(position, value)` holds: the element at the
    /// position is simple and equal to the value. Trigger and column
    /// conditions read this way on purpose: a composite never satisfies a
    /// condition, whereas [`leaf`](Self::leaf), which qualifiers read, takes
    /// a composite's first component.
    #[inline]
    pub(crate) fn holds(&self, conditions: &[(usize, Vec<u8>)]) -> bool {
        conditions.iter().all(|(position, value)| {
            self.element(*position).and_then(Element::simple) == Some(value.as_slice())
        })
    }
}

/// Why a segment could not be written.
#[derive(Debug)]
pub enum SegmentWriteError {
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

impl fmt::Display for SegmentWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentWriteError::Io(e) => write!(f, "write failed: {e}"),
            SegmentWriteError::DelimiterInValue {
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

impl std::error::Error for SegmentWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // `Io` prints the I/O error in full, so the chain stops here
        // instead of printing it a second time.
        match self {
            SegmentWriteError::Io(_) | SegmentWriteError::DelimiterInValue { .. } => None,
        }
    }
}

impl From<io::Error> for SegmentWriteError {
    fn from(e: io::Error) -> Self {
        SegmentWriteError::Io(e)
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
    ) -> Result<(), SegmentWriteError> {
        let syntax = delims.syntax();
        if let Some(byte) = syntax.first_structural(self.id) {
            return Err(located(Refused::Structural(byte), 0, None));
        }
        let mut sink = Sink(out);
        let mut writer = SegmentSerializer::begin(&mut sink, syntax, Trailing::Keep, self.id)?;
        for (position, element) in (1..).zip(&self.elements) {
            writer.element()?;
            write_element(&mut writer, element, position)?;
        }
        Ok(writer.end()?)
    }
}

/// Writes the values of one element; `position` locates them for the error.
fn write_element<O: Output<Error = io::Error>>(
    writer: &mut SegmentSerializer<'_, O>,
    element: &Element<'_>,
    position: usize,
) -> Result<(), SegmentWriteError> {
    let values = match element {
        Element::Simple(value) => {
            return writer.value(value).map_err(|r| located(r, position, None));
        }
        Element::Composite(values) => values,
    };
    for (component, value) in (1..).zip(values) {
        if component > 1 {
            writer.component();
        }
        writer
            .value(value)
            .map_err(|refused| located(refused, position, Some(component)))?;
    }
    Ok(())
}

/// The error for a refused value at `element` (`0` is the id) and `component`.
fn located(
    refused: Refused<io::Error>,
    element: usize,
    component: Option<usize>,
) -> SegmentWriteError {
    match refused {
        Refused::Output(error) => SegmentWriteError::Io(error),
        Refused::Structural(byte) => SegmentWriteError::DelimiterInValue {
            byte,
            element,
            component,
        },
    }
}

#[cfg(test)]
mod tests;
