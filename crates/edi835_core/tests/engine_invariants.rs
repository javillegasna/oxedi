//! Structural invariants of the engine over every real-shaped file.

mod common;

use std::collections::BTreeMap;

use edi835_core::{Event, LoopTree, Spec, Tokenizer};

#[test]
fn every_segment_is_accounted_for_exactly_once_and_loops_balance() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let segments: Vec<_> = Tokenizer::with_delimiters(&bytes, delims).collect();
        let (events, engine) = common::run_engine_keeping(&spec, segments.iter().cloned());
        let mut seen = vec![0usize; segments.len()];
        let mut depth = 0usize;
        let mut opens = 0usize;
        let mut closes = 0usize;
        for event in &events {
            match *event {
                Event::LoopOpened { .. } => {
                    depth += 1;
                    opens += 1;
                }
                Event::LoopClosed { .. } => {
                    assert!(depth > 0, "{name}: close without open");
                    depth -= 1;
                    closes += 1;
                }
                Event::Captured { segment, .. }
                | Event::Unmatched { segment }
                | Event::Empty { segment } => match seen.get_mut(segment) {
                    Some(n) => *n += 1,
                    None => panic!(
                        "{name}: event {event:?} references segment #{segment} but the file has only {} segments",
                        segments.len()
                    ),
                },
            }
        }
        if let Some((index, &count)) = seen.iter().enumerate().find(|&(_, &n)| n != 1) {
            panic!(
                "{name}: segment #{index} appears {count} times among Captured/Unmatched/Empty, expected exactly once"
            );
        }
        assert_eq!(opens, closes, "{name}: opens and closes balance");
        assert_eq!(depth, 0, "{name}: nothing left open after finish");
        assert!(
            engine.path().is_empty(),
            "{name}: path is not empty after finish: {:?}",
            engine.path()
        );
    }
}

#[test]
fn loop_instances_match_their_trigger_counts() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let segments: Vec<_> = Tokenizer::with_delimiters(&bytes, delims).collect();
        let tree = LoopTree::build(&spec, segments.iter().cloned());
        let count_ids = |id: &[u8]| segments.iter().filter(|s| s.id == id).count();
        let instances = |loop_name: &str| tree.nodes_of(spec.loop_id(loop_name).unwrap()).count();
        assert_eq!(
            instances("2100"),
            count_ids(b"CLP"),
            "{name}: one 2100 per CLP"
        );
        assert_eq!(
            instances("2110"),
            count_ids(b"SVC"),
            "{name}: one 2110 per SVC"
        );
        assert_eq!(
            instances("2000"),
            count_ids(b"LX"),
            "{name}: one 2000 per LX"
        );
        assert_eq!(
            instances("transaction"),
            count_ids(b"ST"),
            "{name}: one transaction per ST"
        );
        assert_eq!(
            instances("1000A"),
            count_ids(b"ST"),
            "{name}: one payer loop per transaction"
        );
        assert_eq!(
            instances("1000B"),
            count_ids(b"ST"),
            "{name}: one payee loop per transaction"
        );
        let listed: usize = tree
            .nodes()
            .iter()
            .map(|n| n.segments.len() + n.unmatched.len())
            .sum();
        let non_empty = segments.iter().filter(|s| !s.is_empty()).count();
        assert_eq!(
            listed, non_empty,
            "{name}: the tree lists every non-empty segment once"
        );
    }
}

#[test]
fn unmatched_and_implicit_are_exactly_the_known_anomalies() {
    let spec = Spec::builtin_835();
    let mut unmatched: BTreeMap<String, usize> = BTreeMap::new();
    let mut implicit: BTreeMap<String, usize> = BTreeMap::new();
    for (name, bytes, delims) in common::all_files() {
        let events = common::events_of(&spec, &bytes, delims);
        for event in events {
            match event {
                Event::Unmatched { .. } => *unmatched.entry(name.clone()).or_default() += 1,
                Event::LoopOpened { implicit: true, .. } => {
                    *implicit.entry(name.clone()).or_default() += 1
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        unmatched,
        BTreeMap::from([
            ("multi_claim_sample.txt".to_string(), 4),
            ("trizetto_sample.rmt".to_string(), 1),
        ]),
        "only the multi_claim N3/N4 in loop 2100 and the bogus trizetto XX are unmatched"
    );
    assert_eq!(
        implicit,
        BTreeMap::from([("blue_cross_nc_sample.txt".to_string(), 2)]),
        "only the fragment needs an implicit interchange and group"
    );
}

#[test]
fn feeding_from_a_document_or_a_tokenizer_gives_the_same_events() {
    use edi835_core::Document;
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let doc = Document::with_delimiters(&bytes[..], delims);
        let a = common::run_engine(&spec, &doc);
        let b = common::events_of(&spec, &bytes, delims);
        assert_eq!(a, b, "{name}");
    }
}
