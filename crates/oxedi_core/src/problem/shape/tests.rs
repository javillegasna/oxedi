use super::*;
use crate::conformance::CaseFault;
use crate::delimiters::IsaError;
use crate::diagnostic::{Diagnostic, PARSER_ORIGIN};
use crate::document::{DocumentError, SizeError};
use crate::mask::MaskError;
use crate::message::ElementRef;
use crate::spec_load::{KitError, PackageId, SpecError, TableDefError};
use crate::summary::SummaryError;
use crate::value::ElementTextError;
use crate::write::Finding;
use crate::write::{LoopWriter, PlanError, Refusal, WriteError};

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

fn table<'a>(name: &'a str, column: Option<&'a str>) -> Place<'a> {
    Place::Spec(SpecPlace::Table { name, column })
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

#[test]
fn refusal_places() {
    let duplicate = Refusal::DuplicateSource {
        table: "t".into(),
        column: "c".into(),
        other: "o".into(),
        loop_name: "2100".into(),
        occurrence: "x".into(),
        place: ElementRef::simple(b"CLP".to_vec(), 1),
    };
    assert_eq!(
        (duplicate.kind(), duplicate.place()),
        ("DuplicateSource", table("t", Some("c")))
    );
    let element = |table: Option<&str>| Refusal::RequiredElementWithoutSource {
        table: table.map(String::from),
        loop_name: "2100".into(),
        occurrence: "x".into(),
        place: ElementRef::simple(b"CLP".to_vec(), 1),
        name: "n".into(),
    };
    assert_eq!(element(Some("t")).place(), table("t", None));
    assert_eq!(
        element(None).place(),
        Place::Spec(SpecPlace::Loop {
            name: "2100",
            occurrence: Some("x")
        })
    );
    let loop_ = Refusal::RequiredLoopWithoutSource {
        loop_name: "2100".into(),
        parent: "2000".into(),
        trigger: crate::spec::Selector::Occurrence {
            segment: b"NM1".to_vec(),
            qualifier: None,
        },
    };
    assert_eq!(
        loop_.place(),
        Place::Spec(SpecPlace::Loop {
            name: "2100",
            occurrence: None
        })
    );
    let written = Refusal::WrittenByAnotherTable {
        table: "t".into(),
        column: "c".into(),
        loop_name: "l".into(),
        writer: LoopWriter::Envelope,
    };
    assert_eq!(written.kind(), "WrittenByAnotherTable");
}

#[test]
fn refusal_kinds() {
    let s = |text: &str| text.to_string();
    let trigger = crate::spec::Selector::Occurrence {
        segment: b"NM1".to_vec(),
        qualifier: None,
    };
    let place = || ElementRef::simple(b"CLP".to_vec(), 1);
    let refusals = [
        Refusal::RequiredLoopWithoutSource {
            loop_name: s("l"),
            parent: s("p"),
            trigger: trigger.clone(),
        },
        Refusal::RequiredOccurrenceWithoutSource {
            loop_name: s("l"),
            occurrence: s("o"),
            selector: trigger.clone(),
        },
        Refusal::RequiredElementWithoutSource {
            table: None,
            loop_name: s("l"),
            occurrence: s("o"),
            place: place(),
            name: s("n"),
        },
        Refusal::AmbiguousSource {
            table: s("t"),
            column: None,
            loop_name: s("l"),
            source: trigger.clone(),
            candidates: Vec::new(),
        },
        Refusal::PickNotContiguous {
            table: s("t"),
            column: s("c"),
            loop_name: s("l"),
            occurrence: s("o"),
            picks: Vec::new(),
        },
        Refusal::WrittenByAnotherTable {
            table: s("t"),
            column: s("c"),
            loop_name: s("l"),
            writer: LoopWriter::Envelope,
        },
        Refusal::DuplicateSource {
            table: s("t"),
            column: s("c"),
            other: s("o"),
            loop_name: s("l"),
            occurrence: s("o"),
            place: place(),
        },
        Refusal::EnvelopeColumn {
            table: s("t"),
            column: s("c"),
            loop_name: s("l"),
            occurrence: s("o"),
        },
        Refusal::EnvelopeWithoutOccurrence {
            loop_name: s("l"),
            trigger,
        },
        Refusal::AnchoredByAnotherTable {
            table: s("t"),
            loop_name: s("l"),
            other: s("o"),
        },
        Refusal::SeveralAnchorLoops {
            table: s("t"),
            loops: Vec::new(),
        },
        Refusal::RepeatWithoutInstances {
            table: s("t"),
            loop_name: s("l"),
            segment: None,
        },
        Refusal::CodeOutsideList {
            table: s("t"),
            column: None,
            loop_name: s("l"),
            occurrence: s("o"),
            place: place(),
            value: s("v"),
            codes: Vec::new(),
        },
    ];
    let kinds: Vec<&str> = refusals.iter().map(Problem::kind).collect();
    assert_eq!(
        kinds,
        [
            "RequiredLoopWithoutSource",
            "RequiredOccurrenceWithoutSource",
            "RequiredElementWithoutSource",
            "AmbiguousSource",
            "PickNotContiguous",
            "WrittenByAnotherTable",
            "DuplicateSource",
            "EnvelopeColumn",
            "EnvelopeWithoutOccurrence",
            "AnchoredByAnotherTable",
            "SeveralAnchorLoops",
            "RepeatWithoutInstances",
            "CodeOutsideList"
        ]
    );
}

#[test]
fn spec_error_places() {
    let format = SpecError::Format {
        path: "loops.a".into(),
        source: serde_json::from_str::<u8>("x").unwrap_err(),
    };
    assert_eq!(
        (format.kind(), format.place()),
        ("Format", Place::Spec(SpecPlace::Path("loops.a")))
    );
    let occurrence = SpecError::BadOccurrence {
        loop_name: "2100".into(),
        occurrence: "x".into(),
        reason: Box::new(crate::spec_load::OccurrenceError::EmptyName),
    };
    assert_eq!(
        occurrence.place(),
        Place::Spec(SpecPlace::Loop {
            name: "2100",
            occurrence: Some("x")
        })
    );
    let patch = SpecError::Patch {
        source: Box::new(SpecError::BadTable {
            table: "t".into(),
            column: Some("c".into()),
            reason: TableDefError::NoLoops,
        }),
    };
    assert_eq!(
        (patch.kind(), patch.place()),
        ("Patch", table("t", Some("c")))
    );
    let empty = |loop_name: Option<&str>| SpecError::EmptySegmentId {
        loop_name: loop_name.map(String::from),
        key: "loops.a.trigger.segment".into(),
    };
    for error in [empty(Some("a")), empty(None)] {
        assert_eq!(
            (error.kind(), error.place()),
            (
                "EmptySegmentId",
                Place::Spec(SpecPlace::Path("loops.a.trigger.segment"))
            )
        );
    }
    let cycle = SpecError::Cycle {
        members: Vec::new(),
    };
    assert_eq!(cycle.place(), Place::Spec(SpecPlace::Whole));
    let id = PackageId::parse("x/dictionary/006020").unwrap();
    let package = SpecError::Package {
        id: Box::new(id.clone()),
        source: Box::new(SpecError::NoLoops {
            spec_name: "s".into(),
        }),
    };
    assert_eq!(package.place(), Place::Package { id: &id });
}

#[test]
fn kit_places() {
    let repeated = KitError::RepeatedCase { case: "a".into() };
    assert_eq!(
        (repeated.kind(), repeated.place()),
        (
            "RepeatedCase",
            Place::Kit {
                case: Some("a"),
                file: None
            }
        )
    );
}

#[test]
fn spec_error_kinds_name_their_variants() {
    let kinds = [
        SpecError::NoLoops {
            spec_name: "s".into(),
        },
        SpecError::UnknownParent {
            loop_name: "l".into(),
            parent: "p".into(),
        },
        SpecError::Cycle {
            members: Vec::new(),
        },
        SpecError::ZeroLoopMax {
            loop_name: "l".into(),
        },
        SpecError::UnknownDefaultTable {
            table: "t".into(),
            known: Vec::new(),
        },
        SpecError::EndIsTrigger {
            loop_name: "l".into(),
            segment: "S".into(),
        },
    ]
    .map(|e| e.kind());
    assert_eq!(
        kinds,
        [
            "NoLoops",
            "UnknownParent",
            "Cycle",
            "ZeroLoopMax",
            "UnknownDefaultTable",
            "EndIsTrigger"
        ]
    );
}

#[test]
fn summary_mask_and_case_places() {
    let overflow = SummaryError::Overflow {
        summary: "s".into(),
        table: "t".into(),
        column: "c".into(),
        row: 2,
        amount: "1".into(),
    };
    assert_eq!(
        overflow.place(),
        Place::Cell {
            table: "t",
            row: Some(2),
            column: Some("c")
        }
    );
    let short = MaskError::ShortKey { length: 3 };
    assert_eq!((short.kind(), short.place()), ("ShortKey", Place::Nowhere));
    let missing = CaseFault::MissingFile {
        path: "x.835".into(),
    };
    assert_eq!(
        (missing.kind(), missing.place()),
        (
            "MissingFile",
            Place::Kit {
                case: None,
                file: Some("x.835")
            }
        )
    );
}

#[test]
fn every_family_gives_a_record() {
    fn text_matches(problem: &impl Problem) {
        assert_eq!(problem.record().text, problem.to_string());
    }
    text_matches(&unknown_segment());
    text_matches(&delimiter(cell()));
    text_matches(&WriteError::Findings(Vec::new()));
    text_matches(&DocumentError::Size(SizeError { len: 1 }));
    text_matches(&KitError::NoCases);
    text_matches(&MaskError::ShortKey { length: 3 });
    text_matches(&SpecError::NoLoops {
        spec_name: "s".into(),
    });
    text_matches(&SummaryError::NotOneRow {
        summary: "s".into(),
        table: "t".into(),
        rows: 2,
    });
    text_matches(&CaseFault::NotText { path: "p".into() });
    text_matches(&Refusal::SeveralAnchorLoops {
        table: "t".into(),
        loops: vec!["a".into(), "b".into()],
    });
}

#[test]
fn small_family_kinds() {
    let kit = [
        KitError::FormatTooNew {
            found: 2,
            reader: 1,
        },
        KitError::NoCases,
        KitError::RepeatedCase { case: "a".into() },
        KitError::NoOutputs { case: "a".into() },
    ]
    .map(|e| e.kind());
    assert_eq!(
        kit,
        ["FormatTooNew", "NoCases", "RepeatedCase", "NoOutputs"]
    );
    let mask = [
        MaskError::ShortKey { length: 1 },
        MaskError::UnknownDateMode { given: "x".into() },
    ]
    .map(|e| e.kind());
    assert_eq!(mask, ["ShortKey", "UnknownDateMode"]);
    let case = [
        CaseFault::MissingFile { path: "p".into() },
        CaseFault::NotText { path: "p".into() },
        CaseFault::NotADocument {
            path: "p".into(),
            reason: DocumentError::Size(SizeError { len: 1 }),
        },
    ]
    .map(|e| e.kind());
    assert_eq!(case, ["MissingFile", "NotText", "NotADocument"]);
    let summary = |table: &str| table.to_string();
    let summaries = [
        SummaryError::Undeclared {
            summary: summary("s"),
            declared: Vec::new(),
        },
        SummaryError::MissingTable {
            summary: summary("s"),
            table: summary("t"),
            present: Vec::new(),
        },
        SummaryError::MissingColumn {
            summary: summary("s"),
            table: summary("t"),
            column: summary("c"),
        },
        SummaryError::NotDecimal {
            summary: summary("s"),
            table: summary("t"),
            column: summary("c"),
            found: crate::value::ColumnType::Date32,
        },
        SummaryError::NotOneRow {
            summary: summary("s"),
            table: summary("t"),
            rows: 2,
        },
        SummaryError::Overflow {
            summary: summary("s"),
            table: summary("t"),
            column: summary("c"),
            row: 0,
            amount: summary("1"),
        },
    ]
    .map(|e| e.kind());
    assert_eq!(
        summaries,
        [
            "Undeclared",
            "MissingTable",
            "MissingColumn",
            "NotDecimal",
            "NotOneRow",
            "Overflow"
        ]
    );
}
