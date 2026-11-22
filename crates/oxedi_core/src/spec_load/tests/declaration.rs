//! What a model's presence words write and read: a model writes no default and no `null`,
//! and reads a written default or `null` back as the same model.

use std::collections::BTreeMap;

use serde_json::{from_value, json};

use crate::spec_load::{ColumnModel, LoopModel, SpecModel, TableModel, TriggerModel};

#[test]
fn a_table_and_its_columns_write_no_default_and_no_null() {
    let model = TableModel {
        loops: vec!["A".to_string()],
        reference: None,
        segment: None,
        repeat: None,
        columns: BTreeMap::from([(
            "c".to_string(),
            ColumnModel {
                loop_name: None,
                segment: None,
                conditions: BTreeMap::new(),
                occurrence: None,
                pick: None,
                element: None,
                component: None,
                group_element: None,
                segment_index: false,
                phi: None,
            },
        )]),
        phi_seed: None,
    };
    let written = json!({"loops": ["A"], "columns": {"c": {}}});
    assert_eq!(serde_json::to_value(&model).unwrap(), written);
    let nulls = json!({
        "loops": ["A"], "ref": null, "segment": null, "phi_seed": null,
        "columns": {"c": {
            "loop": null, "segment": null, "where": {}, "occurrence": null, "pick": null,
            "element": null, "component": null, "group_element": null,
            "segment_index": false, "phi": null
        }}
    });
    assert_eq!(from_value::<TableModel>(nulls.clone()).unwrap(), model);
}

#[test]
fn a_spec_model_writes_no_default_and_no_null() {
    let model = SpecModel {
        name: "n".to_string(),
        version: None,
        loops: BTreeMap::from([(
            "A".to_string(),
            LoopModel {
                parent: None,
                trigger: TriggerModel {
                    segment: "ST".to_string(),
                    conditions: BTreeMap::new(),
                },
                occurrences: BTreeMap::new(),
                max: None,
                usage: Default::default(),
                end: None,
                control: None,
            },
        )]),
        segments: BTreeMap::new(),
        tables: BTreeMap::new(),
        balancing: BTreeMap::new(),
        summaries: BTreeMap::new(),
        default_table: None,
    };
    let written = json!({"name": "n", "loops": {"A": {"trigger": {"segment": "ST"}}}});
    assert_eq!(serde_json::to_value(&model).unwrap(), written);
    let defaults = json!({
        "name": "n", "loops": {"A": {"trigger": {"segment": "ST"}}},
        "segments": {}, "tables": {}, "balancing": {}, "summaries": {}
    });
    assert_eq!(from_value::<SpecModel>(defaults).unwrap(), model);
}
