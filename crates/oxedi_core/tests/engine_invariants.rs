//! Structural invariants of the engine over every real-shaped file.

mod common;

use std::collections::BTreeMap;

use oxedi_core::{Event, Instance, LoopEngine, LoopTree, OccurrenceId, Spec, Tokenizer};

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
                | Event::Unmatched { segment, .. }
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
fn every_opening_names_the_segment_captured_right_after_it() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let events = common::events_of(&spec, &bytes, delims);
        for (at, event) in events.iter().enumerate() {
            let Event::LoopOpened { segment, .. } = *event else {
                continue;
            };
            let next_capture = events[at..].iter().find_map(|later| match *later {
                Event::Captured { segment, .. } => Some(segment),
                _ => None,
            });
            assert_eq!(
                next_capture,
                Some(segment),
                "{name}: event #{at} {event:?} names a segment other than the next capture"
            );
        }
    }
}

#[test]
fn feeding_from_a_document_or_a_tokenizer_gives_the_same_events() {
    use oxedi_core::Document;
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let doc = Document::with_delimiters(&bytes[..], delims).unwrap();
        let a = common::run_engine(&spec, &doc);
        let b = common::events_of(&spec, &bytes, delims);
        assert_eq!(a, b, "{name}");
    }
}

#[test]
fn a_tree_folded_from_caller_driven_events_equals_the_built_tree() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let built = LoopTree::build(&spec, Tokenizer::with_delimiters(&bytes, delims));
        let mut engine = LoopEngine::new(&spec);
        let mut builder = LoopTree::builder();
        for segment in Tokenizer::with_delimiters(&bytes, delims) {
            builder.on(engine.feed(&segment));
        }
        builder.on(engine.finish());
        assert_eq!(builder.finish(), built, "{name}");
    }
}

#[test]
fn events_agree_with_a_replay_of_their_own_stack() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let mut stack: Vec<Instance> = Vec::new();
        let mut counters: BTreeMap<usize, usize> = BTreeMap::new();
        let mut next_id = 0usize;
        for event in common::events_of(&spec, &bytes, delims) {
            match event {
                Event::LoopOpened { instance, .. } => {
                    let counter = counters.entry(instance.loop_id.index()).or_default();
                    *counter += 1;
                    let expected = (next_id, *counter, stack.last().map(|top| top.id));
                    next_id += 1;
                    assert_eq!(
                        (
                            instance.id.index(),
                            instance.ordinal_in_stream,
                            instance.parent
                        ),
                        expected,
                        "{name}: opened instance {instance:?} disagrees with the replay"
                    );
                    stack.push(instance);
                }
                Event::LoopClosed { instance } => {
                    assert_eq!(stack.pop(), Some(instance), "{name}: closed {instance:?}");
                }
                Event::Captured { instance, .. } => {
                    assert_eq!(stack.last(), Some(&instance), "{name}: captured in");
                }
                Event::Unmatched { within, .. } => {
                    assert_eq!(stack.last().copied(), within, "{name}: unmatched within");
                }
                Event::Empty { .. } => {}
            }
        }
    }
}

#[test]
fn the_index_agrees_with_a_scan() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let events = common::events_of(&spec, &bytes, delims);
        let segments: Vec<_> = Tokenizer::with_delimiters(&bytes, delims).collect();
        for event in events {
            let Event::Captured {
                instance,
                segment,
                occurrence,
            } = event
            else {
                continue;
            };
            let segment = &segments[segment];
            let scanned = spec
                .get(instance.loop_id)
                .occurrences
                .iter()
                .position(|occurrence| occurrence.matches(segment));
            let carried = occurrence.map(OccurrenceId::index);
            assert_eq!(carried, scanned, "{name}: segment {segment:?}");
        }
    }
}
