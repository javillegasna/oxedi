//! Occurrences: loading, membership derived from them, matching, load-time
//! errors with their full text, and merge patches by occurrence name.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::segs;
use crate::spec::*;

const BASE: &str = r#"{"name":"t",
    "loops":{
        "head":{"trigger":{"segment":"HD"},"max":1,"end":"TR","occurrences":{
            "head":{"segment":"HD","pos":10,"usage":"required","max":1},
            "patient":{"segment":"NM","pos":30,"usage":"required","max":1,
                "qualifier":{"element":1,"codes":["QC"]},"codes":{"2":["1"]}},
            "insured":{"segment":"NM","pos":30,"qualifier":{"element":1,"codes":["IL"]}},
            "note":{"segment":"NT","pos":20,"max":5},
            "id":{"segment":"ID","pos":40,"qualifier":{"element":1,"component":1,"codes":["B","A"]}}
        }},
        "party":{"parent":"head","trigger":{"segment":"N1","where":{"1":"PR"}},"occurrences":{
            "payer":{"segment":"N1","pos":1,"qualifier":{"element":1,"codes":["PR"]}},
            "other":{"segment":"N1","pos":1,"qualifier":{"element":1,"codes":["XX"]}}
        }},
        "bare":{"parent":"head","trigger":{"segment":"BR"}}
    },
    "segments":{
        "NM":{"elements":{
            "1":{"name":"entity","type":"ID","min":2,"max":3},
            "2":{"name":"kind","type":"ID","min":1,"max":1},
            "3":{"name":"amount","type":"R"}
        }},
        "ID":{"elements":{"1":{"name":"ident","type":"AN","composite":{
            "1":{"name":"qualifier","type":"ID"},"2":{"name":"value","type":"AN"}}}}},
        "N1":{"elements":{"1":{"name":"entity","type":"ID"}}}
    }
}"#;

fn base() -> Spec {
    Spec::from_json(BASE).unwrap()
}

fn patched(patch: &str) -> Result<Spec, SpecError> {
    let mut value: Value = serde_json::from_str(BASE).unwrap();
    merge_patch(&mut value, &serde_json::from_str(patch).unwrap());
    Spec::from_json(&value.to_string())
}

fn names(def: &LoopDef) -> Vec<&str> {
    def.occurrences.iter().map(|o| o.name.as_str()).collect()
}

#[test]
fn occurrences_load_in_position_order_with_their_fields() {
    let spec = base();
    let head = spec.get(spec.loop_id("head").unwrap());
    assert_eq!(names(head), ["head", "note", "insured", "patient", "id"]);
    assert_eq!(head.max, Some(1));
    let patient = &head.occurrences[3];
    assert_eq!(patient.segment, b"NM");
    assert_eq!(patient.pos, 30);
    assert_eq!(patient.usage, Usage::Required);
    assert_eq!(patient.max, Some(1));
    assert_eq!(
        patient.qualifier,
        Some(Qualifier {
            element: 1,
            component: None,
            codes: vec!["QC".into()]
        })
    );
    assert_eq!(
        patient.codes,
        BTreeMap::from([((2, None), vec!["1".to_string()])])
    );
    let note = &head.occurrences[1];
    assert_eq!(
        (note.usage, note.max, &note.qualifier),
        (Usage::Situational, Some(5), &None)
    );
    let id = &head.occurrences[4];
    assert_eq!(
        id.qualifier
            .as_ref()
            .map(|q| (q.component, q.codes.clone())),
        Some((Some(1), vec!["A".to_string(), "B".to_string()])),
        "qualifier codes are sorted"
    );
    let party = spec.get(spec.loop_id("party").unwrap());
    assert_eq!(party.max, None);
}

#[test]
fn membership_is_every_occurrence_but_the_ones_the_trigger_opens_on() {
    let spec = base();
    let head = spec.get(spec.loop_id("head").unwrap());
    assert_eq!(
        head.segments,
        [b"NT".to_vec(), b"NM".to_vec(), b"ID".to_vec()]
    );
    assert!(head.accepts(b"TR") && !head.accepts(b"HD"));
    // The trigger opens on `payer`; `other` holds N1 too, so the loop holds N1.
    let party = spec.get(spec.loop_id("party").unwrap());
    assert_eq!(party.segments, [b"N1".to_vec()]);
    let bare = spec.get(spec.loop_id("bare").unwrap());
    assert!(bare.occurrences.is_empty() && bare.segments.is_empty());
}

