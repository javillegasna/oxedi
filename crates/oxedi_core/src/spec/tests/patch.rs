//! Merge patches over a loaded spec.

use serde_json::{Value, from_str};

use super::{CLP_ONLY, TABLED};
use crate::spec::test_support::{builtin, loaded, patched};
use crate::spec::*;
use crate::value::LengthRange;

#[test]
fn a_patch_retouches_one_element_and_keeps_the_rest() {
    let spec = patched(
        &loaded(CLP_ONLY),
        r#"{"segments":{"CLP":{"elements":{"1":{"max":30}}}}}"#,
    )
    .unwrap();
    let clp = spec.segment(b"CLP").unwrap();
    assert_eq!(
        clp.elements[&1].length,
        LengthRange {
            min: Some(1),
            max: Some(30)
        }
    );
    assert_eq!(clp.elements[&1].name, "claim_submitter_id");
    assert_eq!(clp.elements.len(), 3);
}

#[test]
fn a_patch_adds_a_segment_definition() {
    let spec = patched(
        &loaded(CLP_ONLY),
        r#"{"segments":{"ZZ1":{"elements":{"1":{"name":"payer_note","type":"AN"}}}}}"#,
    )
    .unwrap();
    assert_eq!(
        spec.segment(b"ZZ1").unwrap().elements[&1].name,
        "payer_note"
    );
}

#[test]
fn a_patch_adds_a_column_with_three_lines() {
    let spec = patched(
        &loaded(TABLED),
        r#"{"tables":{"bodies":{"columns":{
                "amount":{"segment":"BB","element":2}
            }}}}"#,
    )
    .unwrap();
    let names: Vec<&str> = spec
        .table("bodies")
        .unwrap()
        .columns
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, vec!["amount", "id", "line_at"]);
}

#[test]
fn patch_merges_nested_objects_and_keeps_siblings() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"1000A":{"trigger":{"where":{"2":"ACME"}}}}}"#,
    )
    .unwrap();
    let trigger = &spec.get(spec.loop_id("1000A").unwrap()).trigger;
    assert_eq!(trigger.segment, b"N1");
    assert_eq!(
        trigger.conditions,
        vec![(1, b"PR".to_vec()), (2, b"ACME".to_vec())]
    );
    assert_eq!(spec.loops().len(), 8, "other loops untouched");
}

#[test]
fn patch_null_deletes() {
    let spec = patched(&builtin(Version::V5010),
            r#"{"loops":{"2110":null},"tables":{"services":null,"adjustments":{"loops":["2100"]}},"balancing":{"service_balance":null,"claim_balance":null}}"#,
        )
        .unwrap();
    assert_eq!(spec.loop_id("2110"), None);
    assert!(spec.children(spec.loop_id("2100")).is_empty());
    assert_eq!(spec.table("services"), None);
}

#[test]
fn deleting_a_loop_a_table_anchors_in_names_the_table() {
    let err = patched(&builtin(Version::V5010), r#"{"loops":{"2110":null}}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: table \"adjustments\": loop \"2110\" does not exist"
    );
}

#[test]
fn patches_chain_and_the_written_model_shows_them() {
    let loaded = builtin(Version::V5010)
        .merge_patch(r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#)
        .unwrap()
        .merge_patch(r#"{"loops":{"b":{"trigger":{"segment":"BB"}}}}"#)
        .unwrap();
    assert_eq!(loaded.spec.loops().len(), 10);
    let json = loaded.to_json().unwrap();
    assert!(json.contains("\"AA\"") && json.contains("\"BB\""));
}

#[test]
fn an_unparsable_patch_is_reported_as_a_patch_error() {
    let err = patched(&builtin(Version::V5010), "not json").unwrap_err();
    assert!(
        matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::Json(_))),
        "{err:?}"
    );
    assert!(
        err.to_string()
            .starts_with("applying patch: invalid spec JSON: "),
        "{err}"
    );
}

#[test]
fn merge_patch_follows_rfc_7386() {
    use serde_json::json;
    let cases = [
        (json!({"a": "b"}), json!({"a": "c"}), json!({"a": "c"})),
        (
            json!({"a": "b"}),
            json!({"b": "c"}),
            json!({"a": "b", "b": "c"}),
        ),
        (json!({"a": "b"}), json!({"a": null}), json!({})),
        (
            json!({"a": "b", "b": "c"}),
            json!({"a": null}),
            json!({"b": "c"}),
        ),
        (json!({"a": ["b"]}), json!({"a": "c"}), json!({"a": "c"})),
        (json!({"a": "c"}), json!({"a": ["b"]}), json!({"a": ["b"]})),
        (
            json!({"a": {"b": "c"}}),
            json!({"a": {"b": "d", "c": null}}),
            json!({"a": {"b": "d"}}),
        ),
        (
            json!({"a": [{"b": "c"}]}),
            json!({"a": [1]}),
            json!({"a": [1]}),
        ),
        (json!(["a", "b"]), json!(["c", "d"]), json!(["c", "d"])),
        (json!({"a": "b"}), json!(["c"]), json!(["c"])),
        (json!({"a": "foo"}), json!(null), json!(null)),
        (json!({"a": "foo"}), json!("bar"), json!("bar")),
        (
            json!({"e": null}),
            json!({"a": 1}),
            json!({"e": null, "a": 1}),
        ),
        (
            json!([1, 2]),
            json!({"a": "b", "c": null}),
            json!({"a": "b"}),
        ),
        (
            json!({}),
            json!({"a": {"bb": {"ccc": null}}}),
            json!({"a": {"bb": {}}}),
        ),
    ];
    for (target, patch, expected) in cases {
        let mut result = target.clone();
        merge_patch(&mut result, &patch);
        assert_eq!(result, expected, "target {target} patch {patch}");
    }
}

#[test]
fn a_patch_over_the_model_reads_as_the_patch_over_the_json_as_written() {
    for patch in [
        Spec::BUILTIN_835_4010_PATCH,
        include_str!("../../../specs/edi_835_parser.json"),
    ] {
        let over_model = LoadedSpec::from_json(Spec::BUILTIN_835_JSON)
            .unwrap()
            .merge_patch(patch)
            .unwrap();
        let mut written: Value = from_str(Spec::BUILTIN_835_JSON).unwrap();
        merge_patch(&mut written, &from_str(patch).unwrap());
        assert_eq!(
            over_model.model,
            SpecModel::from_json(&written.to_string()).unwrap()
        );
    }
}

#[test]
fn a_model_patch_wraps_a_compile_failure() {
    let loaded = LoadedSpec::from_json(Spec::BUILTIN_835_JSON).unwrap();
    let err = loaded
        .merge_patch(r#"{"loops":{"2000":null}}"#)
        .unwrap_err();
    assert!(
        matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::UnknownParent { .. }))
    );
    assert_eq!(
        err.to_string(),
        "applying patch: loop \"2100\" names unknown parent \"2000\""
    );
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn a_model_patch_leaves_the_original() {
    let original = LoadedSpec::from_json(Spec::BUILTIN_835_JSON).unwrap();
    let patched = original
        .merge_patch(r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#)
        .unwrap();
    assert_eq!(patched.spec.loops().len(), original.spec.loops().len() + 1);
    assert!(!original.model.loops.contains_key("a"));
    assert_eq!(
        SpecModel::from_json(&patched.to_json().unwrap()).unwrap(),
        patched.model
    );
}
