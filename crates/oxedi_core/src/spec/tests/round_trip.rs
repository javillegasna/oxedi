//! Every spec the core ships or tests reads into its model, writes back as
//! canonical JSON and loads to the same spec.

use serde_json::Value;

use crate::spec::test_support;
use crate::spec::{Builtin, Spec, spec_schema};

use super::CLP_ONLY;

// Every default and absence written out: none of it may come back in the text.
const EXPLICIT_DEFAULTS: &str = r#"{"name":"explicit",
    "loops":{"A":{"parent":null,"end":null,"max":null,"usage":"situational",
        "trigger":{"segment":"AA","where":{}},
        "occurrences":{
            "aa":{"segment":"AA","pos":0,"usage":null,"codes":{}},
            "a1":{"segment":"A1","pos":1,"usage":"situational","max":2,"codes":{}}
        }}},
    "segments":{
        "AA":{"elements":{
            "1":{"name":"amount","type":"R","scale":2,"required":false},
            "2":{"name":"note","type":"AN","required":false}
        }},
        "A1":{"elements":{"1":{"name":"extra","type":"R","scale":2,"required":false}}}
    },
    "tables":{"rows":{"loops":["A"],"ref":null,"columns":{
        "amount":{"segment":"AA","element":1},
        "extra":{"occurrence":"a1","pick":"first","element":1,"segment_index":false,"phi":null}
    }}},
    "balancing":{"sums":{"per":"A",
        "target":[{"occurrence":"aa","elements":[1],"sign":null}],
        "sum":[{"occurrence":"a1","elements":[1],"sign":"+"}]}},
    "summaries":{}
}"#;

fn round_trip_specs() -> Vec<(&'static str, Spec)> {
    let [five, four] = Builtin::all() else {
        panic!("the registry ships two built-in specs");
    };
    let parser = ok(
        "edi_835_parser",
        "merge the patch",
        Spec::builtin_835().merge_patch(include_str!("../../../specs/edi_835_parser.json")),
    );
    vec![
        ("5010", five.spec.as_ref().clone()),
        ("4010", four.spec.as_ref().clone()),
        ("edi_835_parser", parser),
        ("test_support", test_support::spec()),
        (
            "clp_only",
            ok("clp_only", "load", Spec::from_json(CLP_ONLY)),
        ),
        (
            "explicit_defaults",
            ok(
                "explicit_defaults",
                "load",
                Spec::from_json(EXPLICIT_DEFAULTS),
            ),
        ),
    ]
}

// Unwraps a step of the spec `name`, so a failure says which spec and step broke.
fn ok<T, E: std::fmt::Display>(name: &str, step: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| panic!("{name}: {step}: {e}"))
}

fn written(name: &str, spec: &Spec) -> String {
    ok(name, "write", ok(name, "model", spec.model()).to_json())
}

#[test]
fn every_spec_round_trips() {
    for (name, spec) in round_trip_specs() {
        let m1 = ok(name, "model", spec.model());
        let text = ok(name, "write", m1.to_json());
        let m2 = ok(
            name,
            "model of the text",
            ok(name, "load", Spec::from_json(&text)).model(),
        );
        assert_eq!(m1, m2, "{name}: the model changed");
        let as_value: Value = ok(name, "parse the text", serde_json::from_str(&text));
        assert_eq!(
            as_value,
            ok(name, "model to value", serde_json::to_value(&m2)),
            "{name}: the text is not the model"
        );
        assert_eq!(
            m2.to_json().unwrap(),
            text,
            "{name}: a second pass changed the text"
        );
    }
}

#[test]
fn a_round_trip_compiles_to_the_same_spec() {
    for (name, spec) in round_trip_specs() {
        let back = ok(name, "load", Spec::from_json(&written(name, &spec)));
        assert_eq!(back.name(), spec.name(), "{name}: name");
        assert_eq!(back.loops(), spec.loops(), "{name}: loops");
        assert_eq!(
            back.segments().collect::<Vec<_>>(),
            spec.segments().collect::<Vec<_>>(),
            "{name}: segments"
        );
        assert_eq!(back.tables(), spec.tables(), "{name}: tables");
        assert_eq!(back.balancing(), spec.balancing(), "{name}: balancing");
        assert_eq!(back.summaries(), spec.summaries(), "{name}: summaries");
        assert_eq!(
            back.default_table(),
            spec.default_table(),
            "{name}: default table"
        );
        assert_eq!(back.version(), spec.version(), "{name}: version");
        assert_eq!(spec_schema(&back), spec_schema(&spec), "{name}: schema");
    }
}

fn holds_null(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.iter().any(holds_null),
        Value::Object(members) => members.values().any(holds_null),
        _ => false,
    }
}

#[test]
fn a_written_spec_holds_no_null() {
    for (name, spec) in round_trip_specs() {
        let value: Value = ok(
            name,
            "parse the text",
            serde_json::from_str(&written(name, &spec)),
        );
        assert!(!holds_null(&value), "{name}: the written spec holds a null");
    }
}

fn lacks(value: &Value, keys: &[&str]) {
    let object = value.as_object().unwrap();
    for key in keys {
        assert!(!object.contains_key(*key), "`{key}` was written in {value}");
    }
}

#[test]
fn explicit_defaults_are_not_written() {
    let name = "explicit_defaults";
    let text = written(name, &ok(name, "load", Spec::from_json(EXPLICIT_DEFAULTS)));
    let root: Value = serde_json::from_str(&text).unwrap();
    lacks(&root, &["summaries"]);
    let a = &root["loops"]["A"];
    lacks(a, &["parent", "end", "max", "usage"]);
    lacks(&a["trigger"], &["where"]);
    for occurrence in a["occurrences"].as_object().unwrap().values() {
        lacks(occurrence, &["usage", "codes"]);
    }
    let segments = root["segments"].as_object().unwrap();
    for element in segments
        .values()
        .flat_map(|s| s["elements"].as_object().unwrap().values())
    {
        lacks(element, &["required", "scale"]);
    }
    let table = &root["tables"]["rows"];
    lacks(table, &["ref"]);
    let column = &table["columns"]["extra"];
    assert_eq!(column["pick"], "first");
    lacks(column, &["segment_index", "phi"]);
    let rule = &root["balancing"]["sums"];
    for term in rule["target"]
        .as_array()
        .unwrap()
        .iter()
        .chain(rule["sum"].as_array().unwrap())
    {
        lacks(term, &["sign"]);
    }
}
