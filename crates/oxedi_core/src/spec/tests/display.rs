//! The full text of every error and diagnostic.

use super::element_error;
use crate::spec::*;
use crate::value::ElementType;
use serde_json::Value;

fn json_error(text: &str) -> serde_json::Error {
    serde_json::from_str::<Value>(text).unwrap_err()
}

/// Pins an error's full `Display` text and that it chains no source.
fn assert_displays_without_source(err: &SpecError, text: &str) {
    assert_eq!(err.to_string(), text);
    assert!(std::error::Error::source(err).is_none());
}

#[test]
fn json_error_displays_the_parser_message() {
    let err = SpecError::Json(json_error("{"));
    assert_displays_without_source(
        &err,
        "invalid spec JSON: EOF while parsing an object at line 1 column 1",
    );
}

#[test]
fn format_error_displays_the_path_and_the_refused_value() {
    let err = SpecError::Format {
        path: "loops.\"2100.a\".usage".into(),
        source: json_error("{"),
    };
    assert_displays_without_source(
        &err,
        "spec: at loops.\"2100.a\".usage: EOF while parsing an object at line 1 column 1",
    );
}

#[test]
fn patch_error_displays_the_inner_error_once() {
    let err = SpecError::Patch {
        source: Box::new(SpecError::EmptySegmentId {
            loop_name: Some("a".into()),
            key: "end".into(),
        }),
    };
    assert_eq!(
        err.to_string(),
        "applying patch: loop \"a\" has an empty segment id at end"
    );
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn unknown_parent_displays_both_names() {
    let err = SpecError::UnknownParent {
        loop_name: "2100".into(),
        parent: "2000".into(),
    };
    assert_eq!(
        err.to_string(),
        "loop \"2100\" names unknown parent \"2000\""
    );
}

#[test]
fn cycle_displays_the_walk() {
    let err = SpecError::Cycle {
        members: vec!["b".into(), "c".into()],
    };
    assert_eq!(err.to_string(), "loops form a parent cycle: b -> c -> b");
}

#[test]
fn no_loops_displays_the_spec_name() {
    let err = SpecError::NoLoops {
        spec_name: "name".into(),
    };
    assert_eq!(err.to_string(), "spec \"name\" declares no loops");
}

#[test]
fn empty_segment_id_displays_the_loop_and_the_key() {
    let err = SpecError::EmptySegmentId {
        loop_name: Some("2100".into()),
        key: "occurrences.ref.segment".into(),
    };
    assert_eq!(
        err.to_string(),
        "loop \"2100\" has an empty segment id at occurrences.ref.segment"
    );
    let err = SpecError::EmptySegmentId {
        loop_name: None,
        key: "segments.\"\"".into(),
    };
    assert_eq!(
        err.to_string(),
        "the spec has an empty segment id at segments.\"\""
    );
}

#[test]
fn a_loop_ending_on_its_own_trigger_is_rejected() {
    let err =
        Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"end":"AA"}}}"#)
            .unwrap_err();
    assert!(
        matches!(&err, SpecError::EndIsTrigger { loop_name, segment } if loop_name == "a" && segment == "AA"),
        "{err:?}"
    );
    assert!(
        Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"end":"BB"}}}"#)
            .is_ok()
    );
}

#[test]
fn end_is_trigger_displays_the_loop_the_segment_and_why() {
    let err = SpecError::EndIsTrigger {
        loop_name: "a".into(),
        segment: "AA".into(),
    };
    assert_eq!(
        err.to_string(),
        "loop \"a\" has \"end\" \"AA\", the same segment as its \"trigger\": the loop would close on the segment that opens it"
    );
}

#[test]
fn ambiguous_trigger_displays_parent_segment_and_conditions() {
    let err = SpecError::AmbiguousTrigger {
        first: "a".into(),
        second: "b".into(),
        parent: Some("transaction".into()),
        segment: "N1".into(),
        conditions: vec![(1, "PR".into()), (3, "XX".into())],
    };
    assert_eq!(
        err.to_string(),
        "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\", 3: \"XX\"}"
    );
    let err = SpecError::AmbiguousTrigger {
        first: "a".into(),
        second: "b".into(),
        parent: None,
        segment: "AA".into(),
        conditions: Vec::new(),
    };
    assert_eq!(
        err.to_string(),
        "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
    );
}

