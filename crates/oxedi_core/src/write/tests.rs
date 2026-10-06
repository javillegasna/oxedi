//! The write plan over the built-in specs and over a small spec whose
//! tables are patched to raise each refusal.

use super::*;
use crate::spec::Spec;

/// `env` is an envelope with a required `head`; `head` holds two NM
/// occurrences told apart by NM01, a repeating DT and an AJ segment;
/// `group` sits between `head` and its required `item`.
const BASE: &str = r#"{"name":"w",
 "loops":{
  "env":{"trigger":{"segment":"EV"},"end":"EE",
     "control":{"opener_element":1,"closer_element":2,"count_element":1,"count":"children"},
     "occurrences":{"ev":{"segment":"EV","pos":1,"usage":"required","max":1}}},
  "head":{"parent":"env","trigger":{"segment":"HD"},"usage":"required","occurrences":{
      "hd":{"segment":"HD","pos":10,"usage":"required","max":1},
      "nm_a":{"segment":"NM","pos":20,"qualifier":{"element":1,"codes":["A"]}},
      "nm_b":{"segment":"NM","pos":20,"qualifier":{"element":1,"codes":["B","C"]}},
      "dt":{"segment":"DT","pos":30,"max":3},
      "aj":{"segment":"AJ","pos":90}}},
  "group":{"parent":"head","trigger":{"segment":"GR"},"occurrences":{
      "gr":{"segment":"GR","pos":40,"usage":"required"}}},
  "item":{"parent":"group","trigger":{"segment":"IT"},"usage":"required","occurrences":{
      "it":{"segment":"IT","pos":50,"usage":"required"}}}
 },
 "segments":{
   "EV":{"elements":{"1":{"name":"control","type":"AN","required":true}}},
   "HD":{"elements":{"1":{"name":"id","type":"AN","required":true}}},
   "NM":{"elements":{"1":{"name":"kind","type":"ID","required":true,"codes":["A","B","C"]},
       "2":{"name":"entity_type","type":"ID","required":true,"codes":["1","2"]},
       "3":{"name":"name","type":"AN"}}},
   "DT":{"elements":{"1":{"name":"qualifier","type":"ID","required":true},"2":{"name":"date","type":"AN"}}},
   "GR":{"elements":{"1":{"name":"number","type":"N0","required":true}}},
   "IT":{"elements":{"1":{"name":"code","type":"AN","required":true},
       "2":{"name":"pair","type":"AN","composite":{
           "1":{"name":"pair_kind","type":"ID","required":true},"2":{"name":"pair_value","type":"AN"}}}}},
   "AJ":{"elements":{"1":{"name":"group","type":"ID","required":true},
       "2":{"name":"reason","type":"ID","required":true},"3":{"name":"amount","type":"R","required":true}}}
 },
 "tables":{
   "heads":{"loops":["head"],"ref":"head","columns":{
       "id":{"segment":"HD","element":1},
       "a_type":{"occurrence":"nm_a","element":2},
       "a_name":{"occurrence":"nm_a","element":3},
       "start":{"segment":"DT","where":{"1":"S"},"element":2},
       "first_kind":{"occurrence":"dt","element":1},
       "first_date":{"occurrence":"dt","element":2},
       "second_kind":{"occurrence":"dt","pick":2,"element":1},
       "second_date":{"occurrence":"dt","pick":2,"element":2}}},
   "items":{"loops":["item"],"ref":"item","columns":{
       "code":{"segment":"IT","element":1},
       "group_number":{"loop":"group","occurrence":"gr","element":1}}},
   "adjustments":{"loops":["head"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
       "group":{"element":1},"reason":{"group_element":0},"amount":{"group_element":1}}}
 }
}"#;

fn spec(patch: &str) -> Spec {
    Spec::from_json(BASE).unwrap().merge_patch(patch).unwrap()
}

fn refusals(patch: &str) -> Vec<String> {
    match WritePlan::new(&spec(patch)) {
        Ok(_) => Vec::new(),
        Err(error) => error.refusals.iter().map(ToString::to_string).collect(),
    }
}

fn loop_plan<'p>(spec: &Spec, plan: &'p WritePlan, name: &str) -> &'p LoopPlan {
    &plan.loops[spec.loop_id(name).unwrap().index()]
}

#[test]
fn both_built_in_specs_can_be_written() {
    for spec in [Spec::builtin_835(), Spec::builtin_835_4010()] {
        let plan = WritePlan::new(&spec).unwrap();
        let claims = spec
            .tables()
            .iter()
            .position(|t| t.name == "claims")
            .unwrap();
        assert_eq!(
            loop_plan(&spec, &plan, "2000").instances,
            Instances::Groups { table: claims }
        );
        assert_eq!(
            loop_plan(&spec, &plan, "interchange").instances,
            Instances::Envelope
        );
    }
}

