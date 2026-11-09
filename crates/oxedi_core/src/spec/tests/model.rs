use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, from_value, json, to_value};

use crate::spec::{
    ElementModel, LoopModel, OccurrenceModel, QualifierModel, SummaryModel, SummaryTableModel,
    TriggerModel, Usage,
};
use crate::value::{ElementType, TypeCodeError};

fn minimal_loop() -> LoopModel {
    LoopModel {
        parent: None,
        trigger: TriggerModel {
            segment: "ST".into(),
            conditions: BTreeMap::new(),
        },
        occurrences: BTreeMap::new(),
        max: None,
        usage: Usage::Situational,
        end: None,
        control: None,
    }
}

fn json_of<T: Serialize>(model: &T) -> Value {
    to_value(model).unwrap()
}

fn paid(kind: ElementType) -> ElementModel {
    ElementModel {
        name: "paid".into(),
        kind,
        required: false,
        min: None,
        max: None,
        codes: None,
        composite: BTreeMap::new(),
    }
}

#[test]
fn a_summary_holds_one_operation() {
    let written = json!({"count":{"table":"claims"}});
    let model = from_value::<SummaryModel>(written.clone()).unwrap();
    let table = SummaryTableModel {
        table: "claims".into(),
    };
    assert_eq!(model, SummaryModel::Count(table));
    assert_eq!(json_of(&model), written);
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
    assert_eq!(json_of(&qualifier), json!({"element":1,"codes":["QC"]}));
    qualifier.component = Some(2);
    assert_eq!(
        json_of(&qualifier),
        json!({"element":1,"component":2,"codes":["QC"]})
    );
}

#[test]
fn a_loop_model_writes_no_default_and_no_null() {
    let mut model = minimal_loop();
    assert_eq!(json_of(&model), json!({"trigger":{"segment":"ST"}}));
    model.usage = Usage::Required;
    assert_eq!(
        json_of(&model),
        json!({"trigger":{"segment":"ST"},"usage":"required"})
    );
}

#[test]
fn null_reads_as_absent_or_default() {
    let written =
        json!({"trigger":{"segment":"ST"},"parent":null,"end":null,"max":null,"usage":null});
    assert_eq!(from_value::<LoopModel>(written).unwrap(), minimal_loop());
    let written = json!({"segment":"NM1","pos":10,"usage":"situational","codes":{}});
    let occurrence = from_value::<OccurrenceModel>(written).unwrap();
    assert_eq!(json_of(&occurrence), json!({"segment":"NM1","pos":10}));
}

#[test]
fn an_r_scale_is_written_only_when_not_the_default() {
    let r = |scale| ElementType::R { scale };
    for (kind, written) in [
        (r(4), json!({"name":"paid","type":"R","scale":4})),
        (r(2), json!({"name":"paid","type":"R"})),
        (ElementType::N(2), json!({"name":"paid","type":"N2"})),
    ] {
        let model = paid(kind);
        assert_eq!(json_of(&model), written);
        assert_eq!(from_value::<ElementModel>(written).unwrap(), model);
    }
    let written = json!({"name":"paid","type":"R","scale":2,"required":false});
    assert_eq!(from_value::<ElementModel>(written).unwrap(), paid(r(2)));
}

#[test]
fn a_type_the_loader_refuses_is_refused_with_its_text() {
    let refused = from_value::<ElementModel>(json!({"name":"x","type":"AN","scale":2}));
    let expected = TypeCodeError::ScaleWithoutR { kind: "AN".into() }.to_string();
    let found = refused.unwrap_err().to_string();
    assert!(found.starts_with(&expected), "{found}");
}

#[test]
fn an_element_codes_list_is_kept_as_written() {
    let mut model = paid(ElementType::Id);
    model.codes = Some(vec![]);
    let written = json!({"name":"paid","type":"ID","codes":[]});
    assert_eq!(json_of(&model), written);
    assert_eq!(from_value::<ElementModel>(written).unwrap(), model);
    model.codes = None;
    assert_eq!(json_of(&model), json!({"name":"paid","type":"ID"}));
}

#[test]
fn an_unknown_key_is_refused() {
    assert!(from_value::<LoopModel>(json!({"trigger":{"segment":"ST"},"colour":1})).is_err());
    let occurrence = json!({"segment":"NM1","pos":10,"colour":1});
    assert!(from_value::<OccurrenceModel>(occurrence).is_err());
}
