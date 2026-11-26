use super::*;
use crate::delimiters::IsaError;
use crate::diagnostic::{Diagnostic, PARSER_ORIGIN};
use crate::document::{DocumentError, SizeError};
use crate::message::ElementRef;
use crate::value::ElementTextError;
use crate::write::Finding;
use crate::write::{PlanError, WriteError};

fn unknown_segment() -> Diagnostic {
    Diagnostic::new(
        Rule::UnknownSegment {
            segment_id: b"XX".to_vec(),
        },
        Some(3),
        None,
        Some(2),
        vec![],
        b"XX".to_vec(),
    )
}

fn cell() -> ValueOrigin {
    ValueOrigin::Cell {
        table: "claims".into(),
        row: 2,
        column: "id".into(),
    }
}

fn delimiter(origin: ValueOrigin) -> Finding {
    Finding::DelimiterInValue {
        origin,
        place: ElementRef::simple(b"CLP".to_vec(), 1),
        value: b"A:B".to_vec(),
        delimiter: b':',
        role: "component separator",
    }
}

fn not_isa() -> IsaError {
    IsaError::NotIsa {
        found: Vec::new(),
        byte_order_mark: false,
        whitespace: 0,
    }
}

#[test]
fn a_diagnostic_record_holds_its_rule_and_place() {
    let d = unknown_segment();
    assert_eq!(
        d.record(),
        ProblemRecord {
            kind: "UnknownSegment",
            place: Place::File {
                segment: Some(3),
                element: None,
                component: Some(2),
                path: &[],
            },
            text: d.to_string(),
            detail: Detail::Diagnostic {
                level: SnipLevel::L1,
                rule: &d.rule,
                datum: b"XX",
                origin: PARSER_ORIGIN,
                code: None,
            },
        }
    );
}

#[test]
fn a_finding_points_at_its_value_or_the_file() {
    let found = delimiter(cell());
    assert_eq!(
        found.place(),
        Place::Cell {
            table: "claims",
            row: Some(2),
            column: Some("id")
        }
    );
    assert_eq!(
        found.detail(),
        Detail::Finding {
            origin: Some(&cell())
        }
    );
    let read_back = Finding::ReadBack {
        origin: None,
        diagnostic: Box::new(unknown_segment()),
    };
    assert_eq!(read_back.place(), unknown_segment().place());
    let duplicate = Finding::DuplicateRowNumber {
        table: "t".into(),
        row: 4,
        value: 1,
        first: 0,
    };
    assert_eq!(
        duplicate.place(),
        Place::Cell {
            table: "t",
            row: Some(4),
            column: None
        }
    );
    assert_eq!(duplicate.detail(), Detail::Finding { origin: None });
}

#[test]
fn finding_kinds_are_the_variant_names() {
    let kinds = [
        delimiter(cell()),
        Finding::MissingParent {
            table: "t".into(),
            row: 0,
            column: "c".into(),
            value: None,
            parent: "p".into(),
        },
        Finding::MismatchedReference {
            table: "t".into(),
            row: 0,
            column: "c".into(),
            value: 1,
            through: "x".into(),
            parent: "p".into(),
            parent_row: 0,
            expected: 2,
        },
        Finding::DuplicateRowNumber {
            table: "t".into(),
            row: 0,
            value: 1,
            first: 0,
        },
        Finding::OutOfOrder {
            table: "t".into(),
            row: 0,
            column: "c".into(),
            value: 1,
            previous: 0,
        },
        Finding::NotWritable {
            origin: cell(),
            place: ElementRef::simple(b"DTM".to_vec(), 2),
            value: "v".into(),
            reason: ElementTextError::YearOutOfRange,
        },
        Finding::ReadBack {
            origin: None,
            diagnostic: Box::new(unknown_segment()),
        },
    ]
    .map(|finding| finding.kind());
    assert_eq!(
        kinds,
        [
            "DelimiterInValue",
            "MissingParent",
            "MismatchedReference",
            "DuplicateRowNumber",
            "OutOfOrder",
            "NotWritable",
            "ReadBack"
        ]
    );
}

#[test]
fn write_error_places() {
    let plan = WriteError::Plan(PlanError {
        spec: "s".into(),
        refusals: Vec::new(),
    });
    assert_eq!((plan.kind(), plan.place()), ("Plan", Place::Nowhere));
    let type_error = WriteError::ColumnType {
        table: "t".into(),
        column: "c".into(),
        expected: crate::value::ColumnType::Date32,
        found: crate::value::ColumnType::Time32,
    };
    assert_eq!(
        (type_error.kind(), type_error.place()),
        (
            "ColumnType",
            Place::Cell {
                table: "t",
                row: None,
                column: Some("c")
            }
        )
    );
    let unreadable = WriteError::Unreadable(DocumentError::Isa(not_isa()));
    assert_eq!(unreadable.kind(), "Unreadable");
    assert_eq!(unreadable.place(), DocumentError::Isa(not_isa()).place());
}

#[test]
fn document_error_places() {
    let isa = DocumentError::Isa(not_isa());
    assert_eq!(
        (isa.kind(), isa.place()),
        (
            "Isa",
            Place::File {
                segment: Some(0),
                element: None,
                component: None,
                path: &[]
            }
        )
    );
    let size = DocumentError::Size(SizeError { len: 1 });
    assert_eq!((size.kind(), size.place()), ("Size", Place::Nowhere));
}

#[test]
fn write_error_kinds() {
    let errors = [
        WriteError::Plan(PlanError {
            spec: "s".into(),
            refusals: Vec::new(),
        }),
        WriteError::UnknownTable(crate::spec::UnknownTable {
            name: "t".into(),
            tables: Vec::new(),
        }),
        WriteError::UnknownColumn(crate::spec::UnknownColumn {
            table: "t".into(),
            column: "c".into(),
            columns: Vec::new(),
        }),
        WriteError::ColumnType {
            table: "t".into(),
            column: "c".into(),
            expected: crate::value::ColumnType::Date32,
            found: crate::value::ColumnType::Time32,
        },
        WriteError::SameDelimiter {
            first: "a",
            second: "b",
            byte: b'*',
        },
        WriteError::DelimiterNotAllowed {
            role: "a",
            byte: b'*',
        },
        WriteError::NoRepetition {
            version: Vec::new(),
        },
        WriteError::Unreadable(DocumentError::Size(SizeError { len: 1 })),
        WriteError::Findings(Vec::new()),
    ];
    let kinds: Vec<&str> = errors.iter().map(Problem::kind).collect();
    assert_eq!(
        kinds,
        [
            "Plan",
            "UnknownTable",
            "UnknownColumn",
            "ColumnType",
            "SameDelimiter",
            "DelimiterNotAllowed",
            "NoRepetition",
            "Unreadable",
            "Findings"
        ]
    );
}

#[test]
fn document_error_kinds() {
    let kinds = [
        DocumentError::Isa(not_isa()).kind(),
        DocumentError::Size(SizeError { len: 1 }).kind(),
    ];
    assert_eq!(kinds, ["Isa", "Size"]);
}
