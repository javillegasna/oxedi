//! The occurrence a segment takes in a loop, asked of the spec.

use super::segs;
use crate::spec::test_support::nested;
use crate::spec::*;

/// The name of the occurrence `input` (one segment) takes in loop `loop_name`.
fn taken<'s>(spec: &'s Spec, loop_name: &str, input: &[u8]) -> Option<&'s str> {
    let loop_id = spec.loop_id(loop_name).unwrap();
    let segments = segs(input);
    spec.occurrence_of(loop_id, &segments[0])
        .map(|found| spec.get(loop_id).occurrences[found.index()].name.as_str())
}

#[test]
fn a_qualified_occurrence_is_found_by_its_qualifier() {
    let spec = Spec::builtin_835();
    assert_eq!(taken(&spec, "2100", b"NM1*QC~"), Some("patient_name"));
    assert_eq!(taken(&spec, "2100", b"NM1*IL~"), Some("insured_name"));
}

#[test]
fn an_unqualified_occurrence_matches_any_segment_of_its_id() {
    let spec = nested();
    assert_eq!(taken(&spec, "A", b"A1*whatever~"), Some("a1"));
}

#[test]
fn a_segment_no_qualifier_holds_has_no_occurrence() {
    let spec = Spec::builtin_835();
    assert_eq!(taken(&spec, "2100", b"NM1*ZZ~"), None);
}

#[test]
fn a_segment_id_is_resolved_in_the_given_loop() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "one":{"trigger":{"segment":"OA"},"occurrences":{"oa":{"segment":"OA","pos":0},"shared_one":{"segment":"SH","pos":1}}},
            "two":{"trigger":{"segment":"TA"},"occurrences":{"ta":{"segment":"TA","pos":0},"shared_two":{"segment":"SH","pos":1}}},
            "three":{"trigger":{"segment":"HA"},"occurrences":{"ha":{"segment":"HA","pos":0}}}
        }}"#,
    )
    .unwrap();
    assert_eq!(taken(&spec, "one", b"SH~"), Some("shared_one"));
    assert_eq!(taken(&spec, "two", b"SH~"), Some("shared_two"));
    assert_eq!(taken(&spec, "three", b"SH~"), None);
}

#[test]
fn the_index_follows_a_merge_patch() {
    let dtm = |spec: &Spec| taken(spec, "2100", b"DTM*050*20200101~").map(str::to_owned);
    assert_eq!(
        dtm(&Spec::builtin_835()).as_deref(),
        Some("claim_received_date")
    );
    assert_eq!(
        dtm(&Spec::builtin_835_4010()).as_deref(),
        Some("claim_date")
    );
    assert_eq!(
        taken(&Spec::builtin_835_4010(), "2100", b"NM1*GB~"),
        None,
        "the patch removes other_subscriber_name"
    );
}