#[test]
fn the_built_in_tables_without_their_writing_columns_name_what_is_missing() {
    let spec = Spec::builtin_835()
        .merge_patch(
            r#"{"tables":{"payments":{"columns":{"payer_technical_contact_name":null,
                "payer_technical_contact_qualifier":null,"payer_technical_contact_number":null}},
                "claims":{"columns":{"header_number":null,"rendering_provider_entity_type":null}}}}"#,
        )
        .unwrap();
    let error = WritePlan::new(&spec).unwrap_err();
    assert_eq!(
        error.to_string(),
        "spec \"835\" cannot be written (3 reasons)\n\
         1. occurrence \"payer_technical_contact_information\" (\"PER\" where PER01 is \"BL\") is required in every loop \"1000A\", but no column writes it\n\
         2. occurrence \"header_number\" (\"LX\") is required in every loop \"2000\", but no column writes it\n\
         3. required element NM102 (entity_type_qualifier) of occurrence \"service_provider_name\" in loop \"2100\", written by table \"claims\", has no column and no single code to write"
    );
}

#[test]
fn each_column_lands_in_its_occurrence_with_the_codes_that_select_it() {
    let spec = spec("{}");
    let plan = WritePlan::new(&spec).unwrap();
    let heads = spec
        .tables()
        .iter()
        .position(|t| t.name == "heads")
        .unwrap();
    let column = |name: &str| ValueSource::Column {
        column: spec.tables()[heads]
            .columns
            .iter()
            .position(|(n, _)| n == name)
            .unwrap(),
    };
    let head = loop_plan(&spec, &plan, "head");
    assert_eq!(head.instances, Instances::Rows { table: heads });
    let code = |value: &str| ValueSource::Code(value.as_bytes().to_vec());
    let element = |element, value| ElementPlan {
        element,
        component: None,
        value,
    };
    let row = |nth| SegmentSource::Row { table: heads, nth };
    let found: Vec<(usize, SegmentSource, Vec<ElementPlan>)> = head
        .segments
        .iter()
        .map(|s| (s.occurrence, s.source, s.elements.clone()))
        .collect();
    assert_eq!(
        found,
        vec![
            (0, row(1), vec![element(1, column("id"))]),
            (
                1,
                row(1),
                vec![
                    element(1, code("A")),
                    element(2, column("a_type")),
                    element(3, column("a_name"))
                ]
            ),
            (
                3,
                row(1),
                vec![
                    element(1, column("first_kind")),
                    element(2, column("first_date"))
                ]
            ),
            (
                3,
                row(2),
                vec![
                    element(1, column("second_kind")),
                    element(2, column("second_date"))
                ]
            ),
            (
                3,
                row(1),
                vec![element(1, code("S")), element(2, column("start"))]
            ),
            (
                4,
                SegmentSource::Repeat {
                    table: spec
                        .tables()
                        .iter()
                        .position(|t| t.name == "adjustments")
                        .unwrap()
                },
                vec![
                    element(1, ValueSource::Column { column: 1 }),
                    element(
                        2,
                        ValueSource::Group {
                            column: 2,
                            offset: 0
                        }
                    ),
                    element(
                        3,
                        ValueSource::Group {
                            column: 0,
                            offset: 1
                        }
                    ),
                ]
            ),
        ]
    );
    let items = spec
        .tables()
        .iter()
        .position(|t| t.name == "items")
        .unwrap();
    assert_eq!(
        loop_plan(&spec, &plan, "group").instances,
        Instances::Groups { table: items }
    );
    assert_eq!(
        loop_plan(&spec, &plan, "env").segments[0].source,
        SegmentSource::Envelope
    );
}

#[test]
fn a_required_element_with_one_code_is_written_with_it() {
    let spec = spec(
        r#"{"segments":{"NM":{"elements":{"2":{"codes":["2"]}}}},
        "tables":{"heads":{"columns":{"a_type":null}}}}"#,
    );
    let plan = WritePlan::new(&spec).unwrap();
    let nm = &loop_plan(&spec, &plan, "head").segments[1];
    assert_eq!(nm.elements[1].value, ValueSource::Code(b"2".to_vec()));
}

