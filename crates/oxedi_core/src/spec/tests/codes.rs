//! The sorted code lists the schema asks.

use crate::spec::elements::holds_code;
use crate::spec::rejects_code;

#[test]
fn holds_code_on_a_sorted_list() {
    let codes: Vec<String> = vec!["A".into(), "C".into()];
    assert!(holds_code(&codes, b"C"));
    assert!(!holds_code(&codes, b"B"));
    assert!(!holds_code(&[], b"A"));
}

#[test]
fn rejects_code_keeps_an_empty_list_open() {
    assert!(!rejects_code(&[], b"A"));
    assert!(rejects_code(&["A".into()], b"B"));
    assert!(!rejects_code(&["A".into()], b"A"));
}
