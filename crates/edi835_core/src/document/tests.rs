use super::*;
use crate::{Delimiters, Tokenizer};

const ISA: &[u8] =
    b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

fn plain() -> Delimiters {
    Delimiters::new(b'*', b':', b'~')
}

#[test]
fn with_delimiters_indexes_every_frame() {
    let doc = Document::with_delimiters(&b"ST*835~\nSE*2~\n"[..], plain());
    assert_eq!(doc.len(), 3);
    assert_eq!(doc.segment(0).unwrap().id, b"ST");
    assert_eq!(doc.segment(1).unwrap().id, b"SE");
    assert!(doc.segment(2).unwrap().is_empty());
    assert_eq!(doc.segment(3), None);
}

#[test]
fn spans_partition_the_bytes() {
    let input = &b"\nST*835~\n\nSE~X"[..];
    let doc = Document::with_delimiters(input, plain());
    let spans = doc.spans();
    assert_eq!(
        spans[0],
        Span {
            raw: 0..8,
            body: 1..7,
            terminated: true
        }
    );
    assert_eq!(
        spans[1],
        Span {
            raw: 8..13,
            body: 10..12,
            terminated: true
        }
    );
    assert_eq!(
        spans[2],
        Span {
            raw: 13..14,
            body: 13..14,
            terminated: false
        }
    );
    let mut next = 0;
    for span in spans {
        assert_eq!(span.raw.start, next, "spans must be contiguous");
        assert!(span.raw.start <= span.body.start && span.body.end <= span.raw.end);
        next = span.raw.end;
    }
    assert_eq!(next, input.len(), "spans must cover the whole input");

    let tail_only = Document::with_delimiters(&b"ST~\n"[..], plain());
    assert_eq!(
        tail_only.spans()[1],
        Span {
            raw: 3..4,
            body: 4..4,
            terminated: false
        }
    );
}

#[test]
fn segment_equals_tokenizer_output() {
    let input = &b"ISA*00~\r\nSVC*HC:99213*100**12~CAS*CO*45~~"[..];
    let doc = Document::with_delimiters(input, plain());
    let expected: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
    assert_eq!(doc.len(), expected.len());
    for (i, segment) in expected.iter().enumerate() {
        assert_eq!(doc.segment(i).as_ref(), Some(segment), "segment {i}");
    }
}

#[test]
fn parse_reads_delimiters_from_the_isa_and_tolerates_leading_trivia() {
    let mut input = b"\r\n".to_vec();
    input.extend_from_slice(ISA);
    input.extend_from_slice(b"GS*HP:X~");
    let doc = Document::parse(&input[..]).unwrap();
    assert_eq!(doc.delimiters().component, b':');
    assert_eq!(doc.segment(0).unwrap().id, b"ISA");
    assert!(doc.segment(0).unwrap().raw.starts_with(b"\r\n"));
    assert_eq!(
        doc.segment(1).unwrap().element(1).and_then(|e| e.simple()),
        None
    );
}

#[test]
fn parse_keeps_a_byte_order_mark_in_the_first_span() {
    let mut input = b"\xEF\xBB\xBF".to_vec();
    input.extend_from_slice(ISA);
    input.extend_from_slice(b"GS*HP~");
    let doc = Document::parse(&input[..]).unwrap();
    assert_eq!(doc.len(), 2);
    assert_eq!(
        doc.spans()[0],
        Span {
            raw: 0..3 + ISA.len(),
            body: 3..3 + ISA.len() - 1,
            terminated: true
        }
    );
    assert_eq!(doc.segment(0).unwrap().id, b"ISA");
    assert_eq!(doc.segment(1).unwrap().id, b"GS");
    assert_eq!(doc.as_bytes(), &input[..]);
    let expected: Vec<_> = Tokenizer::new(&input).unwrap().collect();
    assert_eq!(doc.segments().collect::<Vec<_>>(), expected);
}

#[test]
fn parse_names_the_trivia_it_skipped_when_no_isa_follows() {
    let message = |input: &[u8]| Document::parse(input).err().map(|error| error.to_string());
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
fn parse_fails_without_an_isa() {
    assert_eq!(
        Document::parse(&b"ST*835~"[..]).err(),
        Some(IsaError::NotIsa {
            found: b"ST*835~".to_vec(),
            byte_order_mark: false,
            whitespace: 0,
        })
    );
    assert_eq!(
        Document::parse(&b""[..]).err(),
        Some(IsaError::NotIsa {
            found: Vec::new(),
            byte_order_mark: false,
            whitespace: 0,
        })
    );
}

#[test]
fn empty_input_is_an_empty_document() {
    let doc = Document::with_delimiters(&b""[..], plain());
    assert!(doc.is_empty());
    assert_eq!(doc.len(), 0);
    assert_eq!(doc.as_bytes(), b"");
    assert_eq!(doc.segment(0), None);
}

#[test]
fn as_bytes_is_the_input_unchanged() {
    let input = &b"ST*835~\n"[..];
    assert_eq!(Document::with_delimiters(input, plain()).as_bytes(), input);
}

#[test]
fn segments_iterator_yields_every_segment_with_consecutive_indices() {
    let doc = Document::with_delimiters(&b"ST*835~SE*2~\n"[..], plain());
    let segments: Vec<_> = doc.segments().collect();
    assert_eq!(segments.len(), 3);
    assert_eq!(doc.segments().len(), 3, "ExactSizeIterator");
    for (i, segment) in segments.iter().enumerate() {
        assert_eq!(segment.index, i);
        assert_eq!(doc.segment(i).as_ref(), Some(segment));
    }
}

#[test]
fn a_document_reference_can_be_iterated_with_for() {
    let doc = Document::with_delimiters(&b"ST*835~SE*2~"[..], plain());
    let mut ids = Vec::new();
    for segment in &doc {
        ids.push(segment.id);
    }
    assert_eq!(ids, vec![&b"ST"[..], b"SE"]);
}

#[test]
fn parse_accepts_owned_bytes() {
    fn assert_static(_: &Document<'static>) {}
    let mut input = ISA.to_vec();
    input.extend_from_slice(b"GS*HP~");
    let doc = Document::parse(input).unwrap();
    assert_static(&doc);
    assert_eq!(doc.len(), 2);
    assert_eq!(doc.segment(1).unwrap().id, b"GS");
}

#[test]
fn into_owned_detaches_from_the_buffer() {
    fn assert_static(_: &Document<'static>) {}
    // A `Segment` borrows from the document, so the snapshot taken before
    // the move must own its bytes: otherwise `into_owned` and `drop` would
    // not compile while it is alive.
    fn snapshot(doc: &Document<'_>) -> Vec<(Vec<u8>, Vec<u8>, usize)> {
        doc.segments()
            .map(|s| (s.raw.to_vec(), s.id.to_vec(), s.elements.len()))
            .collect()
    }
    let buffer = b"ST*835~SE*2~".to_vec();
    let borrowed = Document::with_delimiters(&buffer[..], plain());
    let before = snapshot(&borrowed);
    let owned = borrowed.into_owned();
    drop(buffer);
    assert_static(&owned);
    assert_eq!(snapshot(&owned), before);
    assert_eq!(owned.as_bytes(), b"ST*835~SE*2~");
}

#[test]
fn into_owned_of_an_owned_document_does_not_change_it() {
    let doc = Document::with_delimiters(b"ST*835~".to_vec(), plain());
    let owned = doc.clone().into_owned();
    assert_eq!(owned, doc);
}
