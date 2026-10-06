//! Loop control declarations.

use crate::spec::*;

fn control_error(control: &str) -> SpecError {
    let json = format!(
        r#"{{"name":"t","loops":{{"env":{{"trigger":{{"segment":"HD"}},"end":"TR","control":{control}}}}}}}"#
    );
    Spec::from_json(&json).unwrap_err()
}

#[test]
fn builtin_835_declares_the_envelope_controls() {
    let spec = Spec::builtin_835();
    let control = |name: &str| spec.get(spec.loop_id(name).unwrap()).control;
    assert_eq!(
        control("interchange"),
        Some(Control {
            opener_element: 13,
            closer_element: 2,
            count_element: 1,
            count: ControlCount::Children,
        })
    );
    assert_eq!(
        control("group"),
        Some(Control {
            opener_element: 6,
            closer_element: 2,
            count_element: 1,
            count: ControlCount::Children,
        })
    );
    assert_eq!(
        control("transaction"),
        Some(Control {
            opener_element: 2,
            closer_element: 2,
            count_element: 1,
            count: ControlCount::Segments,
        })
    );
    assert_eq!(control("2100"), None);
}

#[test]
fn bad_controls_are_rejected_with_the_loop_and_the_reason() {
    let cases = [
        (
            r#"{"opener_element":0,"closer_element":2,"count_element":1,"count":"segments"}"#,
            ControlError::ZeroPosition {
                key: "opener_element",
            },
        ),
        (
            r#"{"opener_element":2,"closer_element":2,"count_element":0,"count":"segments"}"#,
            ControlError::ZeroPosition {
                key: "count_element",
            },
        ),
        (
            r#"{"opener_element":2,"closer_element":2,"count_element":1,"count":"segs"}"#,
            ControlError::UnknownCount {
                found: "segs".into(),
            },
        ),
    ];
    for (control, expected) in cases {
        let err = control_error(control);
        assert!(
            matches!(&err, SpecError::BadControl { loop_name, reason } if loop_name == "env" && *reason == expected),
            "{control}: {err:?}"
        );
    }
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"env":{"trigger":{"segment":"HD"},"control":{"opener_element":2,"closer_element":2,"count_element":1,"count":"segments"}}}}"#,
    )
    .unwrap_err();
    assert!(
        matches!(
            &err,
            SpecError::BadControl {
                reason: ControlError::NoEnd,
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_control_that_is_not_an_object_or_misses_a_key_is_rejected() {
    let err = control_error("[2,2,1]");
    assert!(
        matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path == "loops.env.control"),
        "{err:?}"
    );
    let err = control_error(r#"{"opener_element":2,"closer_element":2,"count":"segments"}"#);
    assert!(
        matches!(&err, SpecError::MissingKey { path, key: "count_element" } if path == "loops.env.control"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "spec: missing required key \"count_element\" at loops.env.control"
    );
}

#[test]
fn bad_control_displays_the_loop_and_every_reason() {
    let cases = [
        (
            ControlError::ZeroPosition {
                key: "opener_element",
            },
            "loop \"transaction\" has an invalid \"control\": \"opener_element\" must be a 1-based element position; found 0",
        ),
        (
            ControlError::UnknownCount {
                found: "segs".into(),
            },
            "loop \"transaction\" has an invalid \"control\": \"count\" must be \"segments\" or \"children\"; found \"segs\"",
        ),
        (
            ControlError::NoEnd,
            "loop \"transaction\" has an invalid \"control\": the loop has no \"end\" segment to check",
        ),
    ];
    for (reason, expected) in cases {
        let err = SpecError::BadControl {
            loop_name: "transaction".into(),
            reason,
        };
        assert_eq!(err.to_string(), expected);
        assert!(std::error::Error::source(&err).is_none());
    }
}

#[test]
fn a_scalar_of_the_wrong_kind_is_rejected_with_its_key_path() {
    let loop_json = |loop_def: &str| format!(r#"{{"name":"t","loops":{{"env":{loop_def}}}}}"#);
    let cases = [
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":"2","closer_element":2,"count_element":1,"count":"segments"}}"#,
            ),
            "spec: the value at loops.env.control.opener_element must be a non-negative integer; found a string (\"2\")",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":-2,"count_element":1,"count":"segments"}}"#,
            ),
            "spec: the value at loops.env.control.closer_element must be a non-negative integer; found a negative number (-2)",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":2,"count_element":1,"count":7}}"#,
            ),
            "spec: the value at loops.env.control.count must be a string; found a number (7)",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD","where":{"1":"X","2":5}}}"#),
            "spec: the value at loops.env.trigger.where.2 must be a string; found a number (5)",
        ),
        (
            loop_json(r#"{"trigger":{"segment":7}}"#),
            "spec: the value at loops.env.trigger.segment must be a string; found a number (7)",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":null,"pos":1}}}"#,
            ),
            "spec: the value at loops.env.occurrences.a.segment must be a string; found null",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":"1"}}}"#,
            ),
            "spec: the value at loops.env.occurrences.a.pos must be a non-negative integer; found a string (\"1\")",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"max":-1}"#),
            "spec: the value at loops.env.max must be a non-negative integer; found a negative number (-1)",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":1,"codes":{"1":"X"}}}}"#,
            ),
            "spec: the value at loops.env.occurrences.a.codes.1 must be an array of strings; found a string (\"X\")",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":1,"qualifier":{"element":1,"codes":[2]}}}}"#,
            ),
            "spec: the value at loops.env.occurrences.a.qualifier.codes[0] must be a string; found a number (2)",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"end":true}"#),
            "spec: the value at loops.env.end must be a string; found a boolean (true)",
        ),
    ];
    for (json, expected) in cases {
        let err = Spec::from_json(&json).unwrap_err();
        assert!(
            matches!(&err, SpecError::WrongType { .. }),
            "{json}: {err:?}"
        );
        assert_eq!(err.to_string(), expected, "{json}");
    }
}