#[test]
fn an_occurrence_matches_its_segment_and_qualifier() {
    let spec = base();
    let head = spec.get(spec.loop_id("head").unwrap());
    let find = |name: &str| head.occurrences.iter().find(|o| o.name == name).unwrap();
    let input = segs(b"NM*QC*1~NM*IL~NM*ZZ~ID*A:9~ID*C:1~ID*A~NT~NX*QC~");
    let matching = |name: &str| -> Vec<usize> {
        input
            .iter()
            .filter(|s| find(name).matches(s))
            .map(|s| s.index)
            .collect()
    };
    assert_eq!(matching("patient"), [0]);
    assert_eq!(matching("insured"), [1]);
    assert_eq!(
        matching("id"),
        [3, 5],
        "a simple element is its own first component"
    );
    assert_eq!(
        matching("note"),
        [6],
        "no qualifier: the segment id is enough"
    );
}

#[test]
fn the_builtin_loops_hold_the_segments_they_held_before_occurrences() {
    let held = |spec: &Spec, name: &str| -> BTreeSet<String> {
        spec.get(spec.loop_id(name).unwrap())
            .segments
            .iter()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect()
    };
    let set = |ids: &[&str]| -> BTreeSet<String> { ids.iter().map(|s| s.to_string()).collect() };
    let expected = [
        ("interchange", set(&["TA1"])),
        ("group", set(&[])),
        (
            "transaction",
            set(&["BPR", "TRN", "CUR", "REF", "DTM", "PLB"]),
        ),
        ("1000A", set(&["N3", "N4", "REF", "PER"])),
        ("1000B", set(&["N3", "N4", "REF", "RDM"])),
        ("2000", set(&["TS3", "TS2"])),
        (
            "2100",
            set(&[
                "CAS", "NM1", "MIA", "MOA", "REF", "DTM", "PER", "AMT", "QTY",
            ]),
        ),
        ("2110", set(&["DTM", "CAS", "REF", "AMT", "QTY", "LQ"])),
    ];
    let v5010 = Spec::builtin_835();
    let v4010 = Spec::builtin_835_4010();
    for (name, segments) in expected {
        assert_eq!(held(&v5010, name), segments, "5010 {name}");
        let mut segments = segments;
        if name == "1000B" {
            segments.remove("RDM");
        }
        assert_eq!(held(&v4010, name), segments, "4010 {name}");
    }
}

#[test]
fn a_patch_changes_or_removes_one_occurrence_by_name() {
    let spec =
        patched(r#"{"loops":{"head":{"occurrences":{"note":{"max":9},"insured":null}}}}"#).unwrap();
    let head = spec.get(spec.loop_id("head").unwrap());
    assert_eq!(names(head), ["head", "note", "patient", "id"]);
    assert_eq!(head.occurrences[1].max, Some(9));
    assert_eq!(head.occurrences[1].pos, 20, "the other keys stay");
}

#[test]
fn a_patch_replaces_a_qualifier_code_list_whole() {
    let spec =
        patched(r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"codes":["QD"]}}}}}}"#)
            .unwrap();
    let head = spec.get(spec.loop_id("head").unwrap());
    let patient = head
        .occurrences
        .iter()
        .find(|o| o.name == "patient")
        .unwrap();
    assert_eq!(
        patient.qualifier,
        Some(Qualifier {
            element: 1,
            component: None,
            codes: vec!["QD".into()]
        })
    );
}

