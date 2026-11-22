//! The top level of a spec and the paths its faults report.

use super::{assert_model_faults, table_error};
use crate::message::render_key;
use crate::spec::test_support::{builtin, patched};
use crate::spec::*;
use crate::spec_load::*;
use serde_json::json;

#[test]
fn a_loop_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#"{"name":"t","loops":{"env":{"occurrences":{}}}}"#,
            "spec: at loops.env: missing field `trigger`",
        ),
        (
            r#"{"name":"t","loops":{"env":{"trigger":{"where":{"1":"X"}}}}}"#,
            "spec: at loops.env.trigger: missing field `segment`",
        ),
        (
            r#"{"name":"t","loops":{"a":"AA"}}"#,
            "spec: at loops.a: invalid type: string \"AA\", expected a loop object",
        ),
        (
            r#"{"name":"t","loops":{"a":{"trigger":["AA"]}}}"#,
            "spec: at loops.a.trigger: invalid type: sequence, expected a trigger object",
        ),
        (
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":1}}}}"#,
            "spec: at loops.a.trigger.where: invalid type: integer `1`, expected a map",
        ),
        (
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":null}}}}"#,
            "spec: at loops.a.trigger.where: invalid type: null, expected a map",
        ),
    ];
    for (json, expected) in cases {
        let err = Spec::from_json(json).unwrap_err();
        assert!(matches!(&err, SpecError::Format { .. }), "{json}: {err:?}");
        assert_eq!(err.to_string(), expected, "{json}");
    }
}

#[test]
fn unknown_parent_is_rejected_with_both_names() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"parent":"ghost","trigger":{"segment":"AA"}}}}"#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, SpecError::UnknownParent { loop_name, parent } if loop_name == "a" && parent == "ghost"),
        "{err}"
    );
}

#[test]
fn a_patched_spec_goes_through_the_same_model_read() {
    let err = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"2100":{"trigger":["CLP"]}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: spec: at loops.2100.trigger: invalid type: sequence, expected a trigger object"
    );
}

#[test]
fn keys_with_a_separator_or_whitespace_are_quoted_in_paths() {
    assert_eq!(render_key("2100"), "2100");
    assert_eq!(render_key("a.b"), "\"a.b\"");
    assert_eq!(render_key("x/y"), "\"x/y\"");
    assert_eq!(render_key("x#2"), "\"x#2\"");
    assert_eq!(render_key("a b"), "\"a b\"");
    assert_eq!(render_key(""), "\"\"");
}

#[test]
fn a_spec_error_path_quotes_a_loop_name_that_holds_a_dot() {
    let plain = Spec::from_json(r#"{"name":"t","loops":{"ab":{"trigger":[]}}}"#).unwrap_err();
    assert!(
        matches!(&plain, SpecError::Format { path, .. } if path == "loops.ab.trigger"),
        "{plain:?}"
    );
    let quoted = Spec::from_json(r#"{"name":"t","loops":{"a.b":{"trigger":[]}}}"#).unwrap_err();
    assert!(
        matches!(&quoted, SpecError::Format { path, .. } if path == "loops.\"a.b\".trigger"),
        "{quoted:?}"
    );
    let wrong = Spec::from_json(
        r#"{"name":"t","loops":{"a b":{"trigger":{"segment":"AA","where":{"1":2}}}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        wrong.to_string(),
        "spec: at loops.\"a b\".trigger.where.1: invalid type: integer `2`, expected a string"
    );
}

#[test]
fn an_empty_segment_id_path_quotes_a_table_or_column_name_that_holds_a_dot() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"A":{"trigger":{"segment":"AA"}}},
                "tables":{"a.b":{"loops":["A"],"columns":{"c d":{"segment":"","element":1}}}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "the spec has an empty segment id at tables.\"a.b\".columns.\"c d\".segment"
    );
}

