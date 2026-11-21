use super::*;
use crate::frame::SegmentEnd;

fn syntax(release: Option<u8>) -> Syntax {
    Syntax {
        element: b'*',
        component: b':',
        end: SegmentEnd::new(b'~', release),
    }
}

/// One element of a test segment: its components (one for a simple element).
type Parts<'a> = &'a [&'a [u8]];

/// Writes segment `NM1` with `elements` into a vector and returns the bytes written.
fn written(syntax: Syntax, trailing: Trailing, elements: &[Parts<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let Ok(mut writer) = SegmentSerializer::begin(&mut out, syntax, trailing, b"NM1");
    for parts in elements {
        let Ok(()) = writer.element();
        for (at, part) in parts.iter().enumerate() {
            if at > 0 {
                writer.component();
            }
            let Ok(()) = writer.value(part);
        }
    }
    let Ok(()) = writer.end();
    out
}

/// Each row: the case, its elements, and the segment written from them when
/// trailing separators are kept and when they are trimmed.
const LAYOUTS: &[(&str, &[Parts<'_>], &str, &str)] = &[
    (
        "trailing empty elements",
        &[&[b"IL"], &[b""], &[b""]],
        "NM1*IL**~",
        "NM1*IL~",
    ),
    (
        "an empty element between values",
        &[&[b"IL"], &[b""], &[b"DOE"]],
        "NM1*IL**DOE~",
        "NM1*IL**DOE~",
    ),
    (
        "trailing empty components",
        &[&[b"A", b"", b""]],
        "NM1*A::~",
        "NM1*A~",
    ),
    (
        "the separators of an element in order",
        &[&[b"A", b"", b""], &[b"B"]],
        "NM1*A::*B~",
        "NM1*A*B~",
    ),
    (
        "an empty first component",
        &[&[b"", b"B"]],
        "NM1*:B~",
        "NM1*:B~",
    ),
    (
        "an empty composite then a value",
        &[&[b"", b""], &[b"B"]],
        "NM1*:*B~",
        "NM1**B~",
    ),
    ("no elements", &[], "NM1~", "NM1~"),
];

#[test]
fn lays_out_the_separators() {
    for &(case, elements, kept, trimmed) in LAYOUTS {
        for (trailing, expected) in [(Trailing::Keep, kept), (Trailing::Trim, trimmed)] {
            let out = written(syntax(None), trailing, elements);
            assert_eq!(
                String::from_utf8_lossy(&out),
                expected,
                "{case} ({trailing:?})"
            );
        }
    }
}

#[test]
fn escapes_structural_bytes_with_a_release_character() {
    let out = written(syntax(Some(b'?')), Trailing::Keep, &[&[b"a*b:c~d?e"]]);
    assert_eq!(out, b"NM1*a?*b?:c?~d??e~");
}

#[test]
fn writes_a_structural_byte_as_it_is_without_a_release_character() {
    let out = written(syntax(None), Trailing::Keep, &[&[b"a*b"]]);
    assert_eq!(out, b"NM1*a*b~");
}

#[test]
fn writes_the_repetition_separator_as_data() {
    let delimiters = crate::Delimiters::new(b'*', b':', b'~').with_repetition(b'^');
    let out = written(delimiters.syntax(), Trailing::Keep, &[&[b"a^b"]]);
    assert_eq!(out, b"NM1*a^b~");
}