#[test]
fn bad_element_def_displays_segment_position_and_every_reason() {
    let cases = [
        (
            ElementDefError::EmptyName,
            "segment \"CLP\" element \"01\": \"name\" is empty",
        ),
        (
            ElementDefError::DuplicateName {
                name: "claim_id".into(),
                first: "1".into(),
            },
            "segment \"CLP\" element \"01\": name \"claim_id\" is already used by position \"1\"",
        ),
        (
            ElementDefError::MinAboveMax { min: 5, max: 2 },
            "segment \"CLP\" element \"01\": \"min\" 5 is greater than \"max\" 2",
        ),
        (
            ElementDefError::CompositeOnNonAn { kind: "ID".into() },
            "segment \"CLP\" element \"01\": \"composite\" requires type AN; found type \"ID\"",
        ),
        (
            ElementDefError::NestedComposite,
            "segment \"CLP\" element \"01\": a component cannot declare its own \"composite\"",
        ),
    ];
    for (reason, expected) in cases {
        let err = SpecError::BadElementDef {
            segment: "CLP".into(),
            position: "01".into(),
            reason,
        };
        assert_eq!(err.to_string(), expected);
        assert!(std::error::Error::source(&err).is_none());
    }
}

#[test]
fn an_r_scale_above_18_or_a_zero_max_is_rejected_with_the_value() {
    let cases = [
        (
            r#"{"3":{"name":"a","type":"R","scale":19}}"#,
            ElementDefError::ScaleAboveMaximum { scale: 19 },
        ),
        (
            r#"{"3":{"name":"a","type":"R","scale":200}}"#,
            ElementDefError::ScaleAboveMaximum { scale: 200 },
        ),
        (
            r#"{"3":{"name":"a","type":"AN","max":0}}"#,
            ElementDefError::ZeroMax,
        ),
        (
            r#"{"3":{"name":"a","type":"R","min":0,"max":0}}"#,
            ElementDefError::ZeroMax,
        ),
    ];
    for (elements, expected_reason) in cases {
        let err = element_error(elements);
        assert!(
            matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == "3" && *reason == expected_reason),
            "{elements}: {err:?}"
        );
    }
    let at_the_cap = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"R","scale":18,"max":1}}}}}"#,
    )
    .unwrap();
    assert_eq!(
        at_the_cap.segment(b"AA").unwrap().elements[&1].kind,
        ElementType::R { scale: 18 }
    );
}

#[test]
fn scale_and_max_reasons_display_segment_position_and_value() {
    let at = |reason| SpecError::BadElementDef {
        segment: "CLP".into(),
        position: "12".into(),
        reason,
    };
    assert_eq!(
        at(ElementDefError::ScaleAboveMaximum { scale: 19 }).to_string(),
        "segment \"CLP\" element \"12\": \"scale\" 19 is above the maximum of 18"
    );
    assert_eq!(
        at(ElementDefError::ZeroMax).to_string(),
        "segment \"CLP\" element \"12\": \"max\" is 0; an element holds at least one character"
    );
}

#[test]
fn no_spec_error_chains_its_source() {
    use std::error::Error;
    let patch = SpecError::Patch {
        source: Box::new(SpecError::Json(json_error("{"))),
    };
    let format = SpecError::Format {
        path: ".".into(),
        source: json_error("{"),
    };
    let bad_summary = SpecError::BadSummary {
        summary: "s".into(),
        reason: Box::new(SummaryDefError::EmptyName),
    };
    let default_table = SpecError::UnknownDefaultTable {
        table: "t".into(),
        known: Vec::new(),
    };
    for err in [
        SpecError::Json(json_error("{")),
        patch,
        format,
        bad_summary,
        default_table,
    ] {
        assert!(err.source().is_none(), "{err}");
    }
}

#[test]
fn summary_errors_display_the_summary_and_the_reason() {
    let bad = SpecError::BadSummary {
        summary: "payer".into(),
        reason: Box::new(SummaryDefError::NoFields {
            key: "row.fields".into(),
        }),
    };
    assert_eq!(
        bad.to_string(),
        "summary \"payer\": row.fields lists no field"
    );
    let default_table = SpecError::UnknownDefaultTable {
        table: "claim".into(),
        known: vec!["claims".into(), "payments".into()],
    };
    assert_eq!(
        default_table.to_string(),
        "default_table names the table \"claim\", which the spec does not define; its tables \
         are: claims, payments"
    );
}

#[test]
fn unknown_table_display() {
    let error = UnknownTable {
        name: "nope".to_owned(),
        tables: vec!["claims".to_owned(), "services".to_owned()],
    };
    assert_eq!(
        error.to_string(),
        "table \"nope\" is not a table of the spec; its tables are \"claims\", \"services\""
    );
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn unknown_column_display() {
    let error = UnknownColumn {
        table: "claims".to_owned(),
        column: "nope".to_owned(),
        columns: vec!["id".to_owned(), "status".to_owned()],
    };
    assert_eq!(
        error.to_string(),
        "table \"claims\" has no column \"nope\" in the spec; its columns are \"id\", \"status\""
    );
    assert!(std::error::Error::source(&error).is_none());
}
