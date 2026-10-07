//! The five real fixtures through `Document`: lossless, identical to the
//! tokenizer, borrowed and owned.

mod common;

use oxedi_core::{Delimiters, Document, DocumentError, IsaError, Tokenizer};

const ENVELOPED: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
    "trizetto_sample.rmt",
];

fn assert_matches_tokenizer(doc: &Document<'_>, bytes: &[u8], delims: Delimiters, name: &str) {
    assert_eq!(doc.as_bytes(), bytes, "{name}: bytes must be the file");
    let expected: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    assert_eq!(doc.len(), expected.len(), "{name}: segment count");
    for (i, segment) in expected.iter().enumerate() {
        assert_eq!(doc.segment(i).as_ref(), Some(segment), "{name} segment {i}");
        assert_eq!(doc.raw(i), Some(segment.raw), "{name} raw of segment {i}");
        assert_eq!(
            doc.segment_id(i),
            Some(segment.id),
            "{name} id of segment {i}"
        );
    }
    assert_eq!(doc.raw(expected.len()), None, "{name}: raw past the end");
    assert_eq!(
        doc.segment_id(expected.len()),
        None,
        "{name}: id past the end"
    );
    let from_iter: Vec<_> = doc.segments().collect();
    assert_eq!(
        from_iter, expected,
        "{name}: segments() must match the tokenizer"
    );
    let rebuilt: Vec<u8> = doc
        .spans()
        .flat_map(|s| bytes[s.raw].iter().copied())
        .collect();
    assert_eq!(rebuilt, bytes, "{name}: spans must rebuild the file");
}

#[test]
fn enveloped_fixtures_are_held_losslessly() {
    for name in ENVELOPED {
        let bytes = common::load_fixture(name);
        let doc = Document::parse(&bytes[..]).unwrap_or_else(|e| panic!("{name}: {e}"));
        let delims = *doc.delimiters();
        assert_matches_tokenizer(&doc, &bytes, delims, name);
    }
}

#[test]
fn owned_fixtures_yield_the_same_segments() {
    for name in ENVELOPED {
        let bytes = common::load_fixture(name);
        let delims = *Document::parse(&bytes[..]).unwrap().delimiters();
        let owned = Document::parse(bytes.clone()).unwrap();
        assert_matches_tokenizer(&owned, &bytes, delims, name);
        let promoted = Document::parse(&bytes[..]).unwrap().into_owned();
        assert_eq!(
            promoted, owned,
            "{name}: into_owned must equal parsing an owned Vec"
        );
    }
}

#[test]
fn fragment_without_isa_needs_caller_delimiters() {
    let bytes = common::load_fixture("blue_cross_nc_sample.txt");
    assert_eq!(
        Document::parse(&bytes[..]).err(),
        Some(DocumentError::Isa(IsaError::NotIsa {
            found: b"ST*835*1".to_vec(),
            byte_order_mark: false,
            whitespace: 0,
        }))
    );
    let delims = Delimiters::new(b'*', b':', b'~');
    let doc = Document::with_delimiters(&bytes[..], delims).unwrap();
    assert_eq!(doc.segment(0).unwrap().id, b"ST");
    assert_matches_tokenizer(&doc, &bytes, delims, "blue_cross_nc_sample.txt");
}
