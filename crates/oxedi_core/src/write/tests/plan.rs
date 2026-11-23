//! The write plan over the built-in specs and over a small spec whose
//! tables are patched to raise each refusal.

use crate::spec::Spec;
use crate::spec::test_support::{builtin, loaded, patched};
use crate::spec_load::Version;
use crate::write::*;

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
    patched(&loaded(BASE), patch).unwrap()
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
    // 4010 has no PER*BL: its technical contact columns write nothing.
    let contact: &[&str] = &[
        "payer_technical_contact_name",
        "payer_technical_contact_number",
        "payer_technical_contact_qualifier",
    ];
    for (spec, expected) in [
        (Spec::builtin_835(), &[][..]),
        (Spec::builtin_835_4010(), contact),
    ] {
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
        let unwritten: Vec<&str> = plan
            .unwritten
            .iter()
            .map(|u| spec.tables()[u.table].columns[u.column].0.as_str())
            .collect();
        assert_eq!(unwritten, expected);
        for u in &plan.unwritten {
            assert_eq!((u.place.as_str(), u.code.as_str()), ("PER01", "BL"));
            assert_eq!(u.codes, vec!["CX".to_string()]);
        }
    }
}

#[test]
fn a_where_code_the_segment_excludes_writes_nothing() {
    let spec = spec(
        r#"{"segments":{"DT":{"elements":{"1":{"codes":["S","X"]}}}},
        "tables":{"heads":{"columns":{"q_date":{"segment":"DT","where":{"1":"Q"},"element":2}}}}}"#,
    );
    let plan = WritePlan::new(&spec).unwrap();
    let unwritten = &plan.unwritten[0];
    let heads = &spec.tables()[unwritten.table];
    assert_eq!(plan.unwritten.len(), 1);
    assert_eq!(heads.columns[unwritten.column].0, "q_date");
    assert_eq!(
        (unwritten.place.as_str(), unwritten.code.as_str()),
        ("DT01", "Q")
    );
    assert_eq!(unwritten.codes, vec!["S".to_string(), "X".to_string()]);
}

#[test]
fn a_where_code_outside_the_occurrence_list_selects_nothing() {
    // X is in the element's list but not in the occurrence's own list, so
    // the rule at that place refuses it; with the element lists left out no
    // occurrence takes it either, so it is not an excluded code.
    let found = refusals(
        r#"{"segments":{"DT":{"elements":{"1":{"codes":["S","X"]}}}},
        "loops":{"head":{"occurrences":{"dt":{"codes":{"1":["S"]}}}}},
        "tables":{"heads":{"columns":{"x_date":{"segment":"DT","where":{"1":"X"},"element":2}}}}}"#,
    );
    assert!(found.iter().any(|text| text
        == r#"table "heads" column "x_date" reads "DT" where {1: "X"} in loop "head", which matches none of its occurrences"#));
    assert!(
        found
            .iter()
            .all(|text| !text.contains("with the fixed code"))
    );
}

#[test]
fn a_table_anchored_on_several_loops_is_refused() {
    // Sibling anchors the loader accepts; the writer cannot tell them apart.
    assert_eq!(
        refusals(
            r#"{"loops":{"other":{"parent":"env","trigger":{"segment":"OT"},
                "occurrences":{"ot":{"segment":"OT","pos":1,"usage":"required"},
                    "hd":{"segment":"HD","pos":2}}}},
            "segments":{"OT":{"elements":{"1":{"name":"id","type":"AN"}}}},
            "tables":{"heads":{"loops":["head","other"],"columns":{
                "a_type":null,"a_name":null,"start":null,"first_kind":null,"first_date":null,
                "second_kind":null,"second_date":null}},"adjustments":null}}"#
        ),
        vec![
            "table \"heads\" is anchored on 2 loops (\"head\", \"other\"); a written row opens one loop instance and nothing in the row says which",
            // Its columns are not placed, so what they would write is missing.
            "occurrence \"hd\" (\"HD\") is required in every loop \"head\", but no column writes it"
        ]
    );
}

#[test]
fn a_segment_table_in_a_loop_nothing_writes_is_refused() {
    assert_eq!(
        refusals(
            r#"{"tables":{"items":null,"marks":{"loops":["group"],"segment":"GR","columns":{"number":{"element":1}}}}}"#
        ),
        vec![
            "table \"marks\" writes one \"GR\" per row in loop \"group\", but no table is anchored on that loop and no column reads it, so the segments have no instance to go in"
        ]
    );
}

