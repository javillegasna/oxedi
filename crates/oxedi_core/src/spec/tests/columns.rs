//! Column sources that name an occurrence, read a loop above the anchor or
//! pick a matching segment other than the first.

use crate::spec::*;

const SPEC: &str = r#"{"name":"t",
    "loops":{
        "A":{"trigger":{"segment":"AA"},"occurrences":{
            "aa":{"segment":"AA","pos":0},
            "names":{"segment":"NM","pos":1,"max":3,"qualifier":{"element":1,"codes":["P"]}},
            "other":{"segment":"NM","pos":1,"max":1,"qualifier":{"element":1,"codes":["X"]}},
            "free":{"segment":"FR","pos":2},
            "late":{"segment":"LT","pos":9}
        }},
        "B":{"parent":"A","trigger":{"segment":"BB"},"occurrences":{"bb":{"segment":"BB","pos":3},"names":{"segment":"NM","pos":4}}},
        "C":{"parent":"A","trigger":{"segment":"CC"},"occurrences":{"cc":{"segment":"CC","pos":3},"names":{"segment":"QT","pos":4}}},
        "D":{"parent":"B","trigger":{"segment":"DD"},"occurrences":{"dd":{"segment":"DD","pos":5}}},
        "E":{"parent":"A","trigger":{"segment":"EE"},"occurrences":{"ee":{"segment":"EE","pos":0}}}
    },
    "segments":{"NM":{"elements":{"1":{"name":"kind","type":"ID"},"2":{"name":"name","type":"AN"}}}},
    "tables":TABLES
}"#;

fn load(tables: &str) -> Result<Spec, SpecError> {
    Spec::from_json(&SPEC.replace("TABLES", tables))
}

fn column_error(column: &str) -> (Option<String>, TableDefError) {
    match load(&format!(
        r#"{{"t":{{"loops":["B"],"columns":{{"c":{column}}}}}}}"#
    )) {
        Err(SpecError::BadTable { column, reason, .. }) => (column, reason),
        other => panic!("{column}: {other:?}"),
    }
}

#[test]
fn a_column_names_an_occurrence_of_its_anchor_or_of_a_loop_above_with_a_pick() {
    let spec = load(
        r#"{"t":{"loops":["B"],"columns":{
            "own":{"occurrence":"names","element":2},
            "above":{"loop":"A","occurrence":"names","pick":"last","element":2},
            "third_at":{"loop":"A","occurrence":"names","pick":3,"segment_index":true},
            "itself":{"loop":"B","occurrence":"bb","element":1},
            "where_above":{"loop":"A","segment":"NM","where":{"1":"X"},"element":2}
        }}}"#,
    )
    .unwrap();
    let a = spec.loop_id("A").unwrap();
    let columns = &spec.table("t").unwrap().columns;
    let source = |name: &str| &columns.iter().find(|(n, _)| n == name).unwrap().1;
    assert_eq!(
        source("own"),
        &ColumnSource::Element {
            loop_id: None,
            segment: b"NM".to_vec(),
            conditions: Vec::new(),
            occurrence: Some("names".into()),
            pick: Pick::First,
            element: 2,
            component: None,
        }
    );
    assert_eq!(
        source("above"),
        &ColumnSource::Element {
            loop_id: Some(a),
            segment: b"NM".to_vec(),
            conditions: Vec::new(),
            occurrence: Some("names".into()),
            pick: Pick::Last,
            element: 2,
            component: None,
        }
    );
    assert_eq!(
        source("third_at"),
        &ColumnSource::SegmentIndex {
            loop_id: Some(a),
            segment: b"NM".to_vec(),
            conditions: Vec::new(),
            occurrence: Some("names".into()),
            pick: Pick::Nth(3),
        }
    );
    assert!(
        matches!(
            source("itself"),
            ColumnSource::Element { loop_id: None, .. }
        ),
        "naming the anchor loop reads it as when no loop is named"
    );
    assert!(matches!(
        source("where_above"),
        ColumnSource::Element { loop_id: Some(id), occurrence: None, .. } if *id == a
    ));
}

