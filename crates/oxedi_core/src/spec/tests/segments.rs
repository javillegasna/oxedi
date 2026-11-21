//! Segment and element definitions.

use super::{CLP_ONLY, assert_model_faults, element_error};
use crate::spec::*;
use crate::value::{ElementType, LengthRange, TypeCodeError};

#[test]
fn an_empty_segment_id_in_the_segments_section_is_rejected() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"":{"elements":{}}}}"#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == "segments.\"\""),
        "{err:?}"
    );
}

#[test]
fn non_numeric_zero_or_non_canonical_positions_are_rejected() {
    for position in ["x", "0", "-1", "01", "+1", " 1"] {
        let json = format!(
            r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA","where":{{"{position}":"1"}}}}}}}}}}"#
        );
        let err = Spec::from_json(&json).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "spec: at loops.a.trigger.where: trigger \"where\" key {position:?} is not a position: positions are 1-based integers written in canonical form"
            )
        );
    }
}

#[test]
fn a_position_spelled_twice_is_rejected_by_its_non_canonical_spelling() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":{"1":"X","01":"Y"}}}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "spec: at loops.a.trigger.where: trigger \"where\" key \"01\" is not a position: positions are 1-based integers written in canonical form"
    );
}

#[test]
fn ambiguous_trigger_message_shows_the_parent_and_the_trigger() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "a":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "b":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\"}"
    );
    let err = Spec::from_json(
        r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA"}},
                "b":{"trigger":{"segment":"AA"}}
            }}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
    );
}

#[test]
fn segments_are_keyed_by_id_and_elements_by_position() {
    let spec = Spec::from_json(CLP_ONLY).unwrap();
    let clp = spec.segment(b"CLP").unwrap();
    let first = &clp.elements[&1];
    assert_eq!(first.name, "claim_submitter_id");
    assert_eq!(first.kind, ElementType::An);
    assert!(first.required);
    assert_eq!(
        first.length,
        LengthRange {
            min: Some(1),
            max: Some(38)
        }
    );
    assert_eq!(clp.elements[&3].kind, ElementType::R { scale: 2 });
    assert!(!clp.elements[&12].required, "required defaults to false");
    assert_eq!(clp.elements[&12].kind, ElementType::R { scale: 4 });
    assert_eq!(
        clp.elements.keys().copied().collect::<Vec<_>>(),
        vec![1, 3, 12]
    );
    let svc = spec.segment(b"SVC").unwrap();
    let procedure = &svc.elements[&1];
    assert_eq!(procedure.composite[&1].kind, ElementType::Id);
    assert_eq!(procedure.composite[&2].name, "code");
    assert_eq!(svc.elements[&5].kind, ElementType::N(0));
    let ids: Vec<&[u8]> = spec.segments().map(|(id, _)| id).collect();
    assert_eq!(ids, vec![&b"CLP"[..], &b"SVC"[..]]);
}

#[test]
fn every_type_code_is_read() {
    let cases = [
        ("AN", None, ElementType::An),
        ("ID", None, ElementType::Id),
        ("N0", None, ElementType::N(0)),
        ("N2", None, ElementType::N(2)),
        ("N9", None, ElementType::N(9)),
        ("R", None, ElementType::R { scale: 2 }),
        ("R", Some(6), ElementType::R { scale: 6 }),
        ("DT", None, ElementType::Dt),
        ("TM", None, ElementType::Tm),
    ];
    for (code, scale, expected) in cases {
        assert_eq!(ElementType::parse(code, scale), Ok(expected), "{code}");
    }
    for code in ["an", "N", "N10", "NA", "R2", "", "B"] {
        assert_eq!(
            ElementType::parse(code, None),
            Err(TypeCodeError::UnknownType {
                found: code.to_string()
            }),
            "{code:?}"
        );
    }
}

#[test]
fn element_types_display_their_code_and_meaning() {
    assert_eq!(ElementType::An.to_string(), "AN (string)");
    assert_eq!(ElementType::Id.to_string(), "ID (code)");
    assert_eq!(
        ElementType::N(2).to_string(),
        "N2 (integer with 2 implied decimals)"
    );
    assert_eq!(
        ElementType::R { scale: 2 }.to_string(),
        "R (decimal, scale 2)"
    );
    assert_eq!(ElementType::Dt.to_string(), "DT (date CCYYMMDD or YYMMDD)");
    assert_eq!(
        ElementType::Tm.to_string(),
        "TM (time HHMM, HHMMSS or HHMMSSD..)"
    );
}

