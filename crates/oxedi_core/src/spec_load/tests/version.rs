//! The version a spec declares: its load, its faults and the built-ins' declarations.

use super::assert_model_faults;
use super::{package_of, shipped};
use crate::spec::test_support::{declared, declaring, loaded, patched};
use crate::spec::*;
use crate::spec_load::package::Stack;
use crate::spec_load::*;

fn version_error(version: &str) -> SpecError {
    Spec::from_json(&format!(
        r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"GS"}}}}}},"version":{version}}}"#
    ))
    .unwrap_err()
}

#[test]
fn a_declared_version_loads_and_survives_a_patch() {
    let base = loaded(&declared(
        "t",
        r#"{"segment":"GS","element":8,"values":["005010X221A1"]}"#,
    ));
    let version = base.spec.version().unwrap();
    assert_eq!(version.segment, b"GS");
    assert_eq!(version.element, 8);
    assert_eq!(version.values, vec![b"005010X221A1".to_vec()]);
    let spec = patched(&base, r#"{"version":{"values":["004010X091A1","004010"]}}"#).unwrap();
    let version = spec.version().unwrap();
    assert_eq!(version.segment, b"GS");
    assert_eq!(
        version.values,
        vec![b"004010X091A1".to_vec(), b"004010".to_vec()]
    );
    assert!(declaring("t", "").version().is_none());
    assert!(
        patched(&base, r#"{"version":null}"#)
            .unwrap()
            .version()
            .is_none()
    );
}

fn gs08_with_codes(version: &str) -> String {
    format!(
        r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"GS"}}}}}},"segments":{{"GS":{{"elements":{{"8":{{"name":"v","type":"AN","codes":["B","A","C"]}}}}}}}},"version":{version}}}"#
    )
}

