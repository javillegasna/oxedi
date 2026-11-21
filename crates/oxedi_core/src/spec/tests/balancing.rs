//! Balancing rules: the built-in ones, their validation at load and the
//! text of every error.

use super::assert_model_faults;
use crate::spec::test_support::{builtin, patched};
use crate::spec::*;

const LOOPS: &str = r#""loops":{
        "pay":{"trigger":{"segment":"PY"},"occurrences":{
            "py":{"segment":"PY","pos":1},"aj":{"segment":"AJ","pos":9}}},
        "claim":{"parent":"pay","trigger":{"segment":"CL"},"occurrences":{
            "cl":{"segment":"CL","pos":2},"ca":{"segment":"CA","pos":3}}},
        "other":{"trigger":{"segment":"OT"},"occurrences":{"ot":{"segment":"OT","pos":1}}}
    },
    "segments":{
        "PY":{"elements":{"1":{"name":"kind","type":"ID"},"2":{"name":"total","type":"R"}}},
        "CL":{"elements":{"3":{"name":"charge","type":"R","scale":4},"4":{"name":"paid","type":"R"}}},
        "CA":{"elements":{"2":{"name":"amount","type":"R"},
            "5":{"name":"pair","type":"AN","composite":{"1":{"name":"code","type":"ID"},"2":{"name":"value","type":"R"}}}}},
        "OT":{"elements":{"1":{"name":"amount","type":"R"}}},
        "AJ":{"elements":{"2":{"name":"amount","type":"R"}}}
    }"#;

fn load(balancing: &str) -> Result<Spec, SpecError> {
    Spec::from_json(&format!(
        r#"{{"name":"t",{LOOPS},"balancing":{balancing}}}"#
    ))
}

fn rule_error(rule: &str) -> String {
    load(&format!(r#"{{"r":{rule}}}"#)).unwrap_err().to_string()
}

const VALID: &str = r#"{"per":"pay","target":[{"occurrence":"py","elements":[2]}],
    "sum":[{"loop":"claim","occurrence":"cl","elements":[4]},{"occurrence":"aj","elements":[2],"sign":"-"}]}"#;

#[test]
fn both_built_in_specs_declare_the_three_rules() {
    for spec in [Spec::builtin_835(), Spec::builtin_835_4010()] {
        let names: Vec<&str> = spec.balancing().iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["claim_balance", "service_balance", "transaction_balance"]
        );
        for rule in spec.balancing() {
            assert_eq!(rule.scale, 2, "{}", rule.name);
        }
    }
}

#[test]
fn a_valid_rule_compiles_with_its_loops_occurrences_and_signs() {
    let spec = load(&format!(r#"{{"r":{VALID}}}"#)).unwrap();
    let [rule] = spec.balancing() else {
        panic!("one rule expected");
    };
    assert_eq!(rule.per, spec.loop_id("pay").unwrap());
    assert_eq!(rule.target[0].loop_id, spec.loop_id("pay").unwrap());
    assert_eq!(rule.sum[0].loop_id, spec.loop_id("claim").unwrap());
    assert_eq!(rule.sum[0].occurrence, 0);
    assert!(!rule.sum[0].negative);
    assert!(rule.sum[1].negative);
    assert_eq!(rule.scale, 2);
}

#[test]
fn a_component_amount_is_read_in_each_element() {
    let spec = load(
        r#"{"r":{"per":"claim","target":[{"occurrence":"cl","elements":[3]}],
            "sum":[{"occurrence":"ca","elements":[5],"component":2}]}}"#,
    )
    .unwrap();
    assert_eq!(spec.balancing()[0].sum[0].component, Some(2));
    assert_eq!(spec.balancing()[0].scale, 4);
}

#[test]
fn a_patch_removes_a_rule_by_name() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"balancing":{"service_balance":null}}"#,
    )
    .unwrap();
    assert_eq!(spec.balancing().len(), 2);
}