#[test]
fn a_spec_without_loops_is_rejected() {
    let err = Spec::from_json(r#"{"name":"t","loops":{}}"#).unwrap_err();
    assert!(
        matches!(&err, SpecError::NoLoops { spec_name } if spec_name == "t"),
        "{err:?}"
    );
}

#[test]
fn a_table_value_of_the_wrong_kind_fails_at_its_path() {
    let cases = [
        (
            r#"{"claims":{"loops":["A",2]}}"#,
            "spec: at tables.claims.loops[1]: invalid type: integer `2`, expected a string",
        ),
        (
            r#"{"claims":{"loops":"A"}}"#,
            "spec: at tables.claims.loops: invalid type: string \"A\", expected a sequence",
        ),
        (
            r#"{"claims":{"loops":["A"],"repeat":{"from":"2","step":3}}}"#,
            "spec: at tables.claims.repeat.from: invalid type: string \"2\", expected usize",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":"1"}}}}"#,
            "spec: at tables.claims.columns.x.element: invalid type: string \"1\", expected usize",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment_index":1}}}}"#,
            "spec: at tables.claims.columns.x.segment_index: invalid type: integer `1`, expected a boolean",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":1,"where":{"1":2}}}}}"#,
            "spec: at tables.claims.columns.x.where.1: invalid type: integer `2`, expected a string",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":7,"element":1}}}}"#,
            "spec: at tables.claims.columns.x.occurrence: invalid type: integer `7`, expected a string",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":"o","pick":true,"element":1}}}}"#,
            "spec: at tables.claims.columns.x.pick: \"pick\" must be \"first\", \"last\" or a 1-based position; found true",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":"o","pick":-2,"element":1}}}}"#,
            "spec: at tables.claims.columns.x.pick: \"pick\" must be \"first\", \"last\" or a 1-based position; found -2",
        ),
    ];
    assert_model_faults(&cases, table_error);
}

#[test]
fn a_top_level_value_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#"{"name":7,"loops":{"a":{"trigger":{"segment":"AA"}}}}"#,
            "spec: at name: invalid type: integer `7`, expected a string",
        ),
        (
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"default_table":null}"#,
            "spec: at default_table: invalid type: null, expected a string",
        ),
        (
            r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#,
            "spec: at .: missing field `name`",
        ),
        (r#"{"name":"t"}"#, "spec: at .: missing field `loops`"),
    ];
    assert_model_faults(&cases, |json| Spec::from_json(json).unwrap_err());
}

#[test]
fn a_spec_or_a_section_that_is_not_an_object_fails_at_its_key() {
    let mut cases = Vec::new();
    for (json, found) in [
        ("[]", "sequence"),
        ("\"835\"", "string \"835\""),
        ("null", "null"),
        ("7", "integer `7`"),
    ] {
        let expected = "expected a spec object with \"name\" and \"loops\"";
        cases.push((
            json.to_string(),
            format!("spec: at .: invalid type: {found}, {expected}"),
        ));
    }
    for section in ["loops", "tables", "balancing", "segments", "summaries"] {
        for (value, found) in [
            (json!([]), "sequence"),
            (json!(null), "null"),
            (json!("x"), "string \"x\""),
        ] {
            let mut spec = json!({"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}}});
            spec[section] = value;
            let text = format!("spec: at {section}: invalid type: {found}, expected a map");
            cases.push((spec.to_string(), text));
        }
    }
    let cases: Vec<(&str, &str)> = cases
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    assert_model_faults(&cases, |json| Spec::from_json(json).unwrap_err());
}

#[test]
fn patch_adds_a_loop() {
    let spec = patched(&builtin(Version::V5010),
        r#"{"loops":{"2100-ZZ":{"parent":"2100","trigger":{"segment":"ZZ1"},"occurrences":{"zz1":{"segment":"ZZ1","pos":0},"zz2":{"segment":"ZZ2","pos":1}}}}}"#,
    )
    .unwrap();
    assert_eq!(spec.loops().len(), 9);
    let zz = spec.loop_id("2100-ZZ").unwrap();
    assert_eq!(spec.get(zz).parent, spec.loop_id("2100"));
    assert!(spec.children(spec.loop_id("2100")).contains(&zz));
}
