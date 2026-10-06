//! Element values of a segment.
//!
//! Splitting honours an optional release byte. Values borrow from the input
//! unless a release byte had to be removed, in which case they are owned.

use std::borrow::Cow;

use crate::frame::find_unescaped;

/// A value: borrowed from the buffer, or owned after unescaping.
pub type Value<'a> = Cow<'a, [u8]>;

/// One element of a segment: simple, or composite when it contains an
/// unescaped component separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    /// A single value.
    Simple(Value<'a>),
    /// Values separated by the component separator, in order.
    Composite(Vec<Value<'a>>),
}

impl<'a> Element<'a> {
    /// Parses one raw element (the bytes between two element separators).
    pub fn parse(raw: &'a [u8], component: u8, release: Option<u8>) -> Self {
        let mut element = Element::Simple(Value::Borrowed(&[]));
        element.reparse(raw, component, release);
        element
    }

    /// Overwrites this element with the parse of `raw`, keeping the
    /// allocation of a composite when the new value is a composite too.
    pub(crate) fn reparse(&mut self, raw: &'a [u8], component: u8, release: Option<u8>) {
        let mut pieces = Pieces::new(raw, component, release);
        let first = pieces.next().unwrap_or_default();
        if pieces.is_done() {
            *self = Element::Simple(unescape(first, release));
            return;
        }
        let values = pieces.map(|piece| unescape(piece, release));
        match self {
            Element::Composite(old) => {
                old.clear();
                old.push(unescape(first, release));
                old.extend(values);
            }
            Element::Simple(_) => {
                let mut new = vec![unescape(first, release)];
                new.extend(values);
                *self = Element::Composite(new);
            }
        }
    }

    /// The value of a simple element; `None` for a composite.
    pub fn simple(&self) -> Option<&[u8]> {
        match self {
            Element::Simple(value) => Some(value),
            Element::Composite(_) => None,
        }
    }
}

/// Splits `raw` on every unescaped `sep`. Pieces are not unescaped. Always
/// returns at least one piece, so the caller can take the first as an id.
pub fn split_raw(raw: &[u8], sep: u8, release: Option<u8>) -> Vec<&[u8]> {
    Pieces::new(raw, sep, release).collect()
}

/// The pieces [`split_raw`] returns, yielded one at a time without a buffer.
#[derive(Debug, Clone)]
pub(crate) struct Pieces<'a> {
    /// What is left to split; `None` once the last piece was yielded.
    rest: Option<&'a [u8]>,
    sep: u8,
    release: Option<u8>,
}

impl<'a> Pieces<'a> {
    pub(crate) const fn new(raw: &'a [u8], sep: u8, release: Option<u8>) -> Self {
        Self {
            rest: Some(raw),
            sep,
            release,
        }
    }

    /// `true` once every piece was yielded.
    pub(crate) const fn is_done(&self) -> bool {
        self.rest.is_none()
    }
}

impl<'a> Iterator for Pieces<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let raw = self.rest?;
        match find_unescaped(raw, self.sep, self.release) {
            Some(at) => {
                // `at` is the position of a byte inside `raw`, so both ranges
                // are in bounds and the separator is the byte between them.
                self.rest = raw.get(at + 1..);
                raw.get(..at)
            }
            None => {
                self.rest = None;
                Some(raw)
            }
        }
    }
}

/// Removes release bytes, keeping the byte each one protects. Borrows when
/// there is nothing to remove. A release byte at the very end protects nothing
/// and is dropped.
pub fn unescape(raw: &[u8], release: Option<u8>) -> Value<'_> {
    match release {
        Some(release) if raw.contains(&release) => {
            let mut out = Vec::with_capacity(raw.len());
            let mut literal_next = false;
            for &byte in raw {
                if literal_next {
                    out.push(byte);
                    literal_next = false;
                } else if byte == release {
                    literal_next = true;
                } else {
                    out.push(byte);
                }
            }
            Cow::Owned(out)
        }
        _ => Cow::Borrowed(raw),
    }
}

#[cfg(test)]
mod tests;
