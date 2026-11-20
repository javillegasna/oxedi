//! The syntax of one written segment: where the separators, the terminator
//! and the release byte go.
//!
//! A [`SegmentSerializer`] never looks ahead. A separator is remembered as
//! pending when an element or component starts, and written only when a
//! non-empty value arrives; at the end the pending ones are written too, so
//! trailing empty elements and components are kept. Nothing is allocated
//! per segment, element or value: values are written in runs between the
//! bytes that need the release byte.

use std::convert::Infallible;
use std::io;

use crate::frame::SegmentEnd;

/// Where the bytes of a segment go.
pub(crate) trait Output {
    /// What a failed write reports.
    type Error;

    /// Appends `bytes`.
    fn put(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

impl Output for Vec<u8> {
    type Error = Infallible;

    fn put(&mut self, bytes: &[u8]) -> Result<(), Infallible> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

/// An [`io::Write`] used as an [`Output`].
pub(crate) struct Sink<'w, W: io::Write>(pub(crate) &'w mut W);

impl<W: io::Write> Output for Sink<'_, W> {
    type Error = io::Error;

    fn put(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.0.write_all(bytes)
    }
}

/// The bytes a segment is written with. The terminator and the release byte
/// are the [`SegmentEnd`] the reading side already names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Syntax {
    pub(crate) element: u8,
    pub(crate) component: u8,
    pub(crate) end: SegmentEnd,
}

impl Syntax {
    /// `true` when `byte` cannot be written as data: the bytes the tokenizer
    /// splits on, plus the release byte itself. The repetition separator is
    /// not split on, so it is data.
    pub(crate) fn is_structural(&self, byte: u8) -> bool {
        byte == self.element
            || byte == self.component
            || byte == self.end.terminator
            || self.end.release == Some(byte)
    }

    /// The first structural byte of `bytes`, if any.
    pub(crate) fn first_structural(&self, bytes: &[u8]) -> Option<u8> {
        bytes.iter().copied().find(|&byte| self.is_structural(byte))
    }
}

/// Writes one segment into an [`Output`], one call per id, element,
/// component and value.
pub(crate) struct SegmentSerializer<'o, O: Output> {
    out: &'o mut O,
    syntax: Syntax,
    /// Element separators owed before the next value, empty elements included.
    elements: usize,
    /// Component separators owed before the next value, within the current element.
    components: usize,
}

impl<'o, O: Output> SegmentSerializer<'o, O> {
    /// Starts a segment by writing `id` as it is: the caller has checked it
    /// holds no structural byte.
    pub(crate) fn begin(out: &'o mut O, syntax: Syntax, id: &[u8]) -> Result<Self, O::Error> {
        out.put(id)?;
        Ok(Self {
            out,
            syntax,
            elements: 0,
            components: 0,
        })
    }

    /// Starts the next element.
    pub(crate) fn element(&mut self) -> Result<(), O::Error> {
        // The separators come out in order: those of the elements up to the
        // one that ends here, then its components.
        if self.components > 0 {
            self.flush()?;
        }
        self.elements += 1;
        Ok(())
    }

    /// Starts the next component of the current element; the first component
    /// needs no call.
    pub(crate) fn component(&mut self) {
        self.components += 1;
    }

    /// Writes a value of the current element or component. With a release
    /// byte each structural byte is escaped with it; without one the bytes are
    /// written as they are, so the caller checks them first. An empty value
    /// writes nothing.
    pub(crate) fn value(&mut self, bytes: &[u8]) -> Result<(), O::Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.flush()?;
        match self.syntax.end.release {
            Some(release) => self.put_escaped(release, bytes),
            None => self.out.put(bytes),
        }
    }

    /// Writes `bytes` in runs, each structural byte after a `release`.
    fn put_escaped(&mut self, release: u8, mut rest: &[u8]) -> Result<(), O::Error> {
        while let Some(at) = rest
            .iter()
            .position(|&byte| self.syntax.is_structural(byte))
        {
            let (run, tail) = rest.split_at(at);
            self.out.put(run)?;
            let Some((&byte, after)) = tail.split_first() else {
                break;
            };
            self.out.put(&[release, byte])?;
            rest = after;
        }
        self.out.put(rest)
    }

    /// Ends the segment: the pending separators are written, then the
    /// terminator.
    pub(crate) fn end(mut self) -> Result<(), O::Error> {
        self.flush()?;
        self.out.put(&[self.syntax.end.terminator])
    }

    /// Writes the pending element separators, then the pending component ones.
    fn flush(&mut self) -> Result<(), O::Error> {
        for _ in 0..std::mem::take(&mut self.elements) {
            self.out.put(&[self.syntax.element])?;
        }
        for _ in 0..std::mem::take(&mut self.components) {
            self.out.put(&[self.syntax.component])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
