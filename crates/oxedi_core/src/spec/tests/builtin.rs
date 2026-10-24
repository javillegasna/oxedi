//! The registry of built-in specs.

use std::error::Error;

use crate::Document;
use crate::spec::{Builtin, UnknownVersion};

fn selected(bytes: &[u8]) -> &'static str {
    let document = Document::parse(bytes.to_vec()).unwrap();
    Builtin::select(document.segments()).version
}

#[test]
fn the_fallback_is_the_first_and_named_by_default_version() {
    assert_eq!(Builtin::all()[0].version, Builtin::DEFAULT_VERSION);
    let first = &Builtin::all()[0];
    let fallback = Builtin::fallback();
    assert_eq!(
        (fallback.transaction, fallback.version),
        (first.transaction, first.version)
    );
}

#[test]
fn find_835_4010_gives_the_4010_patch() {
    let found = Builtin::find("835", "4010").unwrap();
    let declared = found.spec.version().unwrap();
    assert!(declared.values.iter().any(|v| v == b"004010X091A1"));
}

#[test]
fn select_reads_the_declared_version() {
    let four = include_bytes!("../../../tests/fixtures/balanced_4010_sample.txt");
    let five = include_bytes!("../../../tests/fixtures/balanced_5010_sample.txt");
    assert_eq!(selected(four), "4010");
    assert_eq!(selected(five), "5010");
    assert_eq!(
        selected(b"ISA*00*          *00*          *ZZ*A              *ZZ*B              *200101*1200*^*00501*000000001*0*P*:~"),
        Builtin::fallback().version
    );
}

#[test]
fn unknown_version_display_lists_the_known() {
    let error = Builtin::find("835", "3070").unwrap_err();
    assert_eq!(
        error.to_string(),
        "unknown version \"3070\"; version must be one of \"5010\", \"4010\""
    );
}

#[test]
fn unknown_version_display_of_an_unknown_transaction() {
    let error = Builtin::find("277", "3070").unwrap_err();
    assert_eq!(
        error.to_string(),
        "unknown version \"3070\"; no built-in spec covers transaction \"277\""
    );
}

#[test]
fn unknown_version_has_no_source() {
    let error: UnknownVersion = Builtin::find("835", "3070").unwrap_err();
    assert!(error.source().is_none());
}
