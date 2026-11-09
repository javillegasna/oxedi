use serde_json::{from_value, json, to_value};

use crate::spec::{SummaryModel, SummaryTableModel};

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