#[test]
fn invalid_occurrences_are_rejected_naming_loop_occurrence_rule_and_datum() {
    let cases = [
        (
            r#"{"loops":{"head":{"occurrences":{"":{"segment":"ZZ","pos":1}}}}}"#,
            "loop \"head\" occurrence \"\": the occurrence name is empty",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"note":{"usage":"R"}}}}}"#,
            "loop \"head\" occurrence \"note\": \"usage\" must be \"required\" or \"situational\"; found \"R\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"note":{"max":0}}}}}"#,
            "loop \"head\" occurrence \"note\": \"max\" is 0; an occurrence that may appear appears at least once",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"element":4}}}}}}"#,
            "loop \"head\" occurrence \"patient\": qualifier names NM04, which the \"segments\" section does not define for \"NM\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"note":{"qualifier":{"element":1,"codes":["A"]}}}}}}"#,
            "loop \"head\" occurrence \"note\": qualifier names NT01, which the \"segments\" section does not define for \"NT\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"id":{"qualifier":{"component":3}}}}}}"#,
            "loop \"head\" occurrence \"id\": qualifier names ID01-3, which the \"segments\" section does not define for \"ID\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"codes":[]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": qualifier.codes is empty; list at least one code",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"id":{"qualifier":{"component":null}}}}}}"#,
            "loop \"head\" occurrence \"id\": qualifier.codes: \"codes\" applies to a simple element or a component; this element declares a \"composite\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"element":3}}}}}}"#,
            "loop \"head\" occurrence \"patient\": qualifier.codes: \"codes\" applies to non-numeric types; type \"R\" holds numbers, which one value can write several ways (1, 01, 1.0)",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"codes":["QC","QC"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": qualifier.codes: code \"QC\" is listed twice, at codes[0] and codes[1]",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"codes":["QCXX"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": qualifier.codes: code \"QCXX\" at codes[0] has length 4; the element allows 2 to 3",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"codes":{"02":["1"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": codes key \"02\" must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"id":{"codes":{"1-x":["A"]}}}}}}"#,
            "loop \"head\" occurrence \"id\": codes key \"1-x\" must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"codes":{"9":["1"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": codes.9 names NM09, which the \"segments\" section does not define for \"NM\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"codes":{"2":[]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": codes.2 is empty; list at least one code",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"codes":{"1":["QC"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": codes key \"1\" names the qualifier's own place; its codes are the qualifier's \"codes\"",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"codes":{"2":["1","1"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": codes.2: code \"1\" is listed twice, at codes[0] and codes[1]",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"insured":{"qualifier":null}}}}}"#,
            "loop \"head\" occurrence \"patient\": holds segment \"NM\" like occurrence \"insured\", and the two have no qualifier on one shared element to tell them apart",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"insured":{"qualifier":{"element":2,"codes":["1"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": holds segment \"NM\" like occurrence \"insured\", and the two have no qualifier on one shared element to tell them apart",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"insured":{"qualifier":{"codes":["IL","QC"]}}}}}}"#,
            "loop \"head\" occurrence \"patient\": holds segment \"NM\" like occurrence \"insured\", and both qualifiers accept code \"QC\"",
        ),
        (
            r#"{"loops":{"party":{"occurrences":{"payer":{"qualifier":{"codes":["PE"]}}}}}}"#,
            "loop \"party\" opens on \"N1\" where {1: \"PR\"}, but none of its occurrences holds that segment with a qualifier the trigger's conditions select",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"head":null}}}}"#,
            "loop \"head\" opens on \"HD\" with no conditions, but none of its occurrences holds that segment with a qualifier the trigger's conditions select",
        ),
        (
            r#"{"loops":{"head":{"max":0}}}"#,
            "loop \"head\" has \"max\" 0; a loop that may appear appears at least once",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"note":{"maximum":2}}}}}"#,
            "spec: unknown key \"maximum\" at loops.head.occurrences.note",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"extra":{"segment":"EX"}}}}}"#,
            "spec: missing required key \"pos\" at loops.head.occurrences.extra",
        ),
        (
            r#"{"loops":{"head":{"occurrences":{"patient":{"qualifier":{"codes":["QC"],"element":null}}}}}}"#,
            "spec: missing required key \"element\" at loops.head.occurrences.patient.qualifier",
        ),
    ];
    for (patch, expected) in cases {
        let err = patched(patch).unwrap_err();
        assert_eq!(err.to_string(), expected, "{patch}");
    }
}

#[test]
fn occurrence_errors_are_their_own_variants() {
    let err = patched(r#"{"loops":{"head":{"occurrences":{"note":{"max":0}}}}}"#).unwrap_err();
    assert!(matches!(
        &err,
        SpecError::BadOccurrence { loop_name, occurrence, reason }
            if loop_name == "head" && occurrence == "note" && **reason == OccurrenceError::ZeroMax
    ));
    let err = patched(r#"{"loops":{"head":{"occurrences":{"head":null}}}}"#).unwrap_err();
    assert!(matches!(&err, SpecError::UnmatchedTrigger { loop_name, .. } if loop_name == "head"));
    let err = patched(r#"{"loops":{"head":{"max":0}}}"#).unwrap_err();
    assert!(matches!(&err, SpecError::ZeroLoopMax { loop_name } if loop_name == "head"));
    assert!(std::error::Error::source(&err).is_none());
}
