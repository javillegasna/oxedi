//! Columns that name an occurrence, read a loop above the anchor or pick a
//! matching segment other than the first.

use super::tests::{delimiters, project, rendered, rows};
use crate::spec::Spec;

const SPEC: &str = r#"{"name":"t",
    "loops":{
        "head":{"trigger":{"segment":"HD"},"end":"TR","occurrences":{
            "hd":{"segment":"HD","pos":0},
            "name":{"segment":"NM","pos":1,"max":3,"qualifier":{"element":1,"codes":["P","Q"]}},
            "other":{"segment":"NM","pos":1,"qualifier":{"element":1,"codes":["X"]}}
        }},
        "claim":{"parent":"head","trigger":{"segment":"CL"},"occurrences":{
            "cl":{"segment":"CL","pos":2,"max":1},
            "ref":{"segment":"RF","pos":3,"max":5}
        }},
        "line":{"parent":"claim","trigger":{"segment":"LN"},"occurrences":{
            "ln":{"segment":"LN","pos":4},
            "ref":{"segment":"RF","pos":5}
        }}
    },
    "segments":{
        "NM":{"elements":{"1":{"name":"kind","type":"ID"},"2":{"name":"name","type":"AN"}}},
        "RF":{"elements":{"1":{"name":"kind","type":"ID"},"2":{"name":"value","type":"AN"}}}
    },
    "tables":{
        "claims":{"loops":["claim"],"ref":"claim","columns":{
            "id":{"segment":"CL","element":1},
            "payer_first":{"loop":"head","occurrence":"name","element":2},
            "payer_last":{"loop":"head","occurrence":"name","pick":"last","element":2},
            "payer_second":{"loop":"head","occurrence":"name","pick":2,"element":2},
            "payer_second_at":{"loop":"head","occurrence":"name","pick":2,"segment_index":true},
            "ref_last":{"occurrence":"ref","pick":"last","element":2},
            "ref_second":{"occurrence":"ref","pick":2,"element":2},
            "line_ref_last":{"loop":"line","occurrence":"ref","pick":"last","element":2}
        }},
        "lines":{"loops":["line"],"ref":"line","columns":{
            "code":{"segment":"LN","element":1},
            "claim_id":{"loop":"claim","occurrence":"cl","element":1},
            "claim_ref":{"loop":"claim","occurrence":"ref","pick":"last","element":2},
            "head_name":{"loop":"head","segment":"NM","where":{"1":"Q"},"element":2}
        }}
    }
}"#;

const INPUT: &str = "HD*B1~NM*P*ACME~NM*X*OTHER~NM*Q*BETA~CL*C1~RF*A*R1~RF*B*R2~RF*C*R3~LN*L1~RF*L*LR1~RF*L*LR2~LN*L2~CL*C2~LN*L3~TR~HD*B2~CL*C3~TR~";

#[test]
fn a_column_reads_the_occurrence_it_names_with_its_pick() {
    let spec = Spec::from_json(SPEC).unwrap();
    let (tables, diagnostics) = project(&spec, INPUT);
    assert_eq!(rendered(&diagnostics), Vec::<String>::new());
    assert_eq!(
        rows(&tables, "claims"),
        vec![
            "row | segment | id | line_ref_last | payer_first | payer_last | payer_second | payer_second_at | ref_last | ref_second",
            "0 | 4 | C1 | LR2 | ACME | BETA | BETA | 3 | R3 | R2",
            "1 | 12 | C2 | ∅ | ACME | BETA | BETA | 3 | ∅ | ∅",
            "2 | 16 | C3 | ∅ | ∅ | ∅ | ∅ | ∅ | ∅ | ∅",
        ],
        "the payer columns read the head instance above each claim; the other NM never counts as a name"
    );
}

#[test]
fn a_column_reads_a_loop_above_the_anchor_as_it_stood_when_the_row_opened() {
    let spec = Spec::from_json(SPEC).unwrap();
    let (tables, _) = project(&spec, INPUT);
    assert_eq!(
        rows(&tables, "lines"),
        vec![
            "row | segment | claim | claim_id | claim_ref | code | head_name",
            "0 | 8 | 0 | C1 | R3 | L1 | BETA",
            "1 | 11 | 0 | C1 | R3 | L2 | BETA",
            "2 | 13 | 1 | C2 | ∅ | L3 | BETA",
        ]
    );
}

#[test]
fn a_value_read_above_the_anchor_starts_over_with_each_instance_of_its_loop() {
    let spec = Spec::from_json(SPEC).unwrap();
    let (tables, _) = project(&spec, "HD*B1~CL*C1~RF*A*R1~LN*L1~CL*C2~LN*L2~RF*Z*LATE~TR~");
    assert_eq!(
        rows(&tables, "lines"),
        vec![
            "row | segment | claim | claim_id | claim_ref | code | head_name",
            "0 | 3 | 0 | C1 | R1 | L1 | ∅",
            "1 | 5 | 1 | C2 | ∅ | L2 | ∅",
        ],
        "C2 has no RF before its line, and an RF of the line is not the claim's"
    );
}

#[test]
fn values_read_above_the_anchor_do_not_outlive_finish() {
    let spec = Spec::from_json(SPEC).unwrap();
    let mut engine = crate::LoopEngine::new(&spec);
    let mut projector = super::Projector::new(&spec, &delimiters());
    let feed =
        |engine: &mut crate::LoopEngine<'_>, projector: &mut super::Projector<'_>, input: &str| {
            for segment in crate::Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
                let events = engine.feed(&segment);
                projector.on(&segment, events);
            }
        };
    feed(&mut engine, &mut projector, "HD*B1~NM*P*ACME~CL*C1~");
    engine.finish();
    projector.finish();
    feed(&mut engine, &mut projector, "CL*C2~");
    engine.finish();
    projector.finish();
    let tables = projector.take_tables();
    let claims = rows(&tables, "claims");
    assert!(claims[1].contains("| C1 | ∅ | ACME |"), "{claims:?}");
    assert!(
        claims[2].contains("| C2 | ∅ | ∅ |"),
        "a claim opened without its head in a new stream sees no payer: {claims:?}"
    );
}
