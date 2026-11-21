use serde::Deserialize;
use serde_json::{Value, from_value, json};

use super::StrictValue;
use crate::spec::{Builtin, SpecModel, SummaryModel, TriggerModel};

fn refusal(source: &Value) -> String {
    SpecModel::read(source).unwrap_err().to_string()
}

#[test]
fn a_trigger_written_as_an_array_is_refused_at_its_path() {
    assert!(from_value::<TriggerModel>(json!(["ST", {}])).is_ok());
    let source = json!({"name":"x","loops":{"a":{"trigger":["ST",{}]}}});
    assert_eq!(
        refusal(&source),
        "spec: at loops.a.trigger: invalid type: sequence, expected a trigger object"
    );
}

#[test]
fn a_column_written_as_an_array_is_refused_at_its_path() {
    let source =
        json!({"name":"x","loops":{},"tables":{"claims":{"loops":[],"columns":{"paid":[]}}}});
    assert_eq!(
        refusal(&source),
        "spec: at tables.claims.columns.paid: invalid type: sequence, expected a column object"
    );
}

#[test]
fn a_summary_operation_written_as_an_array_is_refused_through_its_proxy() {
    assert!(from_value::<SummaryModel>(json!({"count":["claims"]})).is_ok());
    let source = json!({"name":"x","loops":{},"summaries":{"n":{"count":["claims"]}}});
    assert_eq!(
        refusal(&source),
        "spec: at summaries.n.count: invalid type: sequence, expected a summary operation object"
    );
}

#[test]
fn a_component_written_as_an_array_is_refused_inside_a_flattened_element() {
    let source = json!({"name":"x","loops":{},"segments":{"CLP":{"elements":{"1":{
        "name":"id","type":"AN","composite":{"1":[]}}}}}});
    assert_eq!(
        refusal(&source),
        "spec: at segments.CLP.elements.1.composite.1: invalid type: sequence, expected an element object"
    );
}

#[test]
fn a_struct_inside_an_array_is_refused_at_its_index() {
    let source = json!({"name":"x","loops":{},"balancing":{"r":{"per":"a","target":[[2]]}}});
    assert_eq!(
        refusal(&source),
        "spec: at balancing.r.target[0]: invalid type: sequence, expected a balancing value object"
    );
}

#[test]
fn an_enum_variant_reads_its_payload_strictly() {
    #[derive(Debug, Deserialize)]
    enum Wrapped {
        Trigger(TriggerModel),
    }
    let read = |value: &Value| serde_path_to_error::deserialize::<_, Wrapped>(StrictValue(value));
    let Ok(Wrapped::Trigger(trigger)) = read(&json!({"Trigger":{"segment":"ST"}})) else {
        panic!("a trigger object is read");
    };
    assert_eq!(trigger.segment, "ST");
    let err = read(&json!({"Trigger":["ST",{}]})).unwrap_err();
    assert_eq!(err.path().to_string(), "Trigger");
    assert_eq!(
        err.inner().to_string(),
        "invalid type: sequence, expected a trigger object"
    );
}

#[test]
fn every_builtin_reads_as_serde_json_reads_it() {
    for builtin in Builtin::all() {
        let source = &builtin.spec.source;
        let strict = SpecModel::read(source).unwrap();
        let plain = from_value::<SpecModel>(source.clone()).unwrap();
        assert_eq!(
            strict, plain,
            "{} {:?}",
            builtin.transaction, builtin.version
        );
    }
}

#[test]
fn scalars_and_maps_read_as_serde_json_reads_them() {
    let source = json!({
        "name": "x",
        "version": {"segment": "ST", "element": 1, "values": ["x"]},
        "loops": {"a": {
            "trigger": {"segment": "ST", "where": {"1": "835", "2": "A"}},
            "max": 3,
            "usage": null
        }},
        "tables": {"t": {"loops": ["a"], "columns": {
            "first": {"loop": "a", "segment": "ST", "element": 1, "pick": "first"},
            "second": {"loop": "a", "segment": "ST", "element": 1, "pick": 2}
        }}},
        "balancing": {"r": {"per": "a", "target": [{"occurrence": "o", "elements": [2]}],
            "sum": [{"occurrence": "o", "elements": [3], "sign": "-", "component": null}]}}
    });
    let strict = SpecModel::read(&source);
    let plain = from_value::<SpecModel>(source.clone());
    assert_eq!(strict.unwrap(), plain.unwrap());
}
