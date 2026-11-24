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

const TABLES_CASE: &str = r#"{"a":{"input":"in.835","expect":{"tables":"t.txt"}}}"#;

#[test]
fn a_table_difference_names_the_first_line() {
    let (tables, _) = rendered();
    let mut lines: Vec<&str> = tables.lines().collect();
    let real = lines[2].to_owned();
    lines[2] = "x";
    let edited = lines.join("\n");
    let report = run(
        TABLES_CASE,
        &[("in.835", FIVE), ("t.txt", edited.as_bytes())],
    );
    let at = LineDifference {
        line: 3,
        expected: text("x"),
        actual: Line::Text(real.clone()),
    };
    assert_eq!(
        report.cases[0].outcome,
        Outcome::Differs(vec![Difference {
            output: KitOutput::Tables,
            file: "t.txt".to_owned(),
            at,
        }])
    );
    let Outcome::Differs(differences) = &report.cases[0].outcome else {
        panic!("{report}");
    };
    assert_eq!(
        differences[0].to_string(),
        format!("tables (t.txt) line 3: expected \"x\", found {real:?}")
    );
}

#[test]
fn expected_diagnostics_on_a_clean_file() {
    let cases = r#"{"a":{"input":"in.835","expect":{"diagnostics":"d.txt"}}}"#;
    let report = run(cases, &[("in.835", FIVE), ("d.txt", b"x\n")]);
    let Outcome::Differs(differences) = &report.cases[0].outcome else {
        panic!("{report}");
    };
    assert_eq!(differences[0].at.line, 1);
    assert_eq!(differences[0].at.expected, text("x"));
    assert_eq!(differences[0].at.actual, Line::End);
    assert!(report.to_string().contains("found the end"));
}

#[test]
fn faults_break_the_case_and_say_what_to_do() {
    let missing = run(TABLES_CASE, &[("t.txt", b"")]);
    assert_eq!(
        missing.cases[0].outcome,
        Outcome::Broken(CaseFault::MissingFile {
            path: "in.835".to_owned()
        })
    );
    assert_eq!(
        missing.cases[0].to_string(),
        "case \"a\": cannot run: the kit names \"in.835\", which was not given; give the file to the runner"
    );
    let binary = run(TABLES_CASE, &[("in.835", FIVE), ("t.txt", &[0xff])]);
    assert_eq!(
        binary.cases[0].to_string(),
        "case \"a\": cannot run: \"t.txt\" is not UTF-8 text; an expected output is a text file"
    );
    let hello = run(TABLES_CASE, &[("in.835", b"hello"), ("t.txt", b"")]);
    let Outcome::Broken(fault @ CaseFault::NotADocument { .. }) = &hello.cases[0].outcome else {
        panic!("{hello}");
    };
    let reason = Document::parse(&b"hello"[..]).unwrap_err();
    assert_eq!(
        fault.to_string(),
        format!("\"in.835\" is not an X12 document: {reason}; give a file that opens with ISA")
    );
}

#[test]
fn the_first_fault_ends_the_case() {
    let cases =
        r#"{"a":{"input":"in.835","expect":{"tables":"missing.txt","diagnostics":"t.txt"}}}"#;
    let report = run(cases, &[("in.835", FIVE), ("t.txt", &[0xff])]);
    assert_eq!(
        report.cases[0].outcome,
        Outcome::Broken(CaseFault::MissingFile {
            path: "missing.txt".to_owned()
        })
    );
}

#[test]
fn cases_report_in_name_order() {
    let (tables, _) = rendered();
    let cases = r#"{"b":{"input":"in.835","expect":{"tables":"t.txt"}},"a":{"input":"in.835","expect":{"tables":"x.txt"}}}"#;
    let report = run(
        cases,
        &[
            ("in.835", FIVE),
            ("t.txt", tables.as_bytes()),
            ("x.txt", b"x"),
        ],
    );
    let names: Vec<&str> = report.cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert!(!report.passed());
    assert!(report.to_string().contains("1 of 2 cases pass"));
}
