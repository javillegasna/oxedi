//! The type and target compiled for each declared column.

use crate::spec::test_support::builtin;
use std::collections::BTreeMap;

use crate::spec::{
    ColumnPlace, ColumnSource, OccurrencePlace, Reading, Relation, Spec, Target, table_columns,
};
use crate::spec_load::Version;
use crate::value::ColumnType;

use super::compat;

/// The place of column `column` of table `table`, by name.
fn place<'s>(spec: &'s Spec, table: &str, column: &str) -> &'s ColumnPlace {
    let at = spec
        .tables()
        .iter()
        .position(|def| def.name == table)
        .unwrap_or_else(|| panic!("no table {table}"));
    let name = spec.tables()[at]
        .columns
        .iter()
        .position(|(name, _)| name == column)
        .unwrap_or_else(|| panic!("no column {table}.{column}"));
    &spec.places(at)[name]
}

#[test]
fn each_column_place_has_its_schema_type() {
    let compat = compat();
    let five = builtin(Version::V5010);
    let four = builtin(Version::V4010);
    for spec in [&five.spec, &four.spec, &compat.spec] {
        for (at, def) in spec.tables().iter().enumerate() {
            let kinds: Vec<ColumnType> = spec.places(at).iter().map(|p| p.kind).collect();
            let schema: Vec<ColumnType> = table_columns(spec, def)
                .into_iter()
                .skip(2 + def.ancestors.len())
                .map(|(_, kind)| kind)
                .collect();
            assert_eq!(kinds, schema, "table {}", def.name);
        }
    }
}

#[test]
fn a_column_place_names_what_of_the_segment_it_reads() {
    let five = builtin(Version::V5010);
    let compat = compat();
    let code = place(&five.spec, "services", "procedure_code");
    assert_eq!(
        code.target,
        Target::Element {
            element: 1,
            component: Some(2)
        }
    );
    let amount = place(&five.spec, "adjustments", "amount");
    assert_eq!(
        amount.target,
        Target::Group {
            offset: 1,
            component: None
        }
    );
    assert_eq!(
        amount.kind,
        ColumnType::Decimal128 {
            precision: 38,
            scale: 2
        }
    );
    let index = place(&compat.spec, "rows", "service_date_segment");
    assert_eq!(index.target, Target::Index);
    assert_eq!(index.kind, ColumnType::Int64 { scale: 0 });
    assert_eq!(
        place(&five.spec, "claims", "header_number").kind,
        ColumnType::Int64 { scale: 0 }
    );
}

/// A claim loop with two dated occurrences told apart by their first element.
const PLACES: &str = r#"{"name":"t","loops":{
  "claim":{"trigger":{"segment":"CL"},"occurrences":{
    "cl":{"segment":"CL","pos":0},
    "start":{"segment":"DT","pos":1,"qualifier":{"element":1,"codes":["150"]}},
    "end":{"segment":"DT","pos":2,"qualifier":{"element":1,"codes":["151","152"]}}}},
  "note":{"parent":"claim","trigger":{"segment":"NT"}}},
 "segments":{"CL":{"elements":{"1":{"name":"id","type":"AN"}}},
   "DT":{"elements":{"1":{"name":"qualifier","type":"ID"},"2":{"name":"date","type":"DT"}}},
   "NT":{"elements":{"1":{"name":"text","type":"AN"}}}},
 "tables":{"claims":{"loops":["claim"],"columns":{
   "id":{"segment":"CL","element":1},
   "start":{"occurrence":"start","element":2},
   "end":{"segment":"DT","where":{"1":"152"},"element":2},
   "any_date":{"segment":"DT","element":2},
   "by_date":{"segment":"DT","where":{"2":"20240101"},"element":1},
   "other":{"segment":"DT","where":{"1":"999"},"element":2},
   "note":{"loop":"note","segment":"NT","element":1}}}}}"#;

/// The relation of a column and, per read, the loop and what it reads there.
fn reads(spec: &Spec, table: &str, column: &str) -> (String, Vec<(String, String)>) {
    let Reading::Watch { relation, reads } = &place(spec, table, column).reading else {
        return ("Anchor".to_string(), Vec::new());
    };
    let reads = reads
        .iter()
        .map(|read| {
            let loop_def = spec.get(read.loop_id);
            let name =
                |id: crate::spec::OccurrenceId| loop_def.occurrences[id.index()].name.clone();
            let what = match &read.place {
                OccurrencePlace::Named(id) => format!("named {}", name(*id)),
                OccurrencePlace::Chosen(id) => format!("chosen {}", name(*id)),
                OccurrencePlace::Unplaced(why) => format!("unplaced {why:?}"),
            };
            (loop_def.name.clone(), what)
        })
        .collect();
    (format!("{relation:?}"), reads)
}

fn read(loop_name: &str, what: &str) -> (String, String) {
    (loop_name.to_string(), what.to_string())
}