#[test]
fn a_segment_a_loop_lists_without_a_definition_stays_opaque() {
    let spec = Spec::from_json(CLP_ONLY).unwrap();
    assert!(spec.get(spec.loop_id("2100").unwrap()).accepts(b"ZZ1"));
    assert_eq!(spec.segment(b"ZZ1"), None);
}

#[test]
fn a_spec_without_segments_has_none() {
    let spec =
        Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}}}"#).unwrap();
    assert_eq!(spec.segments().count(), 0);
}

#[test]
fn bad_element_definitions_are_rejected_with_segment_position_and_reason() {
    let cases = [
        (
            r#"{"1":{"name":"","type":"AN"}}"#,
            "1",
            ElementDefError::EmptyName,
        ),
        (
            r#"{"1":{"name":"a","type":"AN"},"2":{"name":"a","type":"ID"}}"#,
            "2",
            ElementDefError::DuplicateName {
                name: "a".into(),
                first: "1".into(),
            },
        ),
        (
            r#"{"1":{"name":"a","type":"AN","min":5,"max":2}}"#,
            "1",
            ElementDefError::MinAboveMax { min: 5, max: 2 },
        ),
        (
            r#"{"1":{"name":"a","type":"ID","composite":{"1":{"name":"b","type":"AN"}}}}"#,
            "1",
            ElementDefError::CompositeOnNonAn { kind: "ID".into() },
        ),
        (
            r#"{"1":{"name":"a","type":"AN","composite":{"2":{"name":"b","type":"AN","composite":{"1":{"name":"c","type":"AN"}}}}}}"#,
            "1.composite.2",
            ElementDefError::NestedComposite,
        ),
        (
            r#"{"1":{"name":"a","type":"AN","composite":{"1":{"name":"b","type":"AN"},"2":{"name":"b","type":"AN"}}}}"#,
            "1.composite.2",
            ElementDefError::DuplicateName {
                name: "b".into(),
                first: "1".into(),
            },
        ),
        (
            r#"{"10":{"name":"a","type":"AN"},"2":{"name":"a","type":"AN"}}"#,
            "10",
            ElementDefError::DuplicateName {
                name: "a".into(),
                first: "2".into(),
            },
        ),
    ];
    for (elements, expected_position, expected_reason) in cases {
        let err = element_error(elements);
        assert!(
            matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == expected_position && *reason == expected_reason),
            "{elements}: {err:?}"
        );
    }
}

#[test]
fn names_only_need_to_be_unique_among_siblings() {
    let ok = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{
                "1":{"name":"code","type":"AN","composite":{"1":{"name":"code","type":"AN"}}}
            }}}}"#,
    );
    assert!(ok.is_ok(), "{ok:?}");
}

#[test]
fn a_segment_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#"{"AA":{"elemnts":{}}}"#,
            "spec: at segments.AA.elemnts: unknown field `elemnts`, expected `elements`",
        ),
        (
            r#"{"CLP":[]}"#,
            "spec: at segments.CLP: invalid type: sequence, expected a segment object",
        ),
        (
            r#"{"AA":null}"#,
            "spec: at segments.AA: invalid type: null, expected a segment object",
        ),
    ];
    assert_model_faults(&cases, segments_error);
}

fn segments_error(segments: &str) -> SpecError {
    let json = format!(
        r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{segments}}}"#
    );
    Spec::from_json(&json).unwrap_err()
}

#[test]
fn to_json_round_trips_the_segments_section() {
    let spec = Spec::from_json(CLP_ONLY).unwrap();
    let again = Spec::from_json(&spec.to_json()).unwrap();
    assert!(spec.segments().eq(again.segments()));
    assert_eq!(again.segments().count(), 2);
}

