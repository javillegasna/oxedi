use serde_json::{from_value, json, to_value};

use crate::spec::{SummaryModel, SummaryTableModel};

#[test]
fn a_summary_holds_one_operation() {
    let written = json!({"count":{"table":"claims"}});
    let model = from_value::<SummaryModel>(written.clone()).unwrap();
    assert_eq!(
        model,
        SummaryModel::Count(SummaryTableModel {
            table: "claims".into()
        })
    );
    assert_eq!(to_value(&model).unwrap(), written);
    assert!(
        from_value::<SummaryModel>(json!({"count":{"table":"a"},"sum":{"table":"a","column":"b"}}))
            .is_err()
    );
    assert!(from_value::<SummaryModel>(json!({})).is_err());
    assert!(
        from_value::<SummaryModel>(
            json!({"row":{"table":"a","fields":[{"name":"n","column":"c","x":1}]}})
        )
        .is_err()
    );
}
