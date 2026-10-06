//! The five real fixtures through the whole tokenizer: lossless by construction,
//! one segment per frame, and the ISA-less fragment case.

mod common;

use edi835_core::{Delimiters, IsaError, Segment, Tokenizer, next_frame};

/// (fixture, number of `~` in it). Terminated segments must match exactly.
const ENVELOPED: &[(&str, usize)] = &[
    ("emedny_sample.txt", 69),
    ("united_healthcare_legacy_sample.txt", 65),
    ("multi_claim_sample.txt", 51),
    ("trizetto_sample.rmt", 22),
];
const FRAGMENT: (&str, usize) = ("blue_cross_nc_sample.txt", 32);

fn concat_raw(segments: &[Segment<'_>]) -> Vec<u8> {
    segments
        .iter()
        .flat_map(|s| s.raw.iter().copied())
        .collect()
}

fn count_frames(mut input: &[u8], delims: &Delimiters) -> usize {
    let mut n = 0;
    while let Some((_, rest)) = next_frame(input, delims) {
        n += 1;
        input = rest;
    }
    n
}

fn assert_stage1_gate(bytes: &[u8], segments: &[Segment<'_>], delims: &Delimiters, tildes: usize) {
    assert_eq!(
        concat_raw(segments),
        bytes,
        "concatenated raw must equal the file"
    );
    assert_eq!(segments.iter().filter(|s| s.terminated).count(), tildes);
    assert_eq!(
        bytes.iter().filter(|&&b| b == b'~').count(),
        tildes,
        "fixture changed?"
    );
    assert_eq!(
        segments.len(),
        count_frames(bytes, delims),
        "tokenizer must emit one segment per frame"
    );
    for (expected, segment) in segments.iter().enumerate() {
        assert_eq!(
            segment.index, expected,
            "indices must be consecutive from 0"
        );
    }
    for segment in &segments[..segments.len() - 1] {
        assert!(
            segment.terminated,
            "only the last segment may be unterminated"
        );
    }
    assert!(
        segments
            .iter()
            .filter(|s| s.terminated)
            .all(|s| !s.id.is_empty()),
        "no `~~` in fixtures"
    );
}

#[test]
fn enveloped_fixtures_tokenize_losslessly_from_their_isa() {
    for &(name, tildes) in ENVELOPED {
        let bytes = common::load_fixture(name);
        let tokenizer = Tokenizer::new(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let delims = *tokenizer.delimiters();
        let segments: Vec<_> = tokenizer.collect();
        assert_eq!(segments[0].id, b"ISA", "{name}");
        assert_stage1_gate(&bytes, &segments, &delims, tildes);
    }
}

#[test]
fn fixtures_with_a_newline_per_segment_end_in_an_empty_segment() {
    for name in ["multi_claim_sample.txt", "trizetto_sample.rmt"] {
        let bytes = common::load_fixture(name);
        let last = Tokenizer::new(&bytes).unwrap().last().unwrap();
        assert!(
            last.is_empty(),
            "{name}: trailing LF must be its own empty segment"
        );
        assert_eq!(last.raw, b"\n", "{name}");
    }
}

#[test]
fn fragment_without_isa_needs_caller_delimiters() {
    let (name, tildes) = FRAGMENT;
    let bytes = common::load_fixture(name);
    assert_eq!(
        Tokenizer::new(&bytes).err(),
        Some(IsaError::NotIsa {
            found: b"ST*835*1".to_vec(),
            byte_order_mark: false,
            whitespace: 0,
        })
    );
    let delims = Delimiters::new(b'*', b':', b'~');
    let segments: Vec<_> = Tokenizer::with_delimiters(&bytes, delims).collect();
    assert_eq!(segments[0].id, b"ST");
    assert_stage1_gate(&bytes, &segments, &delims, tildes);
}

#[test]
fn trizetto_anomaly_is_preserved_as_an_unknown_segment() {
    // The fixture has `~XX*654321~` where `*` was probably intended. It stays
    // byte-exact: a lossless tokenizer keeps it as a segment with id `XX`.
    let bytes = common::load_fixture("trizetto_sample.rmt");
    let ids: Vec<Vec<u8>> = Tokenizer::new(&bytes)
        .unwrap()
        .map(|s| s.id.to_vec())
        .collect();
    assert!(ids.contains(&b"XX".to_vec()));
}

#[test]
fn delimiters_read_from_each_fixture_match_the_known_values() {
    let expect = [
        ("emedny_sample.txt", b':', Some(b'^')),
        ("united_healthcare_legacy_sample.txt", b'>', Some(b'^')),
        ("multi_claim_sample.txt", b'>', None),
        ("trizetto_sample.rmt", b'>', None),
    ];
    for (name, component, repetition) in expect {
        let bytes = common::load_fixture(name);
        let d = Delimiters::from_isa(&bytes).unwrap();
        assert_eq!(
            (d.element, d.component, d.segment),
            (b'*', component, b'~'),
            "{name}"
        );
        assert_eq!(d.repetition, repetition, "{name}");
    }
}
