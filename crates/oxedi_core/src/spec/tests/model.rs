use serde_json::{from_value, json, to_value};

use crate::spec::{QualifierModel, SummaryModel, SummaryTableModel};

#[test]
fn a_summary_holds_one_operation() {
    let written = json!({"count":{"table":"claims"}});
    let model = from_value::<SummaryModel>(written.clone()).unwrap();
    let table = SummaryTableModel {
        table: "claims".into(),
    };
    assert_eq!(model, SummaryModel::Count(table));
    assert_eq!(to_value(&model).unwrap(), written);
    for refused in [
        json!({"count":{"table":"a"},"sum":{"table":"a","column":"b"}}),
        json!({}),
        json!({"row":{"table":"a","fields":[{"name":"n","column":"c","x":1}]}}),
    ] {
        assert!(from_value::<SummaryModel>(refused).is_err());
    }
}

#[test]
fn a_qualifier_writes_its_component_only_when_present() {
    let mut qualifier = QualifierModel {
        element: 1,
        component: None,
        codes: vec!["QC".into()],
    };
    assert_eq!(
        to_value(&qualifier).unwrap(),
        json!({"element":1,"codes":["QC"]})
    );
    qualifier.component = Some(2);
    assert_eq!(
        to_value(&qualifier).unwrap(),
        json!({"element":1,"component":2,"codes":["QC"]})
    );
}
