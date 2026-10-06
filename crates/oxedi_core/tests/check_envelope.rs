//! Structural diagnostics over every real-shaped file: the known anomalies,
//! rendered in full, and nothing else.

mod common;

use std::collections::BTreeMap;

use oxedi_core::{Document, SnipLevel, Spec};

#[test]
fn the_known_anomalies_are_reported_exactly_and_nothing_else() {
    let spec = Spec::builtin_835();
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, bytes, delims) in common::all_files() {
        let rendered: Vec<String> = common::diagnostics_of(&spec, &bytes, delims)
            .iter()
            .map(ToString::to_string)
            .collect();
        if !rendered.is_empty() {
            found.insert(name, rendered);
        }
    }
    let transaction = "interchange#1/group#1/transaction#1";
    let unknown = |id: &str, index: usize, path: &str| {
        format!(
            "SNIP 1 · segment \"{id}\" is not part of the structure: no open loop holds it and it opens no loop · segment #{index} · at {path} · datum \"{id}\""
        )
    };
    let se01 = |declared: &str, counted: usize, index: usize| {
        format!(
            "SNIP 1 · SE01 declares \"{declared}\" but the count is {counted} · segment #{index}, element 1 · at {transaction} · datum \"{declared}\""
        )
    };
    let expected = BTreeMap::from([
        (
            "blue_cross_nc_sample.txt".to_string(),
            vec![
                "SNIP 1 · loop \"interchange\" opened without its own trigger (\"ISA\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"".to_string(),
                "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"".to_string(),
                se01("33", 32, 31),
            ],
        ),
        (
            "edi835_test_file.RMT".to_string(),
            vec![se01("1202", 76, 77)],
        ),
        (
            "edi835_test_not_available_claim_id.RMT".to_string(),
            vec![se01("302", 255, 256)],
        ),
        (
            "multi_claim_sample.txt".to_string(),
            vec![
                unknown("N3", 19, &format!("{transaction}/2000#1/2100#1")),
                unknown("N4", 20, &format!("{transaction}/2000#1/2100#1")),
                unknown("N3", 34, &format!("{transaction}/2000#2/2100#2")),
                unknown("N4", 35, &format!("{transaction}/2000#2/2100#2")),
            ],
        ),
        (
            "trizetto_sample.rmt".to_string(),
            vec![
                unknown("XX", 7, &format!("{transaction}/1000A#1")),
                se01("15", 18, 19),
            ],
        ),
    ]);
    assert_eq!(found, expected);
}

#[test]
fn every_diagnostic_is_level_one_and_points_at_a_segment_holding_its_datum() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims).unwrap();
        for diagnostic in common::diagnostics_of(&spec, &bytes, delims) {
            assert_eq!(diagnostic.level, SnipLevel::L1, "{name}: {diagnostic}");
            let span = diagnostic
                .span(&document)
                .unwrap_or_else(|| panic!("{name}: {diagnostic} names no segment"));
            let body = &document.as_bytes()[span.body];
            assert!(
                body.windows(diagnostic.datum.len())
                    .any(|window| window == diagnostic.datum.as_slice()),
                "{name}: {diagnostic} points at {:?}",
                String::from_utf8_lossy(body)
            );
        }
    }
}