#[test]
fn an_element_the_model_refuses_fails_at_its_path() {
    let unknown = "type \"XX\" is not one of AN, ID, N0 to N9, R, DT, TM";
    let rule = "is not a position: positions are 1-based integers written in canonical form";
    let keys =
        "expected one of `name`, `type`, `scale`, `required`, `min`, `max`, `codes`, `composite`";
    let cases = [
        (
            r#"{"3":{"name":"p","type":"AN","min":"1"}}"#,
            "spec: at segments.AA.elements.3.min: invalid type: string \"1\", expected usize".to_string(),
        ),
        (
            r#"{"3":{"name":"p","type":"R","scale":300}}"#,
            "spec: at segments.AA.elements.3: invalid value: integer `300`, expected u8".to_string(),
        ),
        (
            r#"{"3":{"name":"p","type":"AN","required":"yes"}}"#,
            "spec: at segments.AA.elements.3.required: invalid type: string \"yes\", expected a boolean".to_string(),
        ),
        (
            r#"{"3":{"name":5,"type":"AN"}}"#,
            "spec: at segments.AA.elements.3.name: invalid type: integer `5`, expected a string".to_string(),
        ),
        (
            r#"{"1":{"name":"c","type":"AN","composite":{"2":{"name":"x","type":"AN","max":1.5}}}}"#,
            "spec: at segments.AA.elements.1.composite.2.max: invalid type: floating point `1.5`, expected usize".to_string(),
        ),
        (
            r#"{"3":{"name":"p","type":"AN","codes":null}}"#,
            "spec: at segments.AA.elements.3.codes: invalid type: null, expected a sequence".to_string(),
        ),
        (
            r#"{"3":{"name":"p","type":"AN","max":2.0}}"#,
            "spec: at segments.AA.elements.3.max: invalid type: floating point `2.0`, expected usize".to_string(),
        ),
        (
            r#"{"3":{"name":"a","type":"AN","lenght":3}}"#,
            format!("spec: at segments.AA.elements.3: unknown field `lenght`, {keys}"),
        ),
        (
            r#"{"1":{"name":"c","type":"AN","composite":{"2":{"name":"x","typ":"AN"}}}}"#,
            format!("spec: at segments.AA.elements.1.composite.2: unknown field `typ`, {keys}"),
        ),
        (
            r#"{"3":{"name":"a"}}"#,
            "spec: at segments.AA.elements.3: missing field `type`".to_string(),
        ),
        (
            r#"{"1":{"name":"c","type":"AN","composite":{"2":{"type":"AN"}}}}"#,
            "spec: at segments.AA.elements.1.composite.2: missing field `name`".to_string(),
        ),
        (
            r#"[]"#,
            "spec: at segments.AA.elements: invalid type: sequence, expected a map".to_string(),
        ),
        (
            r#"{"1":"claim_id"}"#,
            "spec: at segments.AA.elements.1: invalid type: string \"claim_id\", expected an element object".to_string(),
        ),
        (
            r#"{"1":{"name":"p","type":"AN","composite":{"2":7}}}"#,
            "spec: at segments.AA.elements.1.composite.2: invalid type: integer `7`, expected an element object".to_string(),
        ),
        (
            r#"{"1":{"name":"a","type":"XX"}}"#,
            format!("spec: at segments.AA.elements.1: {unknown}"),
        ),
        (
            r#"{"1":{"name":"a","type":"N2","scale":2}}"#,
            "spec: at segments.AA.elements.1: \"scale\" applies only to type R; found type \"N2\""
                .to_string(),
        ),
        (
            r#"{"1":{"name":"a","type":"AN","composite":{"2":{"name":"b","type":"XX"}}}}"#,
            format!("spec: at segments.AA.elements.1.composite.2: {unknown}"),
        ),
        (
            r#"{"01":{"name":"a","type":"AN"}}"#,
            format!("spec: at segments.AA.elements: segment \"elements\" key \"01\" {rule}"),
        ),
        (
            r#"{"0":{"name":"a","type":"AN"}}"#,
            format!("spec: at segments.AA.elements: segment \"elements\" key \"0\" {rule}"),
        ),
        (
            r#"{"1":{"name":"a","type":"AN","composite":{"0":{"name":"b","type":"AN"}}}}"#,
            format!(
                "spec: at segments.AA.elements.1.composite: element \"composite\" key \"0\" {rule}"
            ),
        ),
        (
            r#"{"1":{"name":"","type":"AN"},"2":{"name":"b","type":"XX"}}"#,
            format!("spec: at segments.AA.elements.2: {unknown}"),
        ),
    ];
    for (elements, text) in cases {
        let err = element_error(elements);
        assert!(
            matches!(err, SpecError::Format { .. }),
            "{elements}: {err:?}"
        );
        assert_eq!(err.to_string(), text, "{elements}");
    }
}
