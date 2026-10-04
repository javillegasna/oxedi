use super::*;
use crate::{Delimiters, Element, IsaError};
use std::borrow::Cow;

const ISA: &[u8] =
    b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

fn plain() -> Delimiters {
    Delimiters::new(b'*', b':', b'~')
}

fn concat_raw(segments: &[Segment<'_>]) -> Vec<u8> {
    segments
        .iter()
        .flat_map(|s| s.raw.iter().copied())
        .collect()
}

#[test]
fn with_delimiters_yields_segments_in_order_with_consecutive_indices() {
    let segments: Vec<_> = Tokenizer::with_delimiters(b"ST*835*1~SE*2*1~", plain()).collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].id, b"ST");
    assert_eq!(segments[1].id, b"SE");
    assert_eq!(segments[0].index, 0);
    assert_eq!(segments[1].index, 1);
    assert_eq!(
        segments[0].elements,
        vec![
            Element::Simple(Cow::Borrowed(b"835")),
            Element::Simple(Cow::Borrowed(b"1"))
        ]
    );
}

#[test]
fn trailing_newline_becomes_an_empty_unterminated_segment() {
    let input = b"ST*835~\n";
    let segments: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
    assert_eq!(segments.len(), 2);
    assert!(segments[1].is_empty());
    assert!(!segments[1].terminated);
    assert_eq!(segments[1].raw, b"\n");
    assert_eq!(concat_raw(&segments), input);
}

#[test]
fn crlf_trivia_is_preserved() {
    let input = b"ST*835~\r\nSE*2*1~\r\n";
    let segments: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
    let ids: Vec<&[u8]> = segments.iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![&b"ST"[..], b"SE", b""]);
    assert_eq!(segments[1].raw, b"\r\nSE*2*1~");
    assert_eq!(concat_raw(&segments), input);
}

#[test]
fn empty_segment_is_preserved_in_place() {
    let segments: Vec<_> = Tokenizer::with_delimiters(b"ST~~SE~", plain()).collect();
    assert_eq!(segments.len(), 3);
    assert!(segments[1].is_empty());
    assert!(segments[1].terminated);
    assert_eq!(segments[1].raw, b"~");
}

#[test]
fn truncated_file_keeps_partial_segment() {
    let segments: Vec<_> = Tokenizer::with_delimiters(b"ST*835~SE*2", plain()).collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].id, b"SE");
    assert!(!segments[1].terminated);
}

#[test]
fn foreign_delimiter_byte_is_data() {
    let delims = Delimiters::new(b'|', b':', b'~');
    let segments: Vec<_> = Tokenizer::with_delimiters(b"REF|F2|LC*A438D~", delims).collect();
    assert_eq!(
        segments[0].element(2).and_then(Element::simple),
        Some(&b"LC*A438D"[..])
    );
}

#[test]
fn non_utf8_bytes_pass_through() {
    let segments: Vec<_> = Tokenizer::with_delimiters(b"NM1*QC*1*P\xC9REZ~", plain()).collect();
    assert_eq!(
        segments[0].element(3).and_then(Element::simple),
        Some(&b"P\xC9REZ"[..])
    );
}

#[test]
fn composite_element_inside_a_segment() {
    let segments: Vec<_> = Tokenizer::with_delimiters(b"SVC*HC:99213*100~", plain()).collect();
    assert_eq!(
        segments[0].element(1),
        Some(&Element::Composite(vec![
            Cow::Borrowed(b"HC"),
            Cow::Borrowed(b"99213")
        ]))
    );
}

#[test]
fn new_reads_delimiters_from_the_isa() {
    let mut input = ISA.to_vec();
    input.extend_from_slice(b"GS*HP:X~");
    let tokenizer = Tokenizer::new(&input).unwrap();
    assert_eq!(tokenizer.delimiters().component, b':');
    let segments: Vec<_> = tokenizer.collect();
    assert_eq!(segments[0].id, b"ISA");
    assert_eq!(
        segments[1].element(1),
        Some(&Element::Composite(vec![
            Cow::Borrowed(b"HP"),
            Cow::Borrowed(b"X")
        ]))
    );
}

#[test]
fn new_tolerates_trivia_before_the_isa() {
    let mut input = b"\r\n".to_vec();
    input.extend_from_slice(ISA);
    let segments: Vec<_> = Tokenizer::new(&input).unwrap().collect();
    assert_eq!(segments[0].id, b"ISA");
    assert!(segments[0].raw.starts_with(b"\r\n"));
}

#[test]
fn new_reads_past_a_byte_order_mark_and_keeps_it_in_the_first_raw() {
    let mut input = b"\xEF\xBB\xBF\n".to_vec();
    input.extend_from_slice(ISA);
    input.extend_from_slice(b"GS*HP~");
    let segments: Vec<_> = Tokenizer::new(&input).unwrap().collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].id, b"ISA");
    assert_eq!(segments[0].index, 0);
    assert!(segments[0].raw.starts_with(b"\xEF\xBB\xBF\nISA"));
    assert_eq!(segments[1].id, b"GS");
    assert_eq!(concat_raw(&segments), input);
}

#[test]
fn new_names_the_trivia_it_skipped_when_no_isa_follows() {
    let message = |input: &[u8]| Tokenizer::new(input).err().map(|error| error.to_string());
    assert_eq!(
        message(b"\xEF\xBB\xBF").as_deref(),
        Some("input does not start with an ISA segment (input holds only a UTF-8 byte order mark)")
    );
    assert_eq!(
        message(b"\n\r\n ").as_deref(),
        Some("input does not start with an ISA segment (input holds only 4 bytes of whitespace)")
    );
    assert_eq!(
        message(b"\xEF\xBB\xBFGS*HP~").as_deref(),
        Some(
            "input does not start with an ISA segment (found bytes [47 53 2a 48 50 7e] after skipping a UTF-8 byte order mark)"
        )
    );
}

#[test]
fn new_fails_without_an_isa() {
    assert_eq!(
        Tokenizer::new(b"ST*835~").err(),
        Some(IsaError::NotIsa {
            found: b"ST*835~".to_vec(),
            byte_order_mark: false,
            whitespace: 0,
        })
    );
    assert_eq!(
        Tokenizer::new(b"").err(),
        Some(IsaError::NotIsa {
            found: Vec::new(),
            byte_order_mark: false,
            whitespace: 0,
        })
    );
}

#[test]
fn release_can_be_injected_through_with_delimiters() {
    let delims = Delimiters::from_isa(ISA).unwrap().with_release(b'?');
    let mut input = ISA.to_vec();
    input.extend_from_slice(b"N1*PR*A?*B~");
    let segments: Vec<_> = Tokenizer::with_delimiters(&input, delims).collect();
    assert_eq!(
        segments[1].element(2).and_then(Element::simple),
        Some(&b"A*B"[..])
    );
}
