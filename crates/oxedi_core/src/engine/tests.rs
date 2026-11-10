use super::*;
use crate::spec::test_support::nested;
use crate::{Delimiters, Spec, Tokenizer};

fn segs(input: &[u8]) -> Vec<Segment<'_>> {
    Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')).collect()
}

/// Feeds every segment and returns the events of the *last* one, plus the path after it.
fn run<'s>(spec: &'s Spec, input: &[u8]) -> (Vec<Event>, Vec<&'s str>) {
    let mut engine = LoopEngine::new(spec);
    let segments = segs(input);
    let mut last = Vec::new();
    for segment in &segments {
        last = engine.feed(segment).to_vec();
    }
    let path = engine.path().iter().map(|&id| spec.loop_name(id)).collect();
    (last, path)
}

/// Runs the input and asserts the events of its last segment and the loop path after it.
fn assert_run(spec: &Spec, input: &[u8], events: Vec<Event>, path: Vec<&str>) {
    let (got_events, got_path) = run(spec, input);
    assert_eq!(got_events, events);
    assert_eq!(got_path, path);
}

fn id(spec: &Spec, name: &str) -> LoopId {
    spec.loop_id(name).unwrap()
}

/// The event that opens the loop `name` at `segment`.
fn opened(spec: &Spec, name: &str, implicit: bool, segment: usize) -> Event {
    Event::LoopOpened {
        id: id(spec, name),
        implicit,
        segment,
    }
}

/// The event that captures `segment` into the loop `name`.
fn captured(spec: &Spec, name: &str, segment: usize) -> Event {
    Event::Captured {
        id: id(spec, name),
        segment,
    }
}

#[test]
fn trigger_opens_child_and_captures_it() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~",
        vec![opened(&spec, "A", false, 0), captured(&spec, "A", 0)],
        vec!["A"],
    );
}

#[test]
fn segment_in_current_loop_is_captured() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~BB~B1~",
        vec![captured(&spec, "B", 2)],
        vec!["A", "B"],
    );
}

#[test]
fn sibling_trigger_closes_and_reopens() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~BB~B1~BB~",
        vec![
            Event::LoopClosed { id: id(&spec, "B") },
            opened(&spec, "B", false, 3),
            captured(&spec, "B", 3),
        ],
        vec!["A", "B"],
    );
}

#[test]
fn segment_of_an_outer_loop_pops_the_inner() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~BB~CC*X~A1~",
        vec![
            Event::LoopClosed { id: id(&spec, "C") },
            Event::LoopClosed { id: id(&spec, "B") },
            captured(&spec, "A", 3),
        ],
        vec!["A"],
    );
}

#[test]
fn end_segment_captures_then_closes() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~BB~AE~",
        vec![
            Event::LoopClosed { id: id(&spec, "B") },
            captured(&spec, "A", 2),
            Event::LoopClosed { id: id(&spec, "A") },
        ],
        vec![],
    );
}

#[test]
fn unknown_segment_is_unmatched_and_keeps_the_path() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~BB~ZZ*1~",
        vec![Event::Unmatched { segment: 2 }],
        vec!["A", "B"],
    );
    assert_run(&spec, b"ZZ~", vec![Event::Unmatched { segment: 0 }], vec![]);
}

#[test]
fn trigger_condition_must_match() {
    let spec = nested();
    let (events, path) = run(&spec, b"AA~BB~CC*Y~");
    assert_eq!(
        events,
        vec![Event::Unmatched { segment: 2 }],
        "CC*Y triggers nothing and B does not hold CC"
    );
    assert_eq!(path, vec!["A", "B"]);
    let (events, path) = run(&spec, b"AA~BB~CC*X~");
    assert_eq!(events[0], opened(&spec, "C", false, 2));
    assert_eq!(path, vec!["A", "B", "C"]);
}

#[test]
fn missing_ancestors_open_implicitly() {
    let spec = nested();
    assert_run(
        &spec,
        b"CC*X~",
        vec![
            opened(&spec, "A", true, 0),
            opened(&spec, "B", true, 0),
            opened(&spec, "C", false, 0),
            captured(&spec, "C", 0),
        ],
        vec!["A", "B", "C"],
    );
    let (events, _) = run(&spec, b"AA~CC*X~");
    assert_eq!(
        events[0],
        opened(&spec, "B", true, 1),
        "only the missing ancestor is implicit"
    );
}

#[test]
fn finish_closes_everything_innermost_first() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    for segment in &segs(b"AA~BB~CC*X~") {
        engine.feed(segment);
    }
    assert_eq!(
        engine.finish(),
        &[
            Event::LoopClosed { id: id(&spec, "C") },
            Event::LoopClosed { id: id(&spec, "B") },
            Event::LoopClosed { id: id(&spec, "A") },
        ]
    );
    assert!(engine.path().is_empty());
    assert!(engine.finish().is_empty(), "finishing twice emits nothing");
}

