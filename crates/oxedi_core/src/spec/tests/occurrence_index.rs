//! The occurrence a segment takes in a loop, asked of the spec.

use crate::spec::test_support::{nested, segs};
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

/// Two loops: `one` lists a seven-byte id and an eight-byte id sharing its
/// first seven bytes, `two` lists a longer id, and both list `SHAREDLONG`.
const LONG_IDS: &str = r#"{"name":"t","loops":{
    "one":{"trigger":{"segment":"ABCDEFG"},"occurrences":{
        "seven":{"segment":"ABCDEFG","pos":0},
        "eight":{"segment":"ABCDEFGH","pos":1},
        "shared_one":{"segment":"SHAREDLONG","pos":2}}},
    "two":{"trigger":{"segment":"TWOLONGID"},"occurrences":{
        "own":{"segment":"TWOLONGID","pos":0},
        "shared_two":{"segment":"SHAREDLONG","pos":1}}}
}}"#;

#[test]
fn an_id_longer_than_seven_bytes_resolves_in_the_loop_that_lists_it() {
    let spec = Spec::from_json(LONG_IDS).unwrap();
    assert_eq!(taken(&spec, "one", b"ABCDEFGH~"), Some("eight"));
    assert_eq!(taken(&spec, "two", b"TWOLONGID~"), Some("own"));
    assert_eq!(taken(&spec, "one", b"TWOLONGID~"), None);
    assert_eq!(taken(&spec, "two", b"ABCDEFGH~"), None);
    assert_eq!(taken(&spec, "one", b"ABCDEFGHI~"), None);
}

#[test]
fn a_seven_byte_id_and_an_eight_byte_id_sharing_their_bytes_have_their_own_occurrences() {
    let spec = Spec::from_json(LONG_IDS).unwrap();
    assert_eq!(taken(&spec, "one", b"ABCDEFG~"), Some("seven"));
    assert_eq!(taken(&spec, "one", b"ABCDEFGH~"), Some("eight"));
    assert_eq!(taken(&spec, "two", b"ABCDEFG~"), None);
}

#[test]
fn two_loops_listing_the_same_long_id_each_give_their_own_occurrence() {
    let spec = Spec::from_json(LONG_IDS).unwrap();
    assert_eq!(taken(&spec, "one", b"SHAREDLONG~"), Some("shared_one"));
    assert_eq!(taken(&spec, "two", b"SHAREDLONG~"), Some("shared_two"));
}
