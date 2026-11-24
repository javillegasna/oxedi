use serde_json::Value;

use super::super::stack::Stack;
use super::super::*;
use super::{envelope, package};
use crate::spec_load::{LoadedSpec, SpecError};

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
    composed(layers, spec).map(|_| ())
}

fn composed(layers: &[&str], spec: &str) -> Result<Composed, SpecError> {
    let top = package(
        id_of(layers[layers.len() - 1]),
        layers,
        &["test/envelope/005010"],
        spec,
    );
    let pool = [envelope(), top.clone()];
    Stack::resolve(&top, &pool)?.compose_checked()
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
        (
            "segments.AA.elements.1.name",
            r#""code""#,
            "dictionary",
            "guide",
        ),
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
fn a_guide_and_the_dictionary_both_own_what_narrows_an_element() {
    for path in [
        "segments.AA.elements.1.codes",
        "segments.AA.elements.1.min",
        "segments.AA.elements.1.max",
        "segments.AA.elements.1.usage",
        "segments.AA.elements.2.composite.1.codes",
    ] {
        let value = if path.ends_with("codes") {
            r#"["X"]"#
        } else if path.ends_with("usage") {
            r#""required""#
        } else {
            "1"
        };
        let spec = nested(path, value);
        for layer in ["guide", "dictionary"] {
            assert!(owned(&[layer], &spec), "{path} by {layer} was refused");
        }
        assert!(!owned(&["views"], &spec), "{path} by views was accepted");
    }
}

#[test]
fn a_guide_owns_neither_the_shape_of_an_element_nor_its_deletion() {
    for (path, value) in [
        ("segments.AA.elements.2.composite.1.name", r#""x""#),
        ("segments.AA.elements.1.type", r#""AN""#),
        ("segments.AA.elements.1.scale", "2"),
        ("segments.AA.elements.1", "null"),
        ("segments.AA", "null"),
        ("segments.ZZ", "{}"),
    ] {
        let StackError::KeyNotOwned { path: refused, .. } =
            refusal(&["guide"], &nested(path, value))
        else {
            panic!("{path} by guide was not refused as a key not owned");
        };
        assert!(refused.starts_with(path), "{refused} for {path}");
        assert!(owned(&["dictionary"], &nested(path, value)));
    }
}

fn guide_element(change: &str) -> String {
    format!(r#"{{"segments":{{"AA":{{"elements":{{"1":{change}}}}}}}}}"#)
}

#[test]
fn a_guide_narrows_a_segment_element_without_naming_an_occurrence() {
    let composed = composed(&["guide"], &guide_element(r#"{"codes":["X"]}"#)).unwrap();
    assert!(composed.changes.is_empty());
    let spec = LoadedSpec::from_model(composed.model).unwrap().spec;
    assert_eq!(spec.element_def(b"AA", 1, None).unwrap().codes, ["X"]);
    let occurrence = &spec.loops()[0].occurrences[0];
    assert_eq!(occurrence.name, "aa");
    assert!(occurrence.elements.is_empty());
}

#[test]
fn a_guide_cannot_widen_a_segment_element() {
    let err = composed(&["guide"], &guide_element(r#"{"codes":["X","Y","Z"]}"#)).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "package \"{GUIDE}\" does not only narrow what its bases allow: segments.AA.elements.1.codes: [\"X\",\"Y\"] becomes [\"X\",\"Y\",\"Z\"]; a package only narrows its bases, unless it is an edition (it holds a layer its base holds) or a patch"
        )
    );
}

#[test]
fn a_guide_cannot_rename_a_segment_element() {
    let err = composed(&["guide"], &guide_element(r#"{"name":"other"}"#)).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "package \"{GUIDE}\" holds segments.AA.elements.1.name, a key of layer \"envelope\" or \"dictionary\", but its layers are \"guide\"; move the key to a package of that layer, or list the layer in \"layers\""
        )
    );
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
fn the_name_belongs_to_no_layer_but_a_patch() {
    let err = compose(&["views"], r#"{"name":"x"}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/views/005010/835/X1\" holds \"name\"; a stack takes its name from its top package's id and only a patch may change it, so remove the key"
    );
}

#[test]
fn a_patch_may_rename_its_stack() {
    let composed = composed(&["patch"], r#"{"name":"x"}"#).unwrap();
    assert_eq!(composed.model.name, "x");
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
fn a_patch_owns_every_key() {
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
    let err = refusal(&["views"], r#"{"name":"x"}"#);
    assert!(std::error::Error::source(&err).is_none());
}
