//! Merge patches over a loaded spec.

use super::{CLP_ONLY, TABLED};
use crate::spec::*;

#[test]
fn a_patch_retouches_one_element_and_keeps_the_rest() {
    let spec = Spec::from_json(CLP_ONLY)
        .unwrap()
        .merge_patch(r#"{"segments":{"CLP":{"elements":{"1":{"max":30}}}}}"#)
        .unwrap();
    let clp = spec.segment(b"CLP").unwrap();
    assert_eq!(clp.elements[&1].max, Some(30));
    assert_eq!(clp.elements[&1].name, "claim_submitter_id");
    assert_eq!(clp.elements.len(), 3);
}

#[test]
fn a_patch_adds_a_segment_definition() {
    let spec = Spec::from_json(CLP_ONLY)
        .unwrap()
        .merge_patch(r#"{"segments":{"ZZ1":{"elements":{"1":{"name":"payer_note","type":"AN"}}}}}"#)
        .unwrap();
    assert_eq!(
        spec.segment(b"ZZ1").unwrap().elements[&1].name,
        "payer_note"
    );
}

#[test]
fn a_patch_adds_a_column_with_three_lines() {
    let spec = Spec::from_json(TABLED).unwrap();
    let patched = spec
        .merge_patch(
            r#"{"tables":{"bodies":{"columns":{
                    "amount":{"segment":"BB","element":2}
                }}}}"#,
        )
        .unwrap();
    let names: Vec<&str> = patched
        .table("bodies")
        .unwrap()
        .columns
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, vec!["amount", "id", "line_at"]);
}

#[test]
fn patch_replaces_arrays_wholesale() {
    let spec = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"1000A":{"segments":["N3","N4"]}}}"#)
        .unwrap();
    assert_eq!(
        spec.get(spec.loop_id("1000A").unwrap()).segments,
        vec![b"N3".to_vec(), b"N4".to_vec()]
    );
}

#[test]
fn patch_merges_nested_objects_and_keeps_siblings() {
    let spec = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"1000A":{"trigger":{"where":{"2":"ACME"}}}}}"#)
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
    let spec = Spec::builtin_835()
        .merge_patch(
            r#"{"loops":{"2110":null},"tables":{"services":null,"adjustments":{"loops":["2100"]}}}"#,
        )
        .unwrap();
    assert_eq!(spec.loop_id("2110"), None);
    assert!(spec.children(spec.loop_id("2100")).is_empty());
    assert_eq!(spec.table("services"), None);
}

#[test]
fn deleting_a_loop_a_table_anchors_in_names_the_table() {
    let err = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"2110":null}}"#)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: table \"adjustments\": loop \"2110\" does not exist"
    );
}

#[test]
fn patches_chain_and_to_json_shows_them() {
    let spec = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#)
        .unwrap()
        .merge_patch(r#"{"loops":{"b":{"trigger":{"segment":"BB"}}}}"#)
        .unwrap();
    assert_eq!(spec.loops().len(), 10);
    assert!(spec.to_json().contains("\"AA\"") && spec.to_json().contains("\"BB\""));
}

#[test]
fn invalid_patch_result_is_rejected_like_any_spec() {
    let err = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"2100":{"parent":"nope"}}}"#)
        .unwrap_err();
    assert!(
        matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::UnknownParent { .. })),
        "{err:?}"
    );
}

#[test]
fn an_unparsable_patch_is_reported_as_a_patch_error() {
    let err = Spec::builtin_835().merge_patch("not json").unwrap_err();
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
fn a_patch_that_breaks_a_reference_says_it_came_from_the_patch() {
    let err = Spec::builtin_835()
        .merge_patch(r#"{"loops":{"2000":null}}"#)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: loop \"2100\" names unknown parent \"2000\""
    );
    let source = std::error::Error::source(&err).map(ToString::to_string);
    assert_eq!(
        source.as_deref(),
        Some("loop \"2100\" names unknown parent \"2000\"")
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