#[test]
fn a_column_naming_an_occurrence_reads_it_by_name() {
    let spec = Spec::from_json(PLACES).unwrap();
    assert_eq!(
        reads(&spec, "claims", "start"),
        ("Anchor".into(), vec![read("claim", "named start")])
    );
}

#[test]
fn a_columns_segment_and_codes_choose_one_occurrence() {
    let spec = Spec::from_json(PLACES).unwrap();
    assert_eq!(
        reads(&spec, "claims", "id").1,
        vec![read("claim", "chosen cl")]
    );
    assert_eq!(
        reads(&spec, "claims", "end").1,
        vec![read("claim", "chosen end")]
    );
    let five = builtin(Version::V5010);
    assert_eq!(
        reads(&five.spec, "services", "allowed_amount").1,
        vec![read("2110", "chosen service_supplemental_amount")]
    );
    let four = builtin(Version::V4010);
    assert_eq!(
        reads(&four.spec, "payments", "payer_technical_contact_name"),
        (
            "Inside".into(),
            vec![read("1000A", "chosen payer_business_contact_information")]
        )
    );
}

#[test]
fn a_column_no_one_occurrence_takes_keeps_its_conditions() {
    let spec = Spec::from_json(PLACES).unwrap();
    for column in ["any_date", "by_date"] {
        assert_eq!(
            reads(&spec, "claims", column).1,
            vec![read("claim", "unplaced QualifierFree")]
        );
    }
    assert_eq!(
        reads(&spec, "claims", "other").1,
        vec![read("claim", "unplaced NoOccurrenceTakes")]
    );
    assert_eq!(
        reads(&spec, "claims", "note"),
        ("Inside".into(), vec![read("note", "unplaced NoOccurrence")])
    );
}

#[test]
fn a_column_above_its_anchor_has_a_carried_slot() {
    let five = builtin(Version::V5010);
    assert_eq!(
        reads(&five.spec, "claims", "header_number"),
        (
            "Above { carried: 0 }".into(),
            vec![read("2000", "named header_number")]
        )
    );
}

#[test]
fn a_column_of_two_anchor_loops_reads_each_loops_occurrence() {
    let compat = compat();
    assert_eq!(
        reads(&compat.spec, "rows_organizations", "name"),
        (
            "Anchor".into(),
            vec![
                read("1000A", "chosen payer_identification"),
                read("1000B", "chosen payee_identification"),
            ]
        )
    );
}

#[test]
fn a_column_of_a_table_anchored_on_a_segment_reads_the_anchor() {
    let five = builtin(Version::V5010);
    let at = five
        .spec
        .tables()
        .iter()
        .position(|def| def.name == "adjustments")
        .unwrap();
    assert!(!five.spec.places(at).is_empty());
    for place in five.spec.places(at) {
        assert_eq!(place.reading, Reading::Anchor);
    }
}

/// How many columns and reads of `spec` fall in each class.
fn classes(spec: &Spec) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let mut count = |class: &'static str| *counts.entry(class).or_insert(0) += 1;
    for (table, def) in spec.tables().iter().enumerate() {
        for (column, place) in spec.places(table).iter().enumerate() {
            let Reading::Watch { relation, reads } = &place.reading else {
                count("anchor");
                continue;
            };
            count(match relation {
                Relation::Anchor => "relation anchor",
                Relation::Inside => "relation inside",
                Relation::Above { .. } => "relation above",
            });
            let by_where = match &def.columns[column].1 {
                ColumnSource::Element { conditions, .. }
                | ColumnSource::SegmentIndex { conditions, .. } => !conditions.is_empty(),
                ColumnSource::GroupElement { .. } => false,
            };
            for read in reads {
                count(match (&read.place, by_where) {
                    (OccurrencePlace::Named(_), _) => "named",
                    (OccurrencePlace::Chosen(_), false) => "chosen by segment",
                    (OccurrencePlace::Chosen(_), true) => "chosen by where",
                    (OccurrencePlace::Unplaced(_), _) => "unplaced",
                });
            }
        }
    }
    counts
}

/// The number of columns and reads of each class in each built-in spec; a
/// change means a column changed class.
#[test]
fn every_builtin_column_reads_by_its_class() {
    let built = [builtin(Version::V5010), builtin(Version::V4010)];
    let builtin_counts = BTreeMap::from([
        ("anchor", 9),
        ("chosen by segment", 26),
        ("chosen by where", 11),
        ("named", 24),
        ("relation above", 1),
        ("relation anchor", 43),
        ("relation inside", 17),
    ]);
    for loaded in &built {
        assert_eq!(classes(&loaded.spec), builtin_counts);
    }
    assert_eq!(
        classes(&compat().spec),
        BTreeMap::from([
            ("anchor", 36),
            ("chosen by segment", 43),
            ("chosen by where", 7),
            ("relation anchor", 39),
            ("relation inside", 2),
        ])
    );
}