#[test]
fn empty_segment_is_reported_as_empty() {
    let spec = nested();
    assert_run(
        &spec,
        b"AA~\n",
        vec![Event::Empty { segment: 1 }],
        vec!["A"],
    );
}

#[test]
fn a_reachable_trigger_wins_over_capture_in_the_current_loop() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "A":{"trigger":{"segment":"AA"}},
            "B":{"parent":"A","trigger":{"segment":"BB"}},
            "C":{"parent":"B","trigger":{"segment":"CC"},"occurrences":{"cc":{"segment":"CC","pos":0},"n1":{"segment":"N1","pos":1}}},
            "D":{"parent":"A","trigger":{"segment":"N1"}}
        }}"#,
    )
    .unwrap();
    assert_run(
        &spec,
        b"AA~BB~CC~N1~",
        vec![
            Event::LoopClosed { id: id(&spec, "C") },
            Event::LoopClosed { id: id(&spec, "B") },
            opened(&spec, "D", false, 3),
            captured(&spec, "D", 3),
        ],
        vec!["A", "D"],
    );
}

#[test]
fn implicit_open_keeps_the_nearest_open_ancestor_below_the_top() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "A":{"trigger":{"segment":"AA"}},
            "B":{"parent":"A","trigger":{"segment":"BB"}},
            "P":{"parent":"A","trigger":{"segment":"PP"}},
            "T":{"parent":"P","trigger":{"segment":"TT"}}
        }}"#,
    )
    .unwrap();
    assert_run(
        &spec,
        b"AA~BB~TT~",
        vec![
            Event::LoopClosed { id: id(&spec, "B") },
            opened(&spec, "P", true, 2),
            opened(&spec, "T", false, 2),
            captured(&spec, "T", 2),
        ],
        vec!["A", "P", "T"],
    );
}

#[test]
fn a_trigger_matching_at_two_depths_opens_under_the_innermost_parent() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "A":{"trigger":{"segment":"AA"}},
            "B":{"parent":"A","trigger":{"segment":"BB"}},
            "X":{"parent":"A","trigger":{"segment":"XX"}},
            "Y":{"parent":"B","trigger":{"segment":"XX"}}
        }}"#,
    )
    .unwrap();
    assert_run(
        &spec,
        b"AA~BB~XX~",
        vec![opened(&spec, "Y", false, 2), captured(&spec, "Y", 2)],
        vec!["A", "B", "Y"],
    );
}

#[test]
fn an_end_segment_listed_by_an_inner_loop_is_captured_there_and_closes_nothing() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "A":{"trigger":{"segment":"AA"},"end":"AE"},
            "B":{"parent":"A","trigger":{"segment":"BB"},"occurrences":{"bb":{"segment":"BB","pos":0},"ae":{"segment":"AE","pos":1}}}
        }}"#,
    )
    .unwrap();
    assert_run(
        &spec,
        b"AA~BB~AE~",
        vec![captured(&spec, "B", 2)],
        vec!["A", "B"],
    );
}

#[test]
fn feeding_after_finish_behaves_like_a_fresh_engine() {
    let spec = nested();
    let segments = segs(b"AA~BB~CC*X~");
    let mut fresh = LoopEngine::new(&spec);
    let expected: Vec<Vec<Event>> = segments
        .iter()
        .map(|segment| fresh.feed(segment).to_vec())
        .collect();
    let mut reused = LoopEngine::new(&spec);
    for segment in &segments {
        reused.feed(segment);
    }
    reused.finish();
    let again: Vec<Vec<Event>> = segments
        .iter()
        .map(|segment| reused.feed(segment).to_vec())
        .collect();
    assert_eq!(again, expected);
}

#[test]
fn an_event_stays_three_words() {
    assert_eq!(
        std::mem::size_of::<Event>(),
        3 * std::mem::size_of::<usize>()
    );
}

#[test]
fn every_explicit_opening_names_its_own_trigger() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let openings: Vec<(&str, bool, usize)> = segs(b"AA~BB~B1~BB~CC*X~")
        .iter()
        .flat_map(|segment| engine.feed(segment).to_vec())
        .filter_map(|event| match event {
            Event::LoopOpened {
                id,
                implicit,
                segment,
            } => Some((spec.loop_name(id), implicit, segment)),
            _ => None,
        })
        .collect();
    assert_eq!(
        openings,
        vec![
            ("A", false, 0),
            ("B", false, 1),
            ("B", false, 3),
            ("C", false, 4)
        ]
    );
}

#[test]
fn feed_returns_the_events_of_that_call_only() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let segments = segs(b"AA~A1~");
    assert_eq!(engine.feed(&segments[0]).len(), 2);
    assert_eq!(engine.feed(&segments[1]).len(), 1);
}
