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