#[test]
fn every_refusal_names_the_table_the_column_and_the_occurrence() {
    let cases: [(&str, &str); 10] = [
        (
            r#"{"tables":{"items":null},"loops":{"group":{"usage":"required"}}}"#,
            "loop \"group\" (trigger \"GR\" with no conditions) is required in every \"head\", but no table is anchored on it and no column reads it",
        ),
        (
            r#"{"tables":{"items":{"columns":{"group_number":null}}}}"#,
            "occurrence \"gr\" (\"GR\") is required in every loop \"group\", but no column writes it",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"a_type":null}}}}"#,
            "required element NM02 (entity_type) of occurrence \"nm_a\" in loop \"head\", written by table \"heads\", has no column and no single code to write",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"b_name":{"segment":"NM","element":3}}}}}"#,
            "table \"heads\" column \"b_name\" reads \"NM\" in loop \"head\", which matches 2 of its occurrences (\"nm_a\", \"nm_b\"); name one with \"occurrence\"",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"x":{"segment":"NM","where":{"1":"Z"},"element":3}}}}}"#,
            "table \"heads\" column \"x\" reads \"NM\" where {1: \"Z\"} in loop \"head\", which matches none of its occurrences",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"second_kind":{"pick":3},"second_date":{"pick":3}}}}}"#,
            "table \"heads\" column \"second_date\" picks occurrence \"dt\" of loop \"head\" out of series (picks \"first\", 3); the picks of one occurrence must run from 1 with none missing, or be a lone \"last\"",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"second_kind":{"pick":"last"},"second_date":{"pick":"last"}}}}}"#,
            "table \"heads\" column \"second_date\" picks occurrence \"dt\" of loop \"head\" out of series (picks \"first\", \"last\"); the picks of one occurrence must run from 1 with none missing, or be a lone \"last\"",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"item_code":{"loop":"item","occurrence":"it","element":1}}}}}"#,
            "table \"heads\" column \"item_code\" reads loop \"item\", whose instances table \"items\" writes",
        ),
        (
            r#"{"tables":{"heads":{"columns":{"again":{"occurrence":"nm_a","element":3}}}}}"#,
            "table \"heads\" column \"again\" writes NM03 of occurrence \"nm_a\" in loop \"head\", which column \"a_name\" already writes",
        ),
        (
            r#"{"tables":{"items":{"columns":{"control":{"loop":"env","segment":"EV","element":1}}}}}"#,
            "table \"items\" column \"control\" reads the envelope occurrence \"ev\" of loop \"env\", which the envelope and the writer's counts give",
        ),
    ];
    for (patch, expected) in cases {
        assert_eq!(refusals(patch), vec![expected.to_string()], "{patch}");
    }
}

#[test]
fn a_component_and_an_implied_loop_are_refused_with_what_is_missing() {
    assert_eq!(
        refusals(
            r#"{"tables":{"items":{"columns":{"pair_value":{"segment":"IT","element":2,"component":2}}}}}"#
        ),
        vec![
            "required element IT02-1 (pair_kind) of occurrence \"it\" in loop \"item\", written by table \"items\", has no column and no single code to write"
        ]
    );
    assert_eq!(
        refusals(
            r#"{"tables":{"items":{"columns":{"group_number":null}}},
            "loops":{"group":{"occurrences":{"gr":{"usage":"situational"},
                "gr2":{"segment":"G2","pos":41,"usage":"required"}}}},
            "segments":{"G2":{"elements":{"1":{"name":"g2","type":"AN","required":true}}}}}"#
        ),
        vec![
            "occurrence \"gr2\" (\"G2\") is required in every loop \"group\", but no column writes it"
        ]
    );
}

#[test]
fn an_ambiguous_anchor_segment_is_refused() {
    assert_eq!(
        refusals(
            r#"{"tables":{"heads":{"columns":{"a_type":{"occurrence":"nm_a","element":2}}},
            "names":{"loops":["head"],"segment":"NM","columns":{"kind":{"element":1}}}}}"#
        ),
        vec![
            "table \"names\" anchor segment reads \"NM\" in loop \"head\", which matches 2 of its occurrences (\"nm_a\", \"nm_b\"); name one with \"occurrence\""
        ]
    );
}

#[test]
fn refusals_display_their_full_text() {
    let missing = Refusal::RequiredElementWithoutSource {
        table: None,
        loop_name: "2000".into(),
        occurrence: "header_number".into(),
        place: "LX01".into(),
        name: "assigned_number".into(),
    };
    assert_eq!(
        missing.to_string(),
        "required element LX01 (assigned_number) of occurrence \"header_number\" in loop \"2000\", which no table writes, has no column and no single code to write"
    );
    let written = Refusal::WrittenByAnotherTable {
        table: "claims".into(),
        column: "isa".into(),
        loop_name: "interchange".into(),
        writer: "the envelope".into(),
    };
    assert_eq!(
        written.to_string(),
        "table \"claims\" column \"isa\" reads loop \"interchange\", whose instances the envelope writes"
    );
    let one = PlanError {
        spec: "s".into(),
        refusals: vec![written],
    };
    assert_eq!(
        one.to_string(),
        "spec \"s\" cannot be written (1 reason)\n1. table \"claims\" column \"isa\" reads loop \"interchange\", whose instances the envelope writes"
    );
    assert!(std::error::Error::source(&one).is_none());
}
