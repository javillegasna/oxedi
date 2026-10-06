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
mod tests;
