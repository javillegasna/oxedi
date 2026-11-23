use serde_json::Value;

use super::super::stack::Stack;
use super::super::*;
use super::{envelope, package};
use crate::spec_load::SpecError;

const SET: &str = "test/transaction-set/005010/835";
const GUIDE: &str = "test/guide/005010/835/X1";

/// The id of a package of `layer` in these tests.
fn id_of(layer: &str) -> &'static str {
    match layer {
        "dictionary" => "test/dictionary/005010",
        "transaction-set" => SET,
        "guide" => GUIDE,
        "views" => "test/views/005010/835/X1",
        _ => "test/patch/005010/835/X1",
    }
}

/// `value` written at the dotted `path`, as a spec object.
fn nested(path: &str, value: &str) -> String {
    let value: Value = serde_json::from_str(value).unwrap();
    let spec = path
        .rsplit('.')
        .fold(value, |inner, key| serde_json::json!({ key: inner }));
    spec.to_string()
}

fn compose(layers: &[&str], spec: &str) -> Result<(), SpecError> {
    let top = package(
        id_of(layers[layers.len() - 1]),
        layers,
        &["test/envelope/005010"],
        spec,
    );
    let pool = [envelope(), top.clone()];
    Stack::resolve(&top, &pool)?.compose_checked().map(|_| ())
}

// Whether the layers own every key the spec writes; the changes a package
// makes may still be refused by the narrowing check.
fn owned(layers: &[&str], spec: &str) -> bool {
    !matches!(
        compose(layers, spec),
        Err(SpecError::Stack { reason }) if matches!(*reason, StackError::KeyNotOwned { .. })
    )
}

fn refusal(layers: &[&str], spec: &str) -> StackError {
    match compose(layers, spec) {
        Err(SpecError::Stack { reason }) => *reason,
        other => panic!("expected a stack refusal, found {other:?}"),
    }
}

#[test]
fn each_key_belongs_to_its_layers() {
    let rows = [
        (
            "version",
            r#"{"segment":"AA","element":1,"values":["X"]}"#,
            "guide",
            "dictionary",
        ),
        ("balancing", "{}", "guide", "views"),
        (
            "loops.a.trigger",
            r#"{"segment":"AA"}"#,
            "transaction-set",
            "guide",
        ),
        ("loops.a.max", "3", "guide", "transaction-set"),
        (
            "loops.a.occurrences.aa.pos",
            "1",
            "transaction-set",
            "guide",
        ),
        (
            "loops.a.occurrences.aa.qualifier",
            r#"{"element":1,"codes":["X"]}"#,
            "guide",
            "transaction-set",
        ),
        ("segments", "{}", "dictionary", "guide"),
        ("tables", "{}", "views", "guide"),
    ];
    for (path, value, owner, other) in rows {
        let spec = nested(path, value);
        assert!(owned(&[owner], &spec), "{path} by {owner} was refused");
        let StackError::KeyNotOwned { path: refused, .. } = refusal(&[other], &spec) else {
            panic!("{path} by {other} was not refused as a key not owned");
        };
        assert_eq!(refused, path);
    }
}

#[test]
fn key_not_owned_display() {
    let err = compose(&["dictionary"], r#"{"loops":{"a":{"usage":"required"}}}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/dictionary/005010\" holds loops.a.usage, a key of layer \"envelope\" or \"guide\", but its layers are \"dictionary\"; move the key to a package of that layer, or list the layer in \"layers\""
    );
}

#[test]
fn the_name_belongs_to_no_layer() {
    let err = compose(&["patch"], r#"{"name":"x"}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/patch/005010/835/X1\" holds \"name\"; a stack takes its name from its top package's id, so remove the key"
    );
}

#[test]
fn a_deletion_needs_every_layer_beneath() {
    let spec = r#"{"loops":{"a":{"occurrences":{"aa":null}}}}"#;
    let StackError::KeyNotOwned { path, owners, .. } = refusal(&["guide"], spec) else {
        panic!("a guide deleted an occurrence");
    };
    assert_eq!(path, "loops.a.occurrences.aa");
    assert_eq!(owners, [Layer::Envelope, Layer::TransactionSet]);
    assert!(owned(&["transaction-set", "guide"], spec));
}

#[test]
fn a_patch_owns_every_key_but_the_name() {
    let spec = r#"{"segments":{},"tables":{},"loops":{"a":{"trigger":{"segment":"AA"}}}}"#;
    assert!(compose(&["patch"], spec).is_ok());
}

#[test]
fn a_name_holding_a_separator_is_quoted_in_the_path() {
    let spec = r#"{"loops":{"a.b":{"occurrences":{"":{"usage":"required"}}}}}"#;
    let StackError::KeyNotOwned { path, .. } = refusal(&["dictionary"], spec) else {
        panic!("a dictionary wrote a guide key");
    };
    assert_eq!(path, r#"loops."a.b".occurrences."".usage"#);
}

#[test]
fn an_unknown_key_is_left_to_the_strict_read() {
    let err = compose(&["dictionary"], r#"{"glossary":{}}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/dictionary/005010\": spec: at glossary: unknown field `glossary`, expected one of `name`, `version`, `loops`, `tables`, `balancing`, `segments`, `summaries`, `default_table`"
    );
}

#[test]
fn a_section_that_is_not_an_object_is_left_to_the_strict_read() {
    let err = compose(&["guide"], r#"{"loops":5}"#).unwrap_err();
    assert!(matches!(&err, SpecError::Package { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "package \"test/guide/005010/835/X1\": spec: at loops: invalid type: integer `5`, expected a map"
    );
}

#[test]
fn ownership_errors_have_no_source() {
    let err = refusal(&["patch"], r#"{"name":"x"}"#);
    assert!(std::error::Error::source(&err).is_none());
}
