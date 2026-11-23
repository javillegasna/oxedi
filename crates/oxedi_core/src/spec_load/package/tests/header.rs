use serde_json::json;

use super::super::*;
use crate::spec_load::SpecError;

const HEADER: &str = r#"{"format":1,"id":"test/envelope/005010","layers":["envelope"],"bases":[]}"#;

fn package_text(header: &str, spec: &str) -> String {
    format!(r#"{{"package":{header},"spec":{spec}}}"#)
}

fn read_err(text: &str) -> SpecError {
    Package::from_json(text).unwrap_err()
}

#[test]
fn a_package_reads_its_header_and_keeps_its_spec() {
    let package = Package::from_json(&package_text(HEADER, r#"{"loops":{}}"#)).unwrap();
    let header = package.header();
    assert_eq!(header.format.get(), 1);
    assert_eq!(package.id().as_str(), "test/envelope/005010");
    assert_eq!(header.layers.top(), Layer::Envelope);
    assert!(header.bases.is_empty());
    assert_eq!(package.body(), &json!({"loops": {}}));
}

#[test]
fn every_format_up_to_the_readers_reads() {
    for n in 1..=PackageHeader::FORMAT {
        let header = HEADER.replace("\"format\":1", &format!("\"format\":{n}"));
        assert!(Package::from_json(&package_text(&header, "{}")).is_ok());
    }
}

#[test]
fn a_newer_format_is_refused_naming_both_before_its_keys() {
    let header = r#"{"format":2,"id":"test/envelope/005010","layers":["envelope"],"bases":[],"hierarchy":{}}"#;
    let err = read_err(&package_text(header, "{}"));
    assert!(matches!(
        err,
        SpecError::BadHeader {
            reason: HeaderError::FormatTooNew {
                found: 2,
                reader: 1
            }
        }
    ));
    assert_eq!(
        err.to_string(),
        "the package header is invalid: package format 2 is newer than 1, the newest format this reader takes; read the package with a newer oxedi, or write it in format 1"
    );
}

#[test]
fn header_faults_name_their_path() {
    let with = |key: &str, value: &str| {
        let mut header: serde_json::Value = serde_json::from_str(HEADER).unwrap();
        header[key] = serde_json::from_str(value).unwrap();
        package_text(&header.to_string(), "{}")
    };
    let mut no_bases: serde_json::Value = serde_json::from_str(HEADER).unwrap();
    no_bases.as_object_mut().unwrap().remove("bases");
    let cases = [
        (
            format!(r#"{{"package":{HEADER},"spec":{{}},"extra":1}}"#),
            "spec: at extra: unknown field `extra`, expected `package` or `spec`",
        ),
        (
            package_text(&no_bases.to_string(), "{}"),
            "spec: at package: missing field `bases`",
        ),
        (
            with("format", "0"),
            "spec: at package.format: invalid value: integer `0`, expected a nonzero u32",
        ),
        (
            with("id", r#""oxedi/glossary/005010""#),
            "spec: at package.id: package id \"oxedi/glossary/005010\": layer \"glossary\" must be \"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\" or \"patch\"",
        ),
        (
            with("layers", "[]"),
            "spec: at package.layers: \"layers\" lists no layer; list the layers the package holds, in layer order",
        ),
        (
            with("layers", r#"["guide","dictionary"]"#),
            "spec: at package.layers: \"layers\" must list each layer once, in layer order (\"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\", \"patch\"); found \"guide\", \"dictionary\"",
        ),
        (
            with("layers", r#"["glossary"]"#),
            "spec: at package.layers[0]: \"layer\" must be \"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\" or \"patch\"; found \"glossary\"",
        ),
        (
            package_text(HEADER, "null"),
            "spec: at spec: invalid type: null, expected a map",
        ),
        (
            package_text(HEADER, "5"),
            "spec: at spec: invalid type: integer `5`, expected a map",
        ),
        (
            package_text(HEADER, "[]"),
            "spec: at spec: invalid type: sequence, expected a map",
        ),
        (
            format!(r#"{{"package":{HEADER}}}"#),
            "spec: at .: missing field `spec`",
        ),
    ];
    for (text, expected) in cases {
        let err = read_err(&text);
        assert!(matches!(err, SpecError::Format { .. }), "{text}");
        assert_eq!(err.to_string(), expected, "{text}");
    }
}

#[test]
fn an_id_naming_another_layer_than_the_top_is_refused() {
    let header = r#"{"format":1,"id":"test/dictionary/005010","layers":["envelope","dictionary","transaction-set"],"bases":[]}"#;
    assert_eq!(
        read_err(&package_text(header, "{}")).to_string(),
        "the package header is invalid: package \"test/dictionary/005010\" names layer \"dictionary\" in its id, but the top layer \"layers\" lists is \"transaction-set\"; the id names the package's top layer"
    );
}

#[test]
fn header_error_has_no_source() {
    let err = HeaderError::FormatTooNew {
        found: 2,
        reader: 1,
    };
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn bad_header_does_not_chain() {
    let err = read_err(&package_text(
        &HEADER.replace("\"format\":1", "\"format\":9"),
        "{}",
    ));
    assert!(std::error::Error::source(&err).is_none());
}
