//! Loop control declarations.

use crate::spec::*;
use crate::spec_load::*;

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
    ];
    for (control, expected) in cases {
        let err = control_error(control);
        assert!(
            matches!(&err, SpecError::BadControl { loop_name, reason } if loop_name == "env" && *reason == expected),
            "{control}: {err:?}"
        );
    }
    assert_eq!(
        control_error(
            r#"{"opener_element":2,"closer_element":2,"count_element":1,"count":"segs"}"#
        )
        .to_string(),
        "spec: at loops.env.control.count: \"count\" must be \"segments\" or \"children\"; found \"segs\""
    );
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
    let cases = [
        (
            "[2,2,1]",
            "spec: at loops.env.control: invalid type: sequence, expected a control object",
        ),
        (
            r#"{"opener_element":2,"closer_element":2,"count":"segments"}"#,
            "spec: at loops.env.control: missing field `count_element`",
        ),
    ];
    for (control, expected) in cases {
        let err = control_error(control);
        assert!(matches!(&err, SpecError::Format { .. }), "{err:?}");
        assert_eq!(err.to_string(), expected, "{control}");
    }
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
fn a_value_of_the_wrong_kind_is_refused_at_its_path() {
    let loop_json = |loop_def: &str| format!(r#"{{"name":"t","loops":{{"env":{loop_def}}}}}"#);
    let cases = [
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":"2","closer_element":2,"count_element":1,"count":"segments"}}"#,
            ),
            "spec: at loops.env.control.opener_element: invalid type: string \"2\", expected usize",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":-2,"count_element":1,"count":"segments"}}"#,
            ),
            "spec: at loops.env.control.closer_element: invalid value: integer `-2`, expected usize",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":2,"count_element":1,"count":7}}"#,
            ),
            "spec: at loops.env.control.count: invalid type: integer `7`, expected a string",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD","where":{"1":"X","2":5}}}"#),
            "spec: at loops.env.trigger.where.2: invalid type: integer `5`, expected a string",
        ),
        (
            loop_json(r#"{"trigger":{"segment":7}}"#),
            "spec: at loops.env.trigger.segment: invalid type: integer `7`, expected a string",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":null,"pos":1}}}"#,
            ),
            "spec: at loops.env.occurrences.a.segment: invalid type: null, expected a string",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":"1"}}}"#,
            ),
            "spec: at loops.env.occurrences.a.pos: invalid type: string \"1\", expected usize",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"max":-1}"#),
            "spec: at loops.env.max: invalid value: integer `-1`, expected usize",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":1,"codes":{"1":"X"}}}}"#,
            ),
            "spec: at loops.env.occurrences.a.codes.1: invalid type: string \"X\", expected a sequence",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":1,"qualifier":{"element":1,"codes":[2]}}}}"#,
            ),
            "spec: at loops.env.occurrences.a.qualifier.codes[0]: invalid type: integer `2`, expected a string",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"end":true}"#),
            "spec: at loops.env.end: invalid type: boolean `true`, expected a string",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"end":"TR","control":null}"#),
            "spec: at loops.env.control: invalid type: null, expected a control object",
        ),
        (
            loop_json(
                r#"{"trigger":{"segment":"HD"},"occurrences":{"a":{"segment":"A","pos":1,"qualifier":null}}}"#,
            ),
            "spec: at loops.env.occurrences.a.qualifier: invalid type: null, expected a qualifier object",
        ),
        (
            loop_json(r#"{"trigger":{"segment":"HD"},"occurrences":null}"#),
            "spec: at loops.env.occurrences: invalid type: null, expected a map",
        ),
    ];
    for (json, expected) in cases {
        let err = Spec::from_json(&json).unwrap_err();
        assert!(matches!(&err, SpecError::Format { .. }), "{json}: {err:?}");
        assert_eq!(err.to_string(), expected, "{json}");
    }
}