#[test]
fn a_fixed_code_outside_the_element_codes_is_refused() {
    // The loader refuses such a qualifier; the plan refuses it too.
    let mut spec = spec("{}");
    let (_, loops) = spec.parts_mut();
    let head = loops.iter_mut().find(|l| l.name == "head").unwrap();
    let nm_a = head
        .occurrences
        .iter_mut()
        .find(|o| o.name == "nm_a")
        .unwrap();
    nm_a.qualifier.as_mut().unwrap().codes = vec!["Q".into()];
    let error = WritePlan::new(&spec).unwrap_err();
    assert_eq!(
        error
            .refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![
            "table \"heads\" column \"a_name\" writes occurrence \"nm_a\" in loop \"head\" with the fixed code \"Q\" in NM01, which its code list (\"A\", \"B\", \"C\") does not allow"
        ]
    );
}

#[test]
fn a_second_table_anchored_on_a_loop_is_refused() {
    // The loader refuses two tables on one anchor; the plan refuses them too.
    let mut spec = spec("{}");
    let (tables, _) = spec.parts_mut();
    let mut others = tables.iter().find(|t| t.name == "heads").unwrap().clone();
    others.name = "others".into();
    tables.push(others);
    let error = WritePlan::new(&spec).unwrap_err();
    assert_eq!(
        error
            .refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![
            "table \"others\" is anchored on loop \"head\", whose rows table \"heads\" already gives; a loop takes its rows from one table"
        ]
    );
}

#[test]
fn the_envelope_segment_is_found_by_the_loop_trigger() {
    // The loader puts the trigger occurrence first; the plan finds it by the
    // trigger wherever it is.
    let moved = |patch: &str| {
        let mut spec = spec(patch);
        let (_, loops) = spec.parts_mut();
        let env = loops.iter_mut().find(|l| l.name == "env").unwrap();
        let mut other = env.occurrences[0].clone();
        other.name = "ex".into();
        other.segment = b"EX".to_vec();
        other.usage = crate::spec::Usage::Situational;
        env.occurrences.insert(0, other);
        spec
    };
    let plain = moved("{}");
    let plan = WritePlan::new(&plain).unwrap();
    let env = loop_plan(&plain, &plan, "env");
    assert_eq!(env.segments.len(), 1);
    assert_eq!(env.segments[0].occurrence, 1);
    let error = WritePlan::new(&moved(
        r#"{"tables":{"items":{"columns":{"control":{"loop":"env","segment":"EV","element":1}}}}}"#,
    ))
    .unwrap_err();
    assert_eq!(
        error
            .refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![
            "table \"items\" column \"control\" reads the envelope occurrence \"ev\" of loop \"env\", which the envelope and the writer's counts give"
        ]
    );
}

