use serde_json::{Value, json};

use super::super::change::ChangeKind;
use super::super::narrow::differences;
use super::super::*;
use super::BASE;
use crate::spec_load::merge_patch;
use crate::spec_load::model::SpecModel;

const D: &str = "test/dictionary/005010";

// The base and the keys only a views package holds.
fn base() -> Value {
    let mut base: Value = serde_json::from_str(BASE).unwrap();
    let views: Value = serde_json::from_str(VIEWS).unwrap();
    merge_patch(&mut base, &views);
    base["name"] = json!("t");
    base
}

const VIEWS: &str = r#"{"tables":{"t":{"loops":["a"],"columns":{"c":{"segment":"AA","element":1}}}},"summaries":{"n":{"count":{"table":"t"}}},"default_table":"t"}"#;

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
            json!({"usage":"situational"}),
            "usage",
            widened(r#""required""#, r#""situational""#),
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
    let bounds = json!({"min": 1, "max": 5, "usage":"required", "codes": ["X"]});
    merge_patch(&mut merged, &element(bounds));
    let bounded = SpecModel::from_value(&merged).unwrap();
    let found = differences(&PackageId::parse(D).unwrap(), &free, &bounded);
    let kinds: Vec<&ChangeKind> = found.iter().map(|change| &change.kind).collect();
    assert_eq!(
        kinds,
        [
            &narrowed(r#""situational""#, r#""required""#),
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
fn what_only_the_package_holds_is_added() {
    let element = json!({"name": "n", "type": "AN"});
    let added = json!({"segments": {
        "BB": {"elements": {"1": element}},
        "AA": {"elements": {"3": element, "2": {"composite": {"2": element}}}}}});
    let paths: Vec<String> = after(added)
        .into_iter()
        .inspect(|(_, kind)| assert!(matches!(kind, ChangeKind::Added { .. })))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        paths,
        [
            "segments.AA.elements.2.composite.2",
            "segments.AA.elements.3",
            "segments.BB"
        ]
    );
    let kind = after(json!({"segments": {"AA": {"elements": {"3": element}}}}));
    let text = r#"{"name":"n","type":"AN"}"#.to_owned();
    assert_eq!(kind[0].1, ChangeKind::Added { after: text });
}

#[test]
fn an_equal_value_is_not_a_difference() {
    let limits = json!({"min": 0, "max": null});
    let free = json!({"segments": {"AA": {"elements": {"2": limits}}}});
    assert!(after(free).is_empty());
    assert!(after(element(json!({"codes": ["Y", "X"]}))).is_empty());
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

fn loop_a(change: Value) -> Value {
    json!({"loops": {"a": change}})
}

fn occurrence(name: &str, change: Value) -> Value {
    loop_a(json!({"occurrences": {name: change}}))
}

#[test]
fn a_loop_or_an_occurrence_that_allows_more_is_widened() {
    let cases = [
        (
            loop_a(json!({"usage": "situational"})),
            "a.usage",
            widened("\"required\"", "\"situational\""),
        ),
        (loop_a(json!({"max": 3})), "a.max", widened("2", "3")),
        (
            loop_a(json!({"max": null})),
            "a.max",
            widened("2", "no maximum"),
        ),
        (
            occurrence("aa", json!({"usage": "situational"})),
            "a.occurrences.aa.usage",
            widened("\"required\"", "\"situational\""),
        ),
        (
            occurrence("aa", json!({"max": 2})),
            "a.occurrences.aa.max",
            widened("1", "2"),
        ),
        (
            occurrence("aa", json!({"elements": {"1": {"codes": ["X", "Y"]}}})),
            "a.occurrences.aa.elements.1.codes",
            widened(r#"["X"]"#, r#"["X","Y"]"#),
        ),
        (
            occurrence("ab", json!({"qualifier": {"codes": ["X", "Y"]}})),
            "a.occurrences.ab.qualifier.codes",
            widened(r#"["Y"]"#, r#"["X","Y"]"#),
        ),
    ];
    for (patch, path, kind) in cases {
        assert_eq!(after(patch), [(format!("loops.{path}"), kind)]);
    }
}

#[test]
fn a_loop_or_an_occurrence_that_allows_less_is_narrowed() {
    let cases = [
        (loop_a(json!({"max": 1})), "a.max", narrowed("2", "1")),
        (
            occurrence("ab", json!({"usage": "required"})),
            "a.occurrences.ab.usage",
            narrowed("\"situational\"", "\"required\""),
        ),
        (
            occurrence("ab", json!({"max": 1})),
            "a.occurrences.ab.max",
            narrowed("no maximum", "1"),
        ),
    ];
    for (patch, path, kind) in cases {
        assert_eq!(after(patch), [(format!("loops.{path}"), kind)]);
    }
}

#[test]
fn a_restriction_the_package_adds_is_added() {
    let codes = occurrence("aa", json!({"elements": {"2": {"codes": ["Z"]}}}));
    let qualifier = occurrence("aa", json!({"qualifier": {"element": 1, "codes": ["X"]}}));
    let loop_b = json!({"loops": {"b": {"trigger": {"segment": "BB"}}}});
    let cases = [
        (
            codes,
            "loops.a.occurrences.aa.elements.2",
            r#"{"codes":["Z"]}"#,
        ),
        (
            qualifier,
            "loops.a.occurrences.aa.qualifier",
            r#"{"element":1,"codes":["X"]}"#,
        ),
        (loop_b, "loops.b", r#"{"trigger":{"segment":"BB"}}"#),
    ];
    for (patch, path, value) in cases {
        let added = ChangeKind::Added {
            after: value.to_owned(),
        };
        assert_eq!(after(patch), [(path.to_owned(), added)]);
    }
}

#[test]
fn structure_is_changed_not_widened() {
    let trigger = json!({"segment": "AA", "where": {"1": "X"}});
    let cases = [
        (
            loop_a(json!({"trigger": trigger})),
            "a.trigger",
            changed(
                r#"{"segment":"AA"}"#,
                r#"{"segment":"AA","where":{"1":"X"}}"#,
            ),
        ),
        (
            loop_a(json!({"parent": "b"})),
            "a.parent",
            changed("null", r#""b""#),
        ),
        (
            occurrence("aa", json!({"pos": 5})),
            "a.occurrences.aa.pos",
            changed("0", "5"),
        ),
        (
            occurrence("aa", json!({"segment": "BB"})),
            "a.occurrences.aa.segment",
            changed(r#""AA""#, r#""BB""#),
        ),
        (
            occurrence("ab", json!({"qualifier": {"element": 2}})),
            "a.occurrences.ab.qualifier.element",
            changed("1", "2"),
        ),
    ];
    for (patch, path, kind) in cases {
        assert_eq!(after(patch), [(format!("loops.{path}"), kind)]);
    }
}

#[test]
fn a_removed_loop_or_occurrence_is_listed() {
    let cases = [
        (json!({"loops": {"a": null}}), "loops.a"),
        (occurrence("aa", Value::Null), "loops.a.occurrences.aa"),
        (
            occurrence("ab", json!({"qualifier": null})),
            "loops.a.occurrences.ab.qualifier",
        ),
        (json!({"balancing": {"r": null}}), "balancing.r"),
        (json!({"tables": {"t": null}}), "tables.t"),
        (json!({"summaries": {"n": null}}), "summaries.n"),
        (json!({"version": null}), "version"),
        (json!({"default_table": null}), "default_table"),
    ];
    for (patch, path) in cases {
        assert_eq!(after(patch), [(path.to_owned(), ChangeKind::Removed)]);
    }
}

#[test]
fn a_rule_or_a_view_the_package_adds_is_added() {
    let table = json!({"loops": ["a"]});
    let patch = json!({"tables": {"u": table}, "summaries": {"m": {"count": {"table": "t"}}}});
    let paths: Vec<String> = after(patch)
        .into_iter()
        .inspect(|(_, kind)| assert!(matches!(kind, ChangeKind::Added { .. })))
        .map(|(path, _)| path)
        .collect();
    assert_eq!(paths, ["tables.u", "summaries.m"]);
}

#[test]
fn the_version_the_rules_and_the_views_are_changed() {
    let table = json!({"loops": ["a"], "columns": {"c": {"segment": "AA", "element": 2}}});
    let distinct = json!({"count": null, "distinct": {"table": "t", "column": "c"}});
    let cases = [
        (json!({"version": {"values": ["W"]}}), "version"),
        (json!({"balancing": {"r": {"per": "b"}}}), "balancing.r"),
        (json!({"tables": {"t": table}}), "tables.t"),
        (json!({"summaries": {"n": distinct}}), "summaries.n"),
        (json!({"default_table": "u"}), "default_table"),
    ];
    for (patch, path) in cases {
        let found = after(patch);
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!(found[0].0, path);
        assert!(matches!(found[0].1, ChangeKind::Changed { .. }), "{path}");
    }
    assert_eq!(
        after(json!({"default_table": "u"}))[0].1,
        changed(r#""t""#, r#""u""#)
    );
}
