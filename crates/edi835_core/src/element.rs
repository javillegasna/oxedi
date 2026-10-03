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
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn simple_value_borrows_from_the_input() {
        let element = Element::parse(b"835", b':', None);
        assert_eq!(element, Element::Simple(Cow::Borrowed(b"835")));
        assert!(matches!(element, Element::Simple(Cow::Borrowed(_))));
    }

    #[test]
    fn composite_splits_on_the_component_separator() {
        let element = Element::parse(b"HC:99213", b':', None);
        assert_eq!(
            element,
            Element::Composite(vec![Cow::Borrowed(b"HC"), Cow::Borrowed(b"99213")])
        );
    }

    #[test]
    fn composite_keeps_empty_components() {
        let element = Element::parse(b"HC::X", b':', None);
        assert_eq!(
            element,
            Element::Composite(vec![
                Cow::Borrowed(b"HC"),
                Cow::Borrowed(b""),
                Cow::Borrowed(b"X")
            ])
        );
    }

    #[test]
    fn release_makes_the_component_separator_literal_and_owns_the_value() {
        let element = Element::parse(b"A?:B", b':', Some(b'?'));
        assert_eq!(element, Element::Simple(Cow::Owned(b"A:B".to_vec())));
        assert!(matches!(element, Element::Simple(Cow::Owned(_))));
    }

    #[test]
    fn release_configured_but_absent_still_borrows() {
        let element = Element::parse(b"ABC", b':', Some(b'?'));
        assert!(matches!(element, Element::Simple(Cow::Borrowed(_))));
    }

    #[test]
    fn escaped_release_is_a_literal_release_byte() {
        assert_eq!(
            unescape(b"A??B", Some(b'?')),
            Cow::<[u8]>::Owned(b"A?B".to_vec())
        );
    }

    #[test]
    fn dangling_release_in_value_is_dropped() {
        assert_eq!(
            unescape(b"AB?", Some(b'?')),
            Cow::<[u8]>::Owned(b"AB".to_vec())
        );
    }

    #[test]
    fn split_raw_keeps_empty_pieces_and_never_returns_none() {
        assert_eq!(split_raw(b"a::b", b':', None), vec![&b"a"[..], b"", b"b"]);
        assert_eq!(split_raw(b"", b':', None), vec![&b""[..]]);
    }

    #[test]
    fn simple_accessor_returns_none_for_composites() {
        assert_eq!(Element::parse(b"X", b':', None).simple(), Some(&b"X"[..]));
        assert_eq!(Element::parse(b"X:Y", b':', None).simple(), None);
    }
}