#[test]
fn deleting_a_loop_a_rule_reads_names_the_rule() {
    let err = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"2110":null},"tables":{"services":null,"adjustments":{"loops":["2100"]}}}"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: balancing rule \"claim_balance\": sum[1].loop names loop \"2110\", which the spec does not declare"
    );
}

#[test]
fn every_rejected_rule_names_the_rule_the_key_and_the_value() {
    let cases = [
        (
            r#"{"per":"nope","target":[],"sum":[]}"#,
            "balancing rule \"r\": per names loop \"nope\", which the spec does not declare",
        ),
        (
            r#"{"per":"pay","target":[],"sum":[]}"#,
            "balancing rule \"r\": target is empty; list at least one value",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"py","elements":[2]}],"sum":[]}"#,
            "balancing rule \"r\": sum is empty; list at least one value",
        ),
        (
            r#"{"per":"claim","target":[{"loop":"pay","occurrence":"py","elements":[2]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].loop reads loop \"pay\", which is neither the rule's \"per\" loop \"claim\" nor a loop below it",
        ),
        (
            r#"{"per":"pay","target":[{"loop":"other","occurrence":"ot","elements":[1]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].loop reads loop \"other\", which is neither the rule's \"per\" loop \"pay\" nor a loop below it",
        ),
        (
            r#"{"per":"pay","target":[{"loop":"zz","occurrence":"py","elements":[2]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].loop names loop \"zz\", which the spec does not declare",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"cl","elements":[4]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].occurrence names occurrence \"cl\", which loop \"pay\" does not declare",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"py","elements":[]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].elements is empty; list at least one element position",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"py","elements":[2,7]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].elements[1] names PY07, which the \"segments\" section does not define",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"py","elements":[1]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].elements[0] names PY01, of type ID; a balancing value must be of type R",
        ),
        (
            r#"{"per":"claim","target":[{"occurrence":"ca","elements":[5]}],"sum":[]}"#,
            "balancing rule \"r\": target[0].elements[0] names CA05, of type AN; a balancing value must be of type R",
        ),
        (
            r#"{"per":"pay","target":[{"occurrence":"py","elements":[2]}],"sum":[{"occurrence":"py","elements":[2],"sign":"-"}]}"#,
            "balancing rule \"r\": sum[0].elements[0] counts PY02 of the same occurrence that target[0].elements[0] already counts",
        ),
    ];
    for (rule, expected) in cases {
        assert_eq!(rule_error(rule), expected, "{rule}");
    }
}