#[test]
fn left_out_values_are_the_elements_codes_in_written_order() {
    let codes = vec![b"B".to_vec(), b"A".to_vec(), b"C".to_vec()];
    let taken = loaded(&gs08_with_codes(r#"{"segment":"GS","element":8}"#));
    assert_eq!(taken.spec.version().unwrap().values, codes);
    let written = loaded(&gs08_with_codes(
        r#"{"segment":"GS","element":8,"values":["X"]}"#,
    ));
    assert_eq!(written.spec.version().unwrap().values, vec![b"X".to_vec()]);
    let spec = patched(&written, r#"{"version":{"values":null}}"#).unwrap();
    assert_eq!(spec.version().unwrap().values, codes);
    let kept = patched(&written, r#"{"version":{"element":8}}"#).unwrap();
    assert_eq!(kept.version().unwrap().values, vec![b"X".to_vec()]);
}

#[test]
fn a_declared_element_without_codes_names_the_missing_codes() {
    let err = Spec::from_json(
        r#"{"name":"t","loops":{"a":{"trigger":{"segment":"GS"}}},"segments":{"GS":{"elements":{"8":{"name":"v","type":"AN"}}}},"version":{"segment":"GS","element":8}}"#,
    )
    .unwrap_err();
    assert!(
        matches!(
            &err,
            SpecError::BadVersion {
                reason: VersionError::NoCodes { .. }
            }
        ),
        "{err:?}"
    );
}

#[test]
fn every_version_fault_is_named() {
    let err = version_error(r#"{"segment":"","element":8,"values":["X"]}"#);
    assert!(
        matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == "version.segment"),
        "{err:?}"
    );
    assert_eq!(
        err.to_string(),
        "the spec has an empty segment id at version.segment"
    );
    let cases = [
        (
            r#"{"segment":"GS","element":0,"values":["X"]}"#,
            VersionError::ZeroElement,
            "the spec's \"version\" is invalid: \"element\" is 0; positions are 1-based",
        ),
        (
            r#"{"segment":"GS","element":8,"values":[]}"#,
            VersionError::NoValues,
            "the spec's \"version\" is invalid: \"values\" is empty; list at least one value the element may hold",
        ),
        (
            r#"{"segment":"GS","element":8,"values":["X",""]}"#,
            VersionError::EmptyValue { index: 1 },
            "the spec's \"version\" is invalid: \"values[1]\" is empty",
        ),
        (
            r#"{"segment":"GS","element":8,"values":["004010","005010","004010"]}"#,
            VersionError::DuplicateValue {
                value: "004010".into(),
                first: 0,
                second: 2,
            },
            "the spec's \"version\" is invalid: value \"004010\" is listed twice, at values[0] and values[2]",
        ),
        (
            r#"{"segment":"GS","element":8}"#,
            VersionError::NoCodes {
                segment: "GS".into(),
                element: 8,
            },
            "the spec's \"version\" is invalid: \"values\" is absent and element 8 of segment \"GS\" is not defined or lists no codes to take them from; write \"values\", or give segments.GS.elements.8 its \"codes\"",
        ),
    ];
    for (version, expected_reason, expected_text) in cases {
        let err = version_error(version);
        assert!(
            matches!(&err, SpecError::BadVersion { reason } if *reason == expected_reason),
            "{version}: {err:?}"
        );
        assert_eq!(err.to_string(), expected_text);
        assert!(std::error::Error::source(&err).is_none());
    }
}

#[test]
fn a_version_the_model_refuses_fails_at_its_path() {
    let cases = [
        (
            r#""GS08""#,
            "spec: at version: invalid type: string \"GS08\", expected a version object",
        ),
        (
            r#"{"segment":"GS","element":8,"values":null}"#,
            "spec: at version.values: invalid type: null, expected a sequence",
        ),
        (
            r#"{"segment":"GS","element":8,"values":["X"],"value":"X"}"#,
            "spec: at version.value: unknown field `value`, expected one of `segment`, `element`, `values`",
        ),
        (
            r#"{"segment":"GS","element":"8","values":["X"]}"#,
            "spec: at version.element: invalid type: string \"8\", expected usize",
        ),
        (
            r#"{"segment":"GS","element":8,"values":[8]}"#,
            "spec: at version.values[0]: invalid type: integer `8`, expected a string",
        ),
        (
            "null",
            "spec: at version: invalid type: null, expected a version object",
        ),
    ];
    assert_model_faults(&cases, version_error);
}

#[test]
fn the_builtins_declare_5010_and_4010_on_gs08() {
    let five = Spec::builtin_835();
    let four = Spec::builtin_835_4010();
    let declared = |spec: &Spec| {
        let version = spec.version().unwrap();
        (
            String::from_utf8_lossy(&version.segment).into_owned(),
            version.element,
            version
                .values
                .iter()
                .map(|value| String::from_utf8_lossy(value).into_owned())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        declared(&five),
        ("GS".into(), 8, vec!["005010X221A1".to_string()])
    );
    assert_eq!(
        declared(&four),
        (
            "GS".into(),
            8,
            vec![
                "004010X091A1".to_string(),
                "004010X091".to_string(),
                "004010".to_string()
            ]
        )
    );
}

#[test]
fn the_4010_builtin_is_the_5010_with_its_editions_merged() {
    let pool = shipped();
    let (five, four) = (
        Builtin::fallback(),
        Builtin::find("835", Version::V4010).unwrap(),
    );
    let stack =
        |builtin: &Builtin| Stack::resolve(package_of(&pool, builtin.id.as_str()), &pool).unwrap();
    let five_ids: Vec<&PackageId> = stack(five).steps().map(|step| step.package.id()).collect();
    let mut value = serde_json::to_value(five.model().unwrap()).unwrap();
    for step in stack(four)
        .steps()
        .filter(|step| !five_ids.contains(&step.package.id()))
    {
        merge_patch(&mut value, step.package.body());
    }
    assert_eq!(
        SpecModel::from_value(&value).unwrap().to_json().unwrap(),
        four.model().unwrap().to_json().unwrap()
    );
}
