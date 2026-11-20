use super::*;

fn syntax(release: Option<u8>) -> Syntax {
    Syntax {
        element: b'*',
        component: b':',
        end: SegmentEnd::new(b'~', release),
    }
}

/// One element of a test segment: its components (one for a simple element).
type Parts<'a> = &'a [&'a [u8]];

/// Writes `id` and `elements` into a vector, returning the output or the
/// refusal.
fn written(
    id: &[u8],
    elements: &[Parts<'_>],
    trailing: Trailing,
    release: Option<u8>,
) -> Result<Vec<u8>, Refused<Infallible>> {
    let mut out = Vec::new();
    let Ok(mut writer) = SegmentSerializer::begin(&mut out, syntax(release), trailing, id);
    for parts in elements {
        let Ok(()) = writer.element();
        for (at, part) in parts.iter().enumerate() {
            if at > 0 {
                writer.component();
            }
            writer.value(part)?;
        }
    }
    let Ok(()) = writer.end();
    Ok(out)
}

fn text(elements: &[Parts<'_>], trailing: Trailing) -> String {
    let out = written(b"NM1", elements, trailing, None).expect("no structural byte");
    String::from_utf8(out).expect("ASCII")
}

const TRAILING_EMPTY: &[Parts<'_>] = &[&[b"IL"], &[b""], &[b""]];
const EMPTY_BETWEEN: &[Parts<'_>] = &[&[b"IL"], &[b""], &[b"DOE"]];

#[test]
fn keeps_trailing_empty_elements() {
    assert_eq!(text(TRAILING_EMPTY, Trailing::Keep), "NM1*IL**~");
}

#[test]
fn trims_trailing_empty_elements() {
    assert_eq!(text(TRAILING_EMPTY, Trailing::Trim), "NM1*IL~");
}

#[test]
fn keeps_an_empty_element_between_values() {
    for trailing in [Trailing::Keep, Trailing::Trim] {
        assert_eq!(text(EMPTY_BETWEEN, trailing), "NM1*IL**DOE~");
    }
}

#[test]
fn keeps_trailing_empty_components() {
    let composite: &[Parts<'_>] = &[&[b"A", b"", b""]];
    assert_eq!(text(composite, Trailing::Keep), "NM1*A::~");
    assert_eq!(text(composite, Trailing::Trim), "NM1*A~");
}

#[test]
fn keeps_the_separators_of_an_element_in_order() {
    let elements: &[Parts<'_>] = &[&[b"A", b"", b""], &[b"B"]];
    assert_eq!(text(elements, Trailing::Keep), "NM1*A::*B~");
    assert_eq!(text(elements, Trailing::Trim), "NM1*A*B~");
}

#[test]
fn escapes_structural_bytes_with_a_release_character() {
    let out = written(b"X", &[&[b"a*b:c~d?e"]], Trailing::Keep, Some(b'?'));
    assert_eq!(out, Ok(b"X*a?*b?:c?~d??e~".to_vec()));
}

#[test]
fn refuses_a_structural_byte_without_a_release_character() {
    let mut out = Vec::new();
    let Ok(mut writer) = SegmentSerializer::begin(&mut out, syntax(None), Trailing::Keep, b"X");
    let Ok(()) = writer.element();
    assert_eq!(writer.value(b"a*b"), Err(Refused::Structural(b'*')));
    assert_eq!(out, b"X");
}

#[test]
fn writes_the_repetition_separator_as_data() {
    let delimiters = crate::Delimiters::new(b'*', b':', b'~').with_repetition(b'^');
    let mut out = Vec::new();
    let Ok(mut writer) =
        SegmentSerializer::begin(&mut out, delimiters.syntax(), Trailing::Keep, b"X");
    let Ok(()) = writer.element();
    assert_eq!(writer.value(b"a^b"), Ok(()));
    let Ok(()) = writer.end();
    assert_eq!(out, b"X*a^b~");
}