#[test]
fn the_built_in_tables_without_their_writing_columns_name_what_is_missing() {
    let spec = patched(&builtin(Version::V5010),
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

/// The refusals of the `items` table with `columns` (JSON) on IT.
fn item_refusals(columns: &str) -> Vec<String> {
    refusals(&format!(
        r#"{{"tables":{{"items":{{"columns":{columns}}}}}}}"#
    ))
}

/// A column of IT02 as whole, or of one of its components.
fn pair(name: &str, component: Option<usize>) -> String {
    let component = component.map_or(String::new(), |c| format!(r#","component":{c}"#));
    format!(r#""{name}":{{"segment":"IT","element":2{component}}}"#)
}

#[test]
fn a_component_column_after_a_whole_element_column_is_refused() {
    let columns = format!("{{{},{}}}", pair("a_pair", None), pair("b_kind", Some(1)));
    assert_eq!(
        item_refusals(&columns),
        vec![
            "table \"items\" column \"b_kind\" writes IT02-1 of occurrence \"it\" in loop \"item\", which column \"a_pair\" already writes"
        ]
    );
}

#[test]
fn a_whole_element_column_after_a_component_column_is_refused() {
    let columns = format!("{{{},{}}}", pair("a_kind", Some(1)), pair("b_pair", None));
    assert_eq!(
        item_refusals(&columns),
        vec![
            "table \"items\" column \"b_pair\" writes IT02 of occurrence \"it\" in loop \"item\", which column \"a_kind\" already writes"
        ]
    );
}

#[test]
fn two_components_of_one_element_are_accepted() {
    let columns = format!("{{{},{}}}", pair("kind", Some(1)), pair("value", Some(2)));
    assert!(item_refusals(&columns).is_empty());
}

/// The elements at IT02 of the only IT segment of loop `item`, after patching
/// the spec with `columns` and `segments` (JSON).
fn item_elements_at_two(columns: &str, segments: &str) -> Vec<ElementPlan> {
    let spec = spec(&format!(
        r#"{{"tables":{{"items":{{"columns":{columns}}}}},"segments":{segments}}}"#
    ));
    let plan = WritePlan::new(&spec).unwrap();
    let it = &loop_plan(&spec, &plan, "item").segments[0];
    it.elements
        .iter()
        .filter(|e| e.element == 2)
        .cloned()
        .collect()
}

/// IT02-2 is the required component, with the given codes.
fn pair_value_required(codes: &str) -> String {
    format!(
        r#"{{"IT":{{"elements":{{"2":{{"composite":{{"2":{{"required":true,"codes":{codes}}}}}}}}}}}}}"#
    )
}

#[test]
fn a_whole_element_column_covers_a_required_component() {
    let columns = format!("{{{}}}", pair("a_pair", None));
    assert!(item_refusals(&columns).is_empty());
}

#[test]
fn a_single_code_is_not_added_beside_a_whole_element_column() {
    let columns = format!("{{{}}}", pair("a_pair", None));
    let at_two = item_elements_at_two(&columns, &pair_value_required(r#"["V"]"#));
    assert_eq!(at_two.len(), 1, "{at_two:#?}");
    assert_eq!(at_two[0].component, None);
    assert!(matches!(at_two[0].value, ValueSource::Column { .. }));
}

#[test]
fn a_component_column_leaves_the_other_required_components_to_be_filled() {
    let columns = format!("{{{}}}", pair("a_kind", Some(1)));
    let at_two = item_elements_at_two(&columns, &pair_value_required(r#"["V"]"#));
    let places: Vec<_> = at_two.iter().map(|e| e.component).collect();
    assert_eq!(places, [Some(1), Some(2)], "{at_two:#?}");
    assert_eq!(at_two[1].value, ValueSource::Code(b"V".to_vec()));
}

/// The elements at IT02 of the segment of occurrence `it2` of loop `head`, an
/// IT occurrence whose qualifier is a code on component 1 of IT02, after
/// patching the spec with `columns`; no refusal is expected.
fn qualified_item_elements(columns: &str) -> Vec<ElementPlan> {
    let patch = format!(
        r#"{{"loops":{{"head":{{"occurrences":{{"it2":{{"segment":"IT","pos":35,
            "qualifier":{{"element":2,"component":1,"codes":["K"]}}}}}}}}}},
            "tables":{{"heads":{{"columns":{columns}}}}}}}"#
    );
    let spec = spec(&patch);
    let plan = WritePlan::new(&spec).unwrap();
    let occurrences = &spec.get(spec.loop_id("head").unwrap()).occurrences;
    let segments = &loop_plan(&spec, &plan, "head").segments;
    let it2 = segments
        .iter()
        .find(|s| occurrences[s.occurrence].name == "it2")
        .unwrap();
    let at_two = it2.elements.iter().filter(|e| e.element == 2);
    at_two.cloned().collect()
}

#[test]
fn a_column_replaces_the_qualifier_code_on_a_place_it_overlaps() {
    let at_two = qualified_item_elements(
        r#"{"k_code":{"occurrence":"it2","element":1},"k_pair":{"occurrence":"it2","element":2}}"#,
    );
    assert_eq!(at_two.len(), 1, "{at_two:#?}");
    assert_eq!(at_two[0].component, None);
    assert!(matches!(at_two[0].value, ValueSource::Column { .. }));
}

#[test]
fn a_qualifier_code_is_not_added_over_a_where_code_on_the_whole_element() {
    let at_two =
        qualified_item_elements(r#"{"k_code":{"segment":"IT","where":{"2":"X"},"element":1}}"#);
    assert_eq!(at_two.len(), 1, "{at_two:#?}");
    assert_eq!(at_two[0].component, None);
    assert_eq!(at_two[0].value, ValueSource::Code(b"X".to_vec()));
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
    let anchored = Refusal::CodeOutsideList {
        table: "adjustments".into(),
        column: None,
        loop_name: "2100".into(),
        occurrence: "claim_adjustment".into(),
        place: "CAS01".into(),
        value: "XX".into(),
        codes: vec!["CO".into(), "PR".into()],
    };
    assert_eq!(
        anchored.to_string(),
        "table \"adjustments\" anchor segment writes occurrence \"claim_adjustment\" in loop \"2100\" with the fixed code \"XX\" in CAS01, which its code list (\"CO\", \"PR\") does not allow"
    );
}
