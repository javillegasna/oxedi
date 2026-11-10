use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, from_value, json, to_value};

use crate::spec::{
    Builtin, ColumnModel, ElementModel, ElementPosition, LoopModel, OccurrenceModel, Pick,
    QualifierModel, SegmentModel, Sign, Spec, SpecModel, SummaryModel, SummaryTableModel,
    TermModel, TriggerModel, Usage, test_support,
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

#[test]
fn a_written_pick_first_is_kept() {
    let mut column = ColumnModel {
        loop_name: None,
        segment: None,
        conditions: BTreeMap::new(),
        occurrence: Some("clp".into()),
        pick: Some(Pick::First),
        element: Some(4),
        component: None,
        group_element: None,
        segment_index: false,
        phi: None,
    };
    let written = json!({"occurrence":"clp","pick":"first","element":4});
    assert_eq!(json_of(&column), written);
    assert_eq!(from_value::<ColumnModel>(written).unwrap(), column);
    column.pick = None;
    let unpicked = json!({"occurrence":"clp","element":4});
    assert_eq!(json_of(&column), unpicked);
    assert_eq!(from_value::<ColumnModel>(unpicked).unwrap(), column);
}

#[test]
fn a_trigger_and_occurrence_write_their_position_keys() {
    let mut model = minimal_loop();
    model.trigger.segment = "NM1".into();
    model.trigger.conditions.insert(1, "QC".into());
    let codes = BTreeMap::from([
        (
            ElementPosition {
                element: 1,
                component: None,
            },
            vec!["QC".to_string(), "IL".to_string()],
        ),
        (
            ElementPosition {
                element: 3,
                component: Some(2),
            },
            vec!["A".to_string()],
        ),
    ]);
    let occurrence = OccurrenceModel {
        segment: "NM1".into(),
        pos: 0,
        usage: Usage::Situational,
        max: None,
        qualifier: None,
        codes,
    };
    model.occurrences.insert("nm1".into(), occurrence);
    let written = json!({
        "trigger": {"segment": "NM1", "where": {"1": "QC"}},
        "occurrences": {"nm1": {
            "segment": "NM1", "pos": 0, "codes": {"1": ["QC", "IL"], "3-2": ["A"]}
        }}
    });
    assert_eq!(json_of(&model), written);
    assert_eq!(from_value::<LoopModel>(written).unwrap(), model);
}

#[test]
fn a_segment_writes_its_element_and_component_keys() {
    let named = |kind, name: &str| ElementModel {
        name: name.into(),
        ..paid(kind)
    };
    let mut procedure = named(ElementType::An, "procedure");
    procedure
        .composite
        .insert(2, named(ElementType::An, "code"));
    let model = SegmentModel {
        elements: BTreeMap::from([(1, named(ElementType::Id, "qualifier")), (4, procedure)]),
    };
    let written = json!({"elements": {
        "1": {"name": "qualifier", "type": "ID"},
        "4": {"name": "procedure", "type": "AN",
              "composite": {"2": {"name": "code", "type": "AN"}}}
    }});
    assert_eq!(json_of(&model), written);
    assert_eq!(from_value::<SegmentModel>(written).unwrap(), model);
}

#[test]
fn a_term_sign_is_written_only_when_minus() {
    let plain = json!({"occurrence":"clp","elements":[4]});
    for written in [
        json!({"occurrence":"clp","elements":[4],"sign":null}),
        json!({"occurrence":"clp","elements":[4],"sign":"+"}),
        plain.clone(),
    ] {
        let term = from_value::<TermModel>(written).unwrap();
        assert_eq!(term.sign, Sign::Plus);
        assert_eq!(json_of(&term), plain);
    }
    let minus = json!({"occurrence":"clp","elements":[4],"sign":"-"});
    let term = from_value::<TermModel>(minus.clone()).unwrap();
    assert_eq!(term.sign, Sign::Minus);
    assert_eq!(json_of(&term), minus);
}

#[test]
fn every_shipped_spec_reads_into_its_model() {
    let specs: Vec<Spec> = Builtin::all()
        .iter()
        .map(|builtin| (*builtin.spec).clone())
        .chain([test_support::spec()])
        .collect();
    assert_eq!(specs.len(), 3);
    for (spec, name) in specs.iter().zip(["835", "835", "t"]) {
        let model = spec.model().unwrap();
        assert_eq!(model.name, spec.name());
        assert_eq!(model.name, name);
    }
}

#[test]
fn a_model_the_loader_refuses_fails_with_its_text() {
    let mut model = test_support::spec().model().unwrap();
    model.loops.get_mut("head").unwrap().max = Some(0);
    assert_eq!(
        Spec::from_model(&model).unwrap_err().to_string(),
        "loop \"head\" has \"max\" 0; a loop that may appear appears at least once"
    );
}

#[test]
fn a_built_model_loads() {
    let mut first = minimal_loop();
    first.occurrences.insert(
        "st".into(),
        OccurrenceModel {
            segment: "ST".into(),
            pos: 0,
            usage: Usage::Situational,
            max: None,
            qualifier: None,
            codes: BTreeMap::new(),
        },
    );
    let model = SpecModel {
        name: "built".into(),
        version: None,
        loops: BTreeMap::from([("st".into(), first)]),
        segments: BTreeMap::new(),
        tables: BTreeMap::new(),
        balancing: BTreeMap::new(),
        summaries: BTreeMap::new(),
        default_table: None,
    };
    let spec = Spec::from_model(&model).unwrap();
    assert!(spec.loop_id("st").is_some());
    let written = json!({"name":"built","loops":{"st":{"trigger":{"segment":"ST"},
        "occurrences":{"st":{"segment":"ST","pos":0}}}}});
    assert_eq!(
        serde_json::from_str::<Value>(&model.to_json().unwrap()).unwrap(),
        written
    );
}

#[test]
fn a_position_key_the_loader_refuses_is_refused_naming_the_map() {
    type Read = Box<dyn Fn(&str) -> String>;
    let reads: [(Read, &str); 4] = [
        (
            Box::new(|key| {
                from_value::<TriggerModel>(json!({"segment":"CLP","where":{key:"X"}}))
                    .unwrap_err()
                    .to_string()
            }),
            "trigger \"where\"",
        ),
        (
            Box::new(|key| {
                from_value::<ColumnModel>(json!({"segment":"CLP","where":{key:"X"},"element":1}))
                    .unwrap_err()
                    .to_string()
            }),
            "column \"where\"",
        ),
        (
            Box::new(|key| {
                from_value::<SegmentModel>(json!({"elements":{key:{"name":"a","type":"AN"}}}))
                    .unwrap_err()
                    .to_string()
            }),
            "segment \"elements\"",
        ),
        (
            Box::new(|key| {
                let component = json!({key:{"name":"b","type":"AN"}});
                let element = json!({"name":"a","type":"AN","composite":component});
                from_value::<SegmentModel>(json!({"elements":{"1":element}}))
                    .unwrap_err()
                    .to_string()
            }),
            "element \"composite\"",
        ),
    ];
    for key in ["0", "01", "+1", "x"] {
        for (read, map) in &reads {
            assert_eq!(
                read(key),
                format!(
                    "{map} key {key:?} is not a position: positions are 1-based integers written in canonical form"
                )
            );
        }
    }
    let trigger = from_value::<TriggerModel>(json!({"segment":"CLP","where":{"10":"X"}}));
    assert_eq!(
        trigger.unwrap().conditions,
        BTreeMap::from([(10, "X".to_string())])
    );
    let segment = from_value::<SegmentModel>(json!({"elements":{"12":{"name":"a","type":"AN"}}}));
    assert_eq!(
        segment.unwrap().elements.keys().copied().collect::<Vec<_>>(),
        vec![12]
    );
}
