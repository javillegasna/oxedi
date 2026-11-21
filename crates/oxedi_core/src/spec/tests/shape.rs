//! The JSON shape checks and the paths they report.

use super::table_error;
use crate::message::render_key;
use crate::spec::shape::kind_of;
use crate::spec::*;

#[test]
fn a_missing_required_key_is_rejected_with_its_key_path() {
    let json = r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#;
    let err = Spec::from_json(json).unwrap_err();
    assert!(matches!(&err, SpecError::MissingKey { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "spec: missing required key \"name\" at the top level"
    );
}

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
fn unknown_and_missing_keys_display_the_key_and_the_path() {
    let cases = [
        (
            SpecError::UnknownKey {
                path: "segments.CLP.elements.3".into(),
                key: "lenght".into(),
                allowed: vec!["max", "min", "name"],
            },
            "spec: unknown key \"lenght\" at segments.CLP.elements.3; the allowed keys are: \
             max, min, name",
        ),
        (
            SpecError::UnknownKey {
                path: String::new(),
                key: "tabels".into(),
                allowed: vec!["loops", "name"],
            },
            "spec: unknown key \"tabels\" at the top level; the allowed keys are: loops, name",
        ),
        (
            SpecError::MissingKey {
                path: "loops.env.control".into(),
                key: "count_element",
            },
            "spec: missing required key \"count_element\" at loops.env.control",
        ),
        (
            SpecError::MissingKey {
                path: String::new(),
                key: "loops",
            },
            "spec: missing required key \"loops\" at the top level",
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
        assert!(std::error::Error::source(&err).is_none());
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
fn a_top_level_without_loops_names_the_missing_key() {
    let err = Spec::from_json(r#"{"name":"t"}"#).unwrap_err();
    assert!(
        matches!(&err, SpecError::MissingKey { path, key: "loops" } if path.is_empty()),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "spec: missing required key \"loops\" at the top level"
    );
}

#[test]
fn a_spec_that_is_not_an_object_says_so_in_plain_words() {
    let err = Spec::from_json("[]").unwrap_err();
    assert!(
        matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path.is_empty()),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "the spec must be a JSON object; found an array"
    );
}

#[test]
fn loops_that_are_not_an_object_are_refused_by_the_walk() {
    let err = Spec::from_json(r#"{"name":"t","loops":[]}"#).unwrap_err();
    assert!(
        matches!(&err, SpecError::NotAnObject { path, found } if path == "loops" && *found == "an array"),
        "{err:?}"
    );
}

#[test]
fn a_patched_spec_goes_through_the_same_shape_check() {
    let err = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"2100":{"trigger":["CLP"]}}}"#)
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
fn every_kind_of_json_value_is_named() {
    use serde_json::json;
    assert_eq!(kind_of(&json!(null)), "null");
    assert_eq!(kind_of(&json!(true)), "a boolean");
    assert_eq!(kind_of(&json!(1)), "a number");
    assert_eq!(kind_of(&json!("x")), "a string");
    assert_eq!(kind_of(&json!([])), "an array");
    assert_eq!(kind_of(&json!({})), "an object");
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
fn a_table_scalar_of_the_wrong_kind_is_rejected_with_its_key_path() {
    let cases = [
        (
            r#"{"claims":{"loops":["A",2]}}"#,
            "spec: the value at tables.claims.loops[1] must be a string; found a number (2)",
        ),
        (
            r#"{"claims":{"loops":"A"}}"#,
            "spec: the value at tables.claims.loops must be an array of strings; found a string (\"A\")",
        ),
        (
            r#"{"claims":{"loops":["A"],"repeat":{"from":"2","step":3}}}"#,
            "spec: the value at tables.claims.repeat.from must be a non-negative integer; found a string (\"2\")",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":"1"}}}}"#,
            "spec: the value at tables.claims.columns.x.element must be a non-negative integer; found a string (\"1\")",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment_index":1}}}}"#,
            "spec: the value at tables.claims.columns.x.segment_index must be a boolean; found a number (1)",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":1,"where":{"1":2}}}}}"#,
            "spec: the value at tables.claims.columns.x.where.1 must be a string; found a number (2)",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":7,"element":1}}}}"#,
            "spec: the value at tables.claims.columns.x.occurrence must be a string; found a number (7)",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":"o","pick":true,"element":1}}}}"#,
            "spec: the value at tables.claims.columns.x.pick must be \"first\", \"last\" or a 1-based position; found a boolean (true)",
        ),
        (
            r#"{"claims":{"loops":["A"],"columns":{"x":{"occurrence":"o","pick":-2,"element":1}}}}"#,
            "spec: the value at tables.claims.columns.x.pick must be \"first\", \"last\" or a 1-based position; found a negative number (-2)",
        ),
    ];
    for (tables, expected) in cases {
        let err = table_error(tables);
        assert!(
            matches!(&err, SpecError::WrongType { .. }),
            "{tables}: {err:?}"
        );
        assert_eq!(err.to_string(), expected, "{tables}");
    }
}

#[test]
fn wrong_type_displays_the_path_what_is_required_and_what_was_found() {
    let err = SpecError::WrongType {
        path: "loops.env.control.opener_element".into(),
        expected: "a non-negative integer",
        found: "a string",
        value: "\"2\"".into(),
    };
    assert_eq!(
        err.to_string(),
        "spec: the value at loops.env.control.opener_element must be a non-negative integer; found a string (\"2\")"
    );
    let null = SpecError::WrongType {
        path: "loops.env.end".into(),
        expected: "a string",
        found: "null",
        value: String::new(),
    };
    assert_eq!(
        null.to_string(),
        "spec: the value at loops.env.end must be a string; found null"
    );
}

#[test]
fn patch_adds_a_loop() {
    let spec = Spec::builtin_835().merge_patch(
        r#"{"loops":{"2100-ZZ":{"parent":"2100","trigger":{"segment":"ZZ1"},"occurrences":{"zz1":{"segment":"ZZ1","pos":0},"zz2":{"segment":"ZZ2","pos":1}}}}}"#,
    )
    .unwrap();
    assert_eq!(spec.loops().len(), 9);
    let zz = spec.loop_id("2100-ZZ").unwrap();
    assert_eq!(spec.get(zz).parent, spec.loop_id("2100"));
    assert!(spec.children(spec.loop_id("2100")).contains(&zz));
}
