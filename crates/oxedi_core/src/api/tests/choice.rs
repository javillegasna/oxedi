use std::sync::Arc;

use crate::api::{ChosenSpec, DelimiterChoice, SpecChoice};
use crate::delimiters::Delimiters;
use crate::document::{Document, DocumentError};
use crate::segment::Segment;
use crate::spec::Spec;
use crate::spec_load::{Builtin, Version};

const FOUR_TEN: &[u8] = include_bytes!("../../../tests/fixtures/balanced_4010_sample.txt");

#[test]
fn declared_chooses_the_builtin_the_file_declares() {
    let document = Document::parse(FOUR_TEN).unwrap();
    let chosen = SpecChoice::<Arc<Spec>>::Declared.choose(document.segments());
    assert!(matches!(
        chosen,
        ChosenSpec::Builtin(b)
            if std::ptr::eq(b, Builtin::select(document.segments())) && b.version == Version::V4010
    ));
}

#[test]
fn given_reads_no_segment() {
    let never = std::iter::from_fn(|| -> Option<Segment<'static>> { panic!("read a segment") });
    let chosen = SpecChoice::Given(Arc::new(Spec::builtin_835())).choose(never);
    assert!(matches!(chosen, ChosenSpec::Given(_)));
}

#[test]
fn the_defaults_are_declared_and_from_header() {
    assert!(matches!(
        SpecChoice::<Arc<Spec>>::default(),
        SpecChoice::Declared
    ));
    assert_eq!(DelimiterChoice::default(), DelimiterChoice::FromHeader);
}

#[test]
fn given_delimiters_read_a_file_without_an_isa() {
    let bytes = b"ST*835*1~SE*2*1~".as_slice();
    let given = DelimiterChoice::Given(Delimiters::new(b'*', b':', b'~'));
    assert_eq!(given.document(bytes).unwrap().len(), 2);
    assert!(matches!(
        DelimiterChoice::FromHeader.document(bytes),
        Err(DocumentError::Isa(_))
    ));
    assert!(DelimiterChoice::FromHeader.tokenizer(b"ST*835*1~").is_err());
}
