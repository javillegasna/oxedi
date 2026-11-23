use serde_json::{Value, json};

use super::super::change::ChangeKind;
use super::super::narrow::differences;
use super::super::*;
use crate::spec_load::merge_patch;
use crate::spec_load::model::SpecModel;

const D: &str = "test/dictionary/005010";

// A loop and a segment `AA` with an element that bounds everything and a
// composite element with one component.
const BASE: &str = r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0}}}},"segments":{"AA":{"elements":{"1":{"name":"code","type":"ID","min":1,"max":2,"required":true,"codes":["X","Y"]},"2":{"name":"pair","type":"AN","composite":{"1":{"name":"part","type":"ID","codes":["P","Q"]}}}}}}}"#;

fn base() -> Value {
    serde_json::from_str(BASE).unwrap()
}

// The differences between the base and the base with `patch` merged over it.
fn after(patch: Value) -> Vec<(String, ChangeKind)> {
    let mut merged = base();
    merge_patch(&mut merged, &patch);
    let (before, after) = (
        SpecModel::from_value(&base()).unwrap(),
        SpecModel::from_value(&merged).unwrap(),
    );
    let id = PackageId::parse(D).unwrap();
    let changes = differences(&id, &before, &after);
    assert!(changes.iter().all(|change| change.package == id));
    changes.into_iter().map(|c| (c.path, c.kind)).collect()
}

fn element(change: Value) -> Value {
    json!({"segments": {"AA": {"elements": {"1": change}}}})
}

fn pair(before: &str, after: &str) -> (String, String) {
    (before.to_owned(), after.to_owned())
}

fn widened(before: &str, after: &str) -> ChangeKind {
    let (before, after) = pair(before, after);
    ChangeKind::Widened { before, after }
}

fn narrowed(before: &str, after: &str) -> ChangeKind {
    let (before, after) = pair(before, after);
    ChangeKind::Narrowed { before, after }
}

fn changed(before: &str, after: &str) -> ChangeKind {
    let (before, after) = pair(before, after);
    ChangeKind::Changed { before, after }
}

const ONE: &str = "segments.AA.elements.1";

#[test]
fn what_allows_more_is_widened() {
    let cases = [
        (
            json!({"codes": ["X", "Z"]}),
            "codes",
            widened(r#"["X","Y"]"#, r#"["X","Z"]"#),
        ),
        (
            json!({"codes": null}),
            "codes",
            widened(r#"["X","Y"]"#, "any code"),
        ),
        (json!({"max": 3}), "max", widened("2", "3")),
        (json!({"max": null}), "max", widened("2", "no maximum")),
        (json!({"min": null}), "min", widened("1", "no minimum")),
        (json!({"min": 0}), "min", widened("1", "0")),
        (
            json!({"required": false}),
            "required",
            widened("required", "not required"),
        ),
    ];
    for (change, key, kind) in cases {
        assert_eq!(after(element(change)), [(format!("{ONE}.{key}"), kind)]);
    }
}

#[test]
fn what_allows_less_is_narrowed() {
    let cases = [
        (
            json!({"codes": ["X"]}),
            "codes",
            narrowed(r#"["X","Y"]"#, r#"["X"]"#),
        ),
        (json!({"max": 1}), "max", narrowed("2", "1")),
        (json!({"min": 2}), "min", narrowed("1", "2")),
    ];
    for (change, key, kind) in cases {
        assert_eq!(after(element(change)), [(format!("{ONE}.{key}"), kind)]);
    }
}

#[test]
fn a_bound_where_there_was_none_narrows() {
    let mut before = base();
    before["segments"]["AA"]["elements"]["1"] = json!({"name": "code", "type": "AN"});
    let free = SpecModel::from_value(&before).unwrap();
    let mut merged = before;
    let bounds = json!({"min": 1, "max": 5, "required": true, "codes": ["X"]});
    merge_patch(&mut merged, &element(bounds));
    let bounded = SpecModel::from_value(&merged).unwrap();
    let found = differences(&PackageId::parse(D).unwrap(), &free, &bounded);
    let kinds: Vec<&ChangeKind> = found.iter().map(|change| &change.kind).collect();
    assert_eq!(
        kinds,
        [
            &narrowed("not required", "required"),
            &narrowed("no minimum", "1"),
            &narrowed("no maximum", "5"),
            &narrowed("any code", r#"["X"]"#),
        ]
    );
}

#[test]
fn a_different_name_or_type_is_changed() {
    let renamed = after(element(json!({"name": "kind"})));
    assert_eq!(
        renamed,
        [(format!("{ONE}.name"), changed(r#""code""#, r#""kind""#))]
    );
    let retyped = after(element(json!({"type": "AN"})));
    assert_eq!(
        retyped,
        [(format!("{ONE}.type"), changed(r#""ID""#, r#""AN""#))]
    );
}

#[test]
fn a_component_is_compared_like_an_element() {
    let component = json!({"composite": {"1": {"codes": ["P", "R"]}}});
    assert_eq!(
        after(json!({"segments": {"AA": {"elements": {"2": component}}}})),
        [(
            "segments.AA.elements.2.composite.1.codes".to_owned(),
            widened(r#"["P","Q"]"#, r#"["P","R"]"#)
        )]
    );
}

#[test]
fn what_the_base_has_and_the_package_lacks_is_removed() {
    let cases = [
        (json!({"segments": {"AA": null}}), "segments.AA"),
        (
            json!({"segments": {"AA": {"elements": {"2": null}}}}),
            "segments.AA.elements.2",
        ),
        (
            json!({"segments": {"AA": {"elements": {"2": {"composite": {"1": null}}}}}}),
            "segments.AA.elements.2.composite.1",
        ),
    ];
    for (patch, path) in cases {
        assert_eq!(after(patch), [(path.to_owned(), ChangeKind::Removed)]);
    }
}

#[test]
fn what_only_the_package_holds_is_not_a_difference() {
    let added = json!({"segments": {"BB": {"elements": {"1": {"name": "n", "type": "AN"}}},
        "AA": {"elements": {"3": {"name": "n", "type": "AN"}}}}});
    assert!(after(added).is_empty());
}

#[test]
fn differences_come_in_key_order() {
    let patch = json!({"segments": {"AA": {"elements": {
        "2": {"name": "other"}, "1": {"max": 3}}}}});
    let paths: Vec<String> = after(patch).into_iter().map(|(path, _)| path).collect();
    assert_eq!(
        paths,
        [
            format!("{ONE}.max"),
            "segments.AA.elements.2.name".to_owned()
        ]
    );
}
