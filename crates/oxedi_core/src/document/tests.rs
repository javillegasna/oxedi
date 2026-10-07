use super::*;
use crate::delimiters::test_support::plain;
use crate::{IsaError, Tokenizer};

const ISA: &[u8] =
    b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

#[test]
fn with_delimiters_indexes_every_frame() {
    let doc = Document::with_delimiters(&b"ST*835~\nSE*2~\n"[..], plain()).unwrap();
    assert_eq!(doc.len(), 3);
    assert_eq!(doc.segment(0).unwrap().id, b"ST");
    assert_eq!(doc.segment(1).unwrap().id, b"SE");
    assert!(doc.segment(2).unwrap().is_empty());
    assert_eq!(doc.segment(3), None);
}

#[test]
fn spans_partition_the_bytes() {
    let input = &b"\nST*835~\n\nSE~X"[..];
    let doc = Document::with_delimiters(input, plain()).unwrap();
    let spans: Vec<Span> = doc.spans().collect();
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
    for span in &spans {
        assert_eq!(span.raw.start, next, "spans must be contiguous");
        assert!(span.raw.start <= span.body.start && span.body.end <= span.raw.end);
        next = span.raw.end;
    }
    assert_eq!(next, input.len(), "spans must cover the whole input");

    let tail_only = Document::with_delimiters(&b"ST~\n"[..], plain()).unwrap();
    assert_eq!(
        tail_only.span(1).unwrap(),
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
    let doc = Document::with_delimiters(input, plain()).unwrap();
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
        doc.span(0).unwrap(),
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
        Some(DocumentError::Isa(IsaError::NotIsa {
            found: b"ST*835~".to_vec(),
            byte_order_mark: false,
            whitespace: 0,
        }))
    );
    assert_eq!(
        Document::parse(&b""[..]).err(),
        Some(DocumentError::Isa(IsaError::NotIsa {
            found: Vec::new(),
            byte_order_mark: false,
            whitespace: 0,
        }))
    );
}

#[test]
fn empty_input_is_an_empty_document() {
    let doc = Document::with_delimiters(&b""[..], plain()).unwrap();
    assert!(doc.is_empty());
    assert_eq!(doc.len(), 0);
    assert_eq!(doc.as_bytes(), b"");
    assert_eq!(doc.segment(0), None);
}

#[test]
fn as_bytes_is_the_input_unchanged() {
    let input = &b"ST*835~\n"[..];
    assert_eq!(
        Document::with_delimiters(input, plain())
            .unwrap()
            .as_bytes(),
        input
    );
}

#[test]
fn segments_iterator_yields_every_segment_with_consecutive_indices() {
    let doc = Document::with_delimiters(&b"ST*835~SE*2~\n"[..], plain()).unwrap();
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
    let doc = Document::with_delimiters(&b"ST*835~SE*2~"[..], plain()).unwrap();
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
    let borrowed = Document::with_delimiters(&buffer[..], plain()).unwrap();
    let before = snapshot(&borrowed);
    let owned = borrowed.into_owned();
    drop(buffer);
    assert_static(&owned);
    assert_eq!(snapshot(&owned), before);
    assert_eq!(owned.as_bytes(), b"ST*835~SE*2~");
}

#[test]
fn into_owned_of_an_owned_document_does_not_change_it() {
    let doc = Document::with_delimiters(b"ST*835~".to_vec(), plain()).unwrap();
    let owned = doc.clone().into_owned();
    assert_eq!(owned, doc);
}

#[test]
fn the_index_stores_eight_bytes_per_segment() {
    assert_eq!(std::mem::size_of::<Body>(), 8);
}

#[test]
fn span_is_none_past_the_last_segment() {
    let doc = Document::with_delimiters(&b"ST~SE~"[..], plain()).unwrap();
    assert_eq!(doc.spans().len(), 2, "ExactSizeIterator");
    assert_eq!(doc.span(2), None);
    assert_eq!(
        doc.spans().collect::<Vec<_>>(),
        vec![doc.span(0).unwrap(), doc.span(1).unwrap()]
    );
}

#[test]
fn size_check_accepts_up_to_the_limit_and_rejects_beyond() {
    assert_eq!(SizeError::LIMIT, u32::MAX as usize);
    assert_eq!(SizeError::check(0), Ok(()));
    assert_eq!(SizeError::check(SizeError::LIMIT), Ok(()));
    if let Some(len) = SizeError::LIMIT.checked_add(1) {
        assert_eq!(SizeError::check(len), Err(SizeError { len }));
    }
}

#[test]
fn size_error_names_the_rule_the_length_and_the_limit() {
    assert_eq!(
        SizeError { len: 4_294_967_296 }.to_string(),
        "input of 4294967296 bytes exceeds the document size limit of 4294967295 bytes \
         (segment offsets are stored as 32-bit integers)"
    );
}

#[test]
fn document_error_shows_the_isa_error_and_chains_it() {
    use std::error::Error;
    let error = DocumentError::from(IsaError::NotIsa {
        found: b"ST*835~".to_vec(),
        byte_order_mark: false,
        whitespace: 0,
    });
    assert_eq!(
        error.to_string(),
        "input does not start with an ISA segment (found bytes [53 54 2a 38 33 35 7e])"
    );
    assert_eq!(
        error.source().map(ToString::to_string),
        Some(error.to_string())
    );
}

#[test]
fn document_error_shows_the_size_error_and_chains_it() {
    use std::error::Error;
    let error = DocumentError::from(SizeError { len: 4_294_967_296 });
    assert_eq!(
        error.to_string(),
        "input of 4294967296 bytes exceeds the document size limit of 4294967295 bytes \
         (segment offsets are stored as 32-bit integers)"
    );
    assert_eq!(
        error.source().map(ToString::to_string),
        Some(error.to_string())
    );
}

#[test]
fn segment_into_one_buffer_equals_segment_and_stops_past_the_end() {
    let input = &b"ISA*00~\r\nCLP*1*2*3*4*5*6~SVC*HC:99213:25*100**12~SVC*X~~LX*1"[..];
    let doc = Document::with_delimiters(input, plain()).unwrap();
    let mut buffer = Segment::empty();
    for index in 0..doc.len() {
        assert!(doc.segment_into(index, &mut buffer));
        assert_eq!(
            doc.segment(index).as_ref(),
            Some(&buffer),
            "segment {index}"
        );
    }
    let last = buffer.clone();
    assert!(!doc.segment_into(doc.len(), &mut buffer));
    assert_eq!(buffer, last, "a miss leaves the buffer untouched");
}
