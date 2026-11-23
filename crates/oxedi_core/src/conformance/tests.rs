use std::collections::BTreeMap;

use super::*;
use crate::spec_load::{Builtin, Kit, KitOutput};
use crate::validate::{Valid, Validation, validate};
use crate::{Document, Processor, Spec};

const FIVE: &[u8] = include_bytes!("../../tests/fixtures/balanced_5010_sample.txt");

fn text(line: &str) -> Line {
    Line::Text(line.to_owned())
}

#[test]
fn equal_texts_have_no_difference() {
    assert_eq!(first_difference("a\nb\n", "a\nb"), None);
}

#[test]
fn a_changed_line() {
    assert_eq!(
        first_difference("a\nb", "a\nc"),
        Some(LineDifference {
            line: 2,
            expected: text("b"),
            actual: text("c"),
        })
    );
}

#[test]
fn actual_is_longer() {
    assert_eq!(
        first_difference("a", "a\nz"),
        Some(LineDifference {
            line: 2,
            expected: Line::End,
            actual: text("z"),
        })
    );
}

#[test]
fn expected_is_longer() {
    assert_eq!(
        first_difference("a\nz", "a"),
        Some(LineDifference {
            line: 2,
            expected: text("z"),
            actual: Line::End,
        })
    );
}

#[test]
fn crlf_and_lf_line_ends_are_equal() {
    assert_eq!(first_difference("a\r\nb", "a\nb"), None);
}

#[test]
fn two_empty_texts_are_equal() {
    assert_eq!(first_difference("", ""), None);
}

#[test]
fn an_empty_text_lacks_the_empty_line_a_lone_break_holds() {
    assert_eq!(
        first_difference("", "\n"),
        Some(LineDifference {
            line: 1,
            expected: Line::End,
            actual: text(""),
        })
    );
}

#[test]
fn multi_byte_lines_are_kept_whole() {
    assert_eq!(
        first_difference("año\nñandú", "año\nñandá"),
        Some(LineDifference {
            line: 2,
            expected: text("ñandú"),
            actual: text("ñandá"),
        })
    );
}

#[test]
fn render_writes_the_tables_and_one_line_per_diagnostic() {
    let document = Document::parse(FIVE).unwrap();
    let (tables, diagnostics) = Processor::run(&Spec::builtin_835(), &document, None);
    let lines: String = diagnostics.iter().map(|d| format!("{d}\n")).collect();
    assert_eq!(
        render(KitOutput::Tables, &tables, &diagnostics),
        tables.to_string()
    );
    assert_eq!(render(KitOutput::Diagnostics, &tables, &diagnostics), lines);
    assert_eq!(render(KitOutput::Diagnostics, &tables, &[]), "");
}

/// The tables text and the diagnostics text of the balanced 5010 file.
fn rendered() -> (String, String) {
    let document = Document::parse(FIVE).unwrap();
    let (tables, diagnostics) = Processor::run(&Spec::builtin_835(), &document, None);
    (
        tables.to_string(),
        diagnostics.iter().map(|d| format!("{d}\n")).collect(),
    )
}

/// The built-in 5010 stack, as the validator loads it.
fn stack() -> Valid {
    let packages = Builtin::packages();
    match validate(packages[4], packages) {
        Validation::Yes(valid) => *valid,
        Validation::No(invalid) => panic!("the shipped stack does not load: {invalid}"),
    }
}

fn kit_of(package: &str, cases: &str) -> Kit {
    let json = format!(r#"{{"kit":{{"format":1,"package":"{package}"}},"cases":{cases}}}"#);
    Kit::from_json(&json).unwrap()
}

/// Runs a kit of the given cases (a JSON object) over `files` with the 5010 stack.
fn run(cases: &str, files: &[(&str, &[u8])]) -> KitReport {
    let kit = kit_of("oxedi/views/005010/835/X221A1", cases);
    let files: BTreeMap<&str, &[u8]> = files.iter().copied().collect();
    run_kit(&kit, &stack(), &files).unwrap()
}

#[test]
fn a_case_that_matches_passes() {
    let (tables, diagnostics) = rendered();
    let cases = r#"{"a":{"input":"in.835","expect":{"tables":"t.txt","diagnostics":"d.txt"}}}"#;
    let files = [
        ("in.835", FIVE),
        ("t.txt", tables.as_bytes()),
        ("d.txt", diagnostics.as_bytes()),
    ];
    let report = run(cases, &files);
    assert!(report.passed());
    assert_eq!(report.cases[0].outcome, Outcome::Passed);
    assert_eq!(
        report.to_string(),
        "kit \"oxedi/views/005010/835/X221A1\": 1 of 1 cases pass\ncase \"a\": passes"
    );
}

#[test]
fn a_kit_for_another_package_is_refused() {
    let kit = kit_of(
        "oxedi/guide/005010/835/X221A1",
        r#"{"a":{"input":"in.835","expect":{"tables":"t.txt"}}}"#,
    );
    let err = run_kit(&kit, &stack(), &BTreeMap::new()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "the kit checks package \"oxedi/guide/005010/835/X221A1\", but the stack's top package is \"oxedi/views/005010/835/X221A1\"; run the kit on the stack that has \"oxedi/guide/005010/835/X221A1\" on top, or change the kit's \"package\" to \"oxedi/views/005010/835/X221A1\""
    );
    assert!(std::error::Error::source(&err).is_none());
}