#[test]
fn a_sign_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#"{"r":{"per":"pay","target":[{"occurrence":"py","elements":[2],"sign":"*"}],"sum":[]}}"#,
            "spec: at balancing.r.target[0].sign: \"sign\" must be \"+\" or \"-\"; found \"*\"",
        ),
        (
            r#"{"r":{"per":"nope","target":[],"sum":[{"occurrence":"py","elements":[2],"sign":"plus"}]}}"#,
            "spec: at balancing.r.sum[0].sign: \"sign\" must be \"+\" or \"-\"; found \"plus\"",
        ),
        (
            r#"{"":{"per":"pay","target":[],"sum":[{"occurrence":"py","elements":[2],"sign":"x"}]}}"#,
            "spec: at balancing.\"\".sum[0].sign: \"sign\" must be \"+\" or \"-\"; found \"x\"",
        ),
    ];
    for (rule, expected) in cases {
        let err = load(rule).unwrap_err();
        assert!(matches!(err, SpecError::Format { .. }), "{err}");
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn an_empty_rule_name_is_rejected() {
    assert_eq!(
        load(&format!(r#"{{"":{VALID}}}"#)).unwrap_err().to_string(),
        "balancing rule \"\": the rule name is empty"
    );
}

#[test]
fn a_balancing_rule_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#"{"r":{"per":"pay","target":[]}}"#,
            "spec: at balancing.r: missing field `sum`",
        ),
        (
            r#"{"r":{"per":"pay","target":[],"sum":[],"tolerance":0}}"#,
            "spec: at balancing.r.tolerance: unknown field `tolerance`, expected one of `per`, `target`, `sum`",
        ),
        (
            r#"{"r":{"per":"pay","target":{},"sum":[]}}"#,
            "spec: at balancing.r.target: invalid type: map, expected a sequence",
        ),
        (
            r#"{"r":{"per":"pay","target":[{"occurrence":"py"}],"sum":[]}}"#,
            "spec: at balancing.r.target[0]: missing field `elements`",
        ),
        (
            r#"{"r":{"per":"pay","target":[{"occurrence":"py","elements":2}],"sum":[]}}"#,
            "spec: at balancing.r.target[0].elements: invalid type: integer `2`, expected a sequence",
        ),
        (
            r#"{"r":{"per":"pay","target":[{"occurrence":"py","elements":["2"]}],"sum":[]}}"#,
            "spec: at balancing.r.target[0].elements[0]: invalid type: string \"2\", expected usize",
        ),
        (
            r#"{"r":{"per":"pay","target":[],"sum":[{"occurrence":"py","elements":[2],"sign":1}]}}"#,
            "spec: at balancing.r.sum[0].sign: invalid type: integer `1`, expected a string",
        ),
        (
            r#"{"r":{"per":"pay","target":null,"sum":[]}}"#,
            "spec: at balancing.r.target: invalid type: null, expected a sequence",
        ),
    ];
    assert_model_faults(&cases, |balancing| load(balancing).unwrap_err());
}

#[test]
fn balance_errors_display_their_full_text() {
    let cases = [
        (BalanceError::EmptyName, "the rule name is empty"),
        (
            BalanceError::UnknownLoop {
                key: "per".into(),
                name: "x".into(),
            },
            "per names loop \"x\", which the spec does not declare",
        ),
        (
            BalanceError::NoTerms { key: "sum" },
            "sum is empty; list at least one value",
        ),
        (
            BalanceError::LoopOutsidePer {
                key: "sum[0].loop".into(),
                loop_name: "a".into(),
                per: "b".into(),
            },
            "sum[0].loop reads loop \"a\", which is neither the rule's \"per\" loop \"b\" nor a loop below it",
        ),
        (
            BalanceError::UnknownOccurrence {
                key: "sum[0].occurrence".into(),
                loop_name: "a".into(),
                occurrence: "o".into(),
            },
            "sum[0].occurrence names occurrence \"o\", which loop \"a\" does not declare",
        ),
        (
            BalanceError::NoElements {
                key: "sum[0].elements".into(),
            },
            "sum[0].elements is empty; list at least one element position",
        ),
        (
            BalanceError::UndefinedElement {
                key: "sum[0].elements[0]".into(),
                place: "SVC01-2".into(),
            },
            "sum[0].elements[0] names SVC01-2, which the \"segments\" section does not define",
        ),
        (
            BalanceError::NotDecimal {
                key: "sum[0].elements[0]".into(),
                place: "CLP01".into(),
                found: "AN".into(),
            },
            "sum[0].elements[0] names CLP01, of type AN; a balancing value must be of type R",
        ),
        (
            BalanceError::RepeatedValue {
                key: "sum[1].elements[0]".into(),
                first: "sum[0].elements[0]".into(),
                place: "CAS03".into(),
            },
            "sum[1].elements[0] counts CAS03 of the same occurrence that sum[0].elements[0] already counts",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn a_bad_rule_displays_the_rule_and_does_not_chain_its_source() {
    let bad = SpecError::BadBalance {
        rule: "r".into(),
        reason: Box::new(BalanceError::EmptyName),
    };
    assert_eq!(
        bad.to_string(),
        "balancing rule \"r\": the rule name is empty"
    );
    assert!(std::error::Error::source(&bad).is_none());
}
