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
        if find_unescaped(raw, component, release).is_some() {
            let values = split_raw(raw, component, release)
                .into_iter()
                .map(|piece| unescape(piece, release))
                .collect();
            Element::Composite(values)
        } else {
            Element::Simple(unescape(raw, release))
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
pub fn split_raw(mut raw: &[u8], sep: u8, release: Option<u8>) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    while let Some(at) = find_unescaped(raw, sep, release) {
        pieces.push(&raw[..at]);
        raw = &raw[at + 1..];
    }
    pieces.push(raw);
    pieces
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