#[test]
fn a_column_source_by_occurrence_that_cannot_be_read_is_rejected() {
    let cases = [
        (
            r#"{"occurrence":"nope","element":1}"#,
            TableDefError::UnknownOccurrence {
                loop_name: "B".into(),
                occurrence: "nope".into(),
                known: Box::new(["bb".into(), "names".into()]),
            },
        ),
        (
            r#"{"loop":"D","occurrence":"names","element":1}"#,
            TableDefError::UnknownOccurrence {
                loop_name: "D".into(),
                occurrence: "names".into(),
                known: Box::new(["dd".into()]),
            },
        ),
        (
            r#"{"loop":"C","occurrence":"names","element":1}"#,
            TableDefError::UnrelatedLoop {
                loop_name: "C".into(),
                anchor: "B".into(),
            },
        ),
        (
            r#"{"occurrence":"names","segment":"NM","element":1}"#,
            TableDefError::OccurrenceAndSegment {
                key: "segment",
                written: r#""NM""#.into(),
            },
        ),
        (
            r#"{"occurrence":"names","where":{"1":"P"},"element":1}"#,
            TableDefError::OccurrenceAndSegment {
                key: "where",
                written: r#"{"1":"P"}"#.into(),
            },
        ),
        (
            r#"{"segment":"NM","pick":"last","element":1}"#,
            TableDefError::PickNeedsOccurrence {
                written: r#""last""#.into(),
            },
        ),
        (
            r#"{"occurrence":"names","pick":"middle","element":1}"#,
            TableDefError::BadPick {
                written: r#""middle""#.into(),
            },
        ),
        (
            r#"{"occurrence":"names","pick":0,"element":1}"#,
            TableDefError::BadPick {
                written: "0".into(),
            },
        ),
        (
            r#"{"loop":"A","occurrence":"other","pick":"first","element":1}"#,
            TableDefError::PickOnSingle {
                loop_name: "A".into(),
                occurrence: "other".into(),
                pick: Pick::First,
            },
        ),
        (
            r#"{"loop":"A","occurrence":"names","pick":4,"element":1}"#,
            TableDefError::PickBeyondMax {
                nth: 4,
                loop_name: "A".into(),
                occurrence: "names".into(),
                max: 3,
            },
        ),
    ];
    for (column, expected) in cases {
        assert_eq!(
            column_error(column),
            (Some("c".into()), expected),
            "{column}"
        );
    }
}

#[test]
fn an_occurrence_named_without_loop_must_be_one_segment_in_every_anchor() {
    let err =
        load(r#"{"t":{"loops":["B","C"],"columns":{"c":{"occurrence":"names","element":1}}}}"#)
            .unwrap_err();
    assert!(
        matches!(
            &err,
            SpecError::BadTable { reason: TableDefError::OccurrenceSegments { occurrence, segments }, .. }
                if occurrence == "names" && **segments == LoopSegments {
                    first_loop: "B".into(),
                    first_segment: "NM".into(),
                    loop_name: "C".into(),
                    segment: "QT".into(),
                }
        ),
        "{err:?}"
    );
}

#[test]
fn a_table_anchored_on_a_segment_takes_no_occurrence_or_pick() {
    for (column, key, written) in [
        (
            r#"{"occurrence":"names","element":1}"#,
            "occurrence",
            r#""names""#,
        ),
        (r#"{"pick":2,"element":1}"#, "pick", "2"),
    ] {
        let err = load(&format!(
            r#"{{"t":{{"loops":["A"],"segment":"NM","columns":{{"c":{column}}}}}}}"#
        ))
        .unwrap_err();
        assert!(
            matches!(
                &err,
                SpecError::BadTable { reason: TableDefError::AnchorSegmentOnly { key: k, anchor_segment, written: w }, .. }
                    if *k == key && anchor_segment == "NM" && w == written
            ),
            "{column}: {err:?}"
        );
    }
}

#[test]
fn a_pick_displays_as_a_spec_writes_it() {
    assert_eq!(Pick::First.to_string(), "\"first\"");
    assert_eq!(Pick::Last.to_string(), "\"last\"");
    assert_eq!(Pick::Nth(3).to_string(), "3");
    assert_eq!(Pick::default(), Pick::First);
}

#[test]
fn a_column_above_the_anchor_at_segments_after_the_anchor_opens_is_rejected() {
    let after = TableDefError::AfterAnchor {
        occurrence: "late".into(),
        positions: Box::new(AnchorPositions {
            loop_name: "A".into(),
            pos: 9,
            anchor: "B".into(),
            anchor_pos: 3,
        }),
    };
    for column in [
        r#"{"loop":"A","occurrence":"late","element":1}"#,
        r#"{"loop":"A","segment":"LT","segment_index":true}"#,
    ] {
        assert_eq!(
            column_error(column),
            (Some("c".into()), after.clone()),
            "{column}"
        );
    }
    let spec = load(
        r#"{"t":{"loops":["E"],"columns":{"c":{"loop":"A","occurrence":"late","element":1}}}}"#,
    );
    assert!(
        spec.is_ok(),
        "a loop numbering its own positions is not compared: {spec:?}"
    );
}

#[test]
fn the_built_in_claims_cannot_read_the_transaction_after_the_claims() {
    let err = Spec::builtin_835()
        .merge_patch(
            r#"{"tables":{"claims":{"columns":{"plb":{"loop":"transaction","occurrence":"provider_adjustment","element":1}}}}}"#,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "applying patch: table \"claims\" column \"plb\": occurrence \"provider_adjustment\" (position 30100) of loop \"transaction\" comes after anchor loop \"2100\" opens (position 20100): its segments arrive after the rows are appended, so the column never fills"
    );
}
