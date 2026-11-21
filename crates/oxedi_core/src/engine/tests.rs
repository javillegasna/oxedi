use super::*;
use crate::spec::test_support::nested;
use crate::{Delimiters, Spec, Tokenizer};

fn segs(input: &[u8]) -> Vec<Segment<'_>> {
    Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')).collect()
}

/// An event as the tests compare it: the loop by name, no instance numbers.
#[derive(Debug, PartialEq)]
enum Seen<'s> {
    Opened(&'s str, bool, usize),
    Closed(&'s str),
    Captured(&'s str, usize),
    Unmatched(usize),
    Empty(usize),
}

fn seen<'s>(spec: &'s Spec, event: &Event) -> Seen<'s> {
    match *event {
        Event::LoopOpened {
            instance,
            implicit,
            segment,
        } => Seen::Opened(spec.loop_name(instance.loop_id), implicit, segment),
        Event::LoopClosed { instance } => Seen::Closed(spec.loop_name(instance.loop_id)),
        Event::Captured {
            instance, segment, ..
        } => Seen::Captured(spec.loop_name(instance.loop_id), segment),
        Event::Unmatched { segment, .. } => Seen::Unmatched(segment),
        Event::Empty { segment } => Seen::Empty(segment),
    }
}

/// Feeds every segment and returns the events of the *last* one, plus the path after it.
fn run<'s>(spec: &'s Spec, input: &[u8]) -> (Vec<Seen<'s>>, Vec<&'s str>) {
    let (events, path) = last_events(spec, input);
    (events.iter().map(|event| seen(spec, event)).collect(), path)
}

/// Feeds every segment and returns the raw events of the last one, plus the
/// loop path after it.
fn last_events<'s>(spec: &'s Spec, input: &[u8]) -> (Vec<Event>, Vec<&'s str>) {
    let mut engine = LoopEngine::new(spec);
    let mut last = Vec::new();
    for segment in &segs(input) {
        last = engine.feed(segment).to_vec();
    }
    let path = engine
        .path()
        .iter()
        .map(|open| spec.loop_name(open.loop_id))
        .collect();
    (last, path)
}

/// Runs the input and asserts the events of its last segment and the loop path after it.
fn assert_run(spec: &Spec, input: &[u8], events: Vec<Seen<'_>>, path: Vec<&str>) {
    let (got_events, got_path) = run(spec, input);
    assert_eq!(got_events, events);
    assert_eq!(got_path, path);
}

fn id(spec: &Spec, name: &str) -> LoopId {
    spec.loop_id(name).unwrap()
}

/// The loop `name` of `spec`, named by the spec so it outlives the lookup.
fn name_of<'s>(spec: &'s Spec, name: &str) -> &'s str {
    spec.loop_name(id(spec, name))
}

/// The event that opens the loop `name` at `segment`.
fn opened<'s>(spec: &'s Spec, name: &str, implicit: bool, segment: usize) -> Seen<'s> {
    Seen::Opened(name_of(spec, name), implicit, segment)
}

/// The event that captures `segment` into the loop `name`.
fn captured<'s>(spec: &'s Spec, name: &str, segment: usize) -> Seen<'s> {
    Seen::Captured(name_of(spec, name), segment)
}

/// The event that closes the loop `name`.
fn closed<'s>(spec: &'s Spec, name: &str) -> Seen<'s> {
    Seen::Closed(name_of(spec, name))
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
            closed(&spec, "B"),
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
            closed(&spec, "C"),
            closed(&spec, "B"),
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
            closed(&spec, "B"),
            captured(&spec, "A", 2),
            closed(&spec, "A"),
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
        vec![Seen::Unmatched(2)],
        vec!["A", "B"],
    );
    assert_run(&spec, b"ZZ~", vec![Seen::Unmatched(0)], vec![]);
}

#[test]
fn trigger_condition_must_match() {
    let spec = nested();
    let (events, path) = run(&spec, b"AA~BB~CC*Y~");
    assert_eq!(
        events,
        vec![Seen::Unmatched(2)],
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
    let closes: Vec<Seen<'_>> = engine.finish().iter().map(|e| seen(&spec, e)).collect();
    assert_eq!(
        closes,
        vec![closed(&spec, "C"), closed(&spec, "B"), closed(&spec, "A")]
    );
    assert!(engine.path().is_empty());
    assert!(engine.finish().is_empty(), "finishing twice emits nothing");
}

#[test]
fn empty_segment_is_reported_as_empty() {
    let spec = nested();
    assert_run(&spec, b"AA~\n", vec![Seen::Empty(1)], vec!["A"]);
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
            closed(&spec, "C"),
            closed(&spec, "B"),
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
            closed(&spec, "B"),
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
fn an_event_stays_eight_words() {
    assert_eq!(
        std::mem::size_of::<Event>(),
        8 * std::mem::size_of::<usize>()
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
                instance,
                implicit,
                segment,
            } => Some((spec.loop_name(instance.loop_id), implicit, segment)),
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

/// Feeds every segment of `input` to `engine`.
fn feed_all(engine: &mut LoopEngine<'_>, input: &[u8]) {
    for segment in &segs(input) {
        engine.feed(segment);
    }
}

/// The open path as `(loop name, id, ordinal, parent id)`.
fn describe<'s>(spec: &'s Spec, path: &[Instance]) -> Vec<(&'s str, usize, usize, Option<usize>)> {
    path.iter()
        .map(|open| {
            (
                spec.loop_name(open.loop_id),
                open.id.index(),
                open.ordinal_in_stream,
                open.parent.map(InstanceId::index),
            )
        })
        .collect()
}

/// The open path after feeding `input`.
fn instances<'s>(spec: &'s Spec, input: &[u8]) -> Vec<(&'s str, usize, usize, Option<usize>)> {
    let mut engine = LoopEngine::new(spec);
    feed_all(&mut engine, input);
    describe(spec, engine.path())
}

#[test]
fn instances_are_numbered_in_opening_order() {
    assert_eq!(
        instances(&nested(), b"AA~BB~BB~"),
        vec![("A", 0, 1, None), ("B", 2, 2, Some(0))]
    );
}

#[test]
fn the_ordinal_counts_the_loop_across_parents() {
    assert_eq!(
        instances(&nested(), b"AA~BB~AA~BB~"),
        vec![("A", 2, 2, None), ("B", 3, 2, Some(2))]
    );
}

#[test]
fn implicit_ancestors_get_their_own_instances() {
    assert_eq!(
        instances(&nested(), b"CC*X~"),
        vec![
            ("A", 0, 1, None),
            ("B", 1, 1, Some(0)),
            ("C", 2, 1, Some(1))
        ]
    );
}

#[test]
fn finish_restarts_ids_and_ordinals() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    feed_all(&mut engine, b"AA~BB~");
    engine.finish();
    feed_all(&mut engine, b"AA~");
    let open = describe(&spec, engine.path());
    assert_eq!(open, vec![("A", 0, 1, None)]);
}

/// An instance of `name` as the engine numbers it.
fn instance(
    spec: &Spec,
    name: &str,
    index: usize,
    ordinal: usize,
    parent: Option<usize>,
) -> Instance {
    Instance {
        id: InstanceId::at(index),
        loop_id: id(spec, name),
        ordinal_in_stream: ordinal,
        parent: parent.map(InstanceId::at),
    }
}

#[test]
fn every_event_names_its_instance() {
    let spec = nested();
    let a = instance(&spec, "A", 0, 1, None);
    let b = instance(&spec, "B", 1, 1, Some(0));
    assert_eq!(
        last_events(&spec, b"AA~BB~AE~").0,
        vec![
            Event::LoopClosed { instance: b },
            Event::Captured {
                instance: a,
                segment: 2,
                occurrence: None
            },
            Event::LoopClosed { instance: a },
        ]
    );
}

#[test]
fn an_unmatched_segment_names_the_instance_it_fell_in() {
    let spec = nested();
    let b = instance(&spec, "B", 1, 1, Some(0));
    assert_eq!(
        last_events(&spec, b"AA~BB~ZZ~").0,
        vec![Event::Unmatched {
            segment: 2,
            within: Some(b)
        }]
    );
    assert_eq!(
        last_events(&spec, b"ZZ~").0,
        vec![Event::Unmatched {
            segment: 0,
            within: None
        }]
    );
}

#[test]
fn closes_is_true_only_for_a_close_of_that_loop() {
    let spec = nested();
    let a = instance(&spec, "A", 0, 1, None);
    let closing = Event::LoopClosed { instance: a };
    assert!(closing.closes(id(&spec, "A")));
    assert!(!closing.closes(id(&spec, "B")));
    let capturing = Event::Captured {
        instance: a,
        segment: 0,
        occurrence: None,
    };
    assert!(!capturing.closes(id(&spec, "A")));
}

/// The name of the occurrence the capture event of the last segment of
/// `input` carries, inside loop `loop_name`; `None` when it carries none.
fn captured_occurrence<'s>(spec: &'s Spec, loop_name: &str, input: &[u8]) -> Option<&'s str> {
    let loop_id = id(spec, loop_name);
    let (events, _) = last_events(spec, input);
    let occurrence = events.iter().find_map(|event| match *event {
        Event::Captured {
            instance,
            occurrence,
            ..
        } if instance.loop_id == loop_id => Some(occurrence),
        _ => None,
    });
    assert!(occurrence.is_some(), "no capture in {loop_name}");
    occurrence
        .flatten()
        .map(|found| spec.get(loop_id).occurrences[found.index()].name.as_str())
}

#[test]
fn a_captured_segment_names_its_occurrence() {
    let spec = nested();
    assert_eq!(captured_occurrence(&spec, "A", b"AA~A1~"), Some("a1"));
    assert_eq!(captured_occurrence(&spec, "A", b"AA~"), Some("aa"));
    // The loop declares no occurrence for its end segment.
    assert_eq!(captured_occurrence(&spec, "A", b"AA~AE~"), None);
}

#[test]
fn a_held_segment_with_no_matching_occurrence_is_captured_with_none() {
    let spec = Spec::builtin_835();
    assert_eq!(
        captured_occurrence(&spec, "2100", b"CLP*1*1*1*1~NM1*ZZ~"),
        None
    );
}

/// Feeds every segment and returns the events of the last step and the open
/// path after it, as `lineage` takes them.
fn last_step(engine: &mut LoopEngine<'_>, input: &[u8]) -> (Vec<Event>, Vec<Instance>) {
    let mut last = Vec::new();
    for segment in &segs(input) {
        last = engine.feed(segment).to_vec();
    }
    (last, engine.path().to_vec())
}

/// The instance of the first event `pick` accepts.
fn instance_of(events: &[Event], pick: fn(&Event) -> Option<Instance>) -> Instance {
    events
        .iter()
        .find_map(pick)
        .expect("the step has the event")
}

fn captured_instance(event: &Event) -> Option<Instance> {
    match *event {
        Event::Captured { instance, .. } => Some(instance),
        _ => None,
    }
}

fn closed_instance(event: &Event) -> Option<Instance> {
    match *event {
        Event::LoopClosed { instance } => Some(instance),
        _ => None,
    }
}

/// The instance ids of a lineage, outermost first.
fn ids(path: &[Instance]) -> Vec<usize> {
    path.iter().map(|instance| instance.id.index()).collect()
}

#[test]
fn lineage_of_a_captured_segment_lists_its_open_ancestors() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let (events, open) = last_step(&mut engine, b"AA~BB~");
    let instance = instance_of(&events, captured_instance);
    assert_eq!(ids(&open), vec![0, 1]);
    assert_eq!(ids(&lineage(instance, &open, &events)), vec![0, 1]);
}

#[test]
fn lineage_keeps_the_loops_closed_after_the_capture() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let (events, open) = last_step(&mut engine, b"AA~BB~AE~");
    assert!(open.is_empty());
    let captured = instance_of(&events, captured_instance);
    assert_eq!(ids(&lineage(captured, &open, &events)), vec![0]);
    let closed = instance_of(&events, closed_instance);
    assert_eq!(closed.loop_id, id(&spec, "B"));
    assert_eq!(ids(&lineage(closed, &open, &events)), vec![0, 1]);
}

#[test]
fn lineage_of_a_close_before_an_open_skips_the_new_instances() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let (events, open) = last_step(&mut engine, b"AA~BB~AA~");
    let closed = instance_of(&events, closed_instance);
    assert_eq!(closed.loop_id, id(&spec, "B"));
    assert_eq!(ids(&open), vec![2]);
    assert_eq!(ids(&lineage(closed, &open, &events)), vec![0, 1]);
}

#[test]
fn lineage_at_finish_reads_the_closing_events() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    last_step(&mut engine, b"AA~BB~");
    let events = engine.finish().to_vec();
    let closed = events
        .iter()
        .filter_map(closed_instance)
        .find(|instance| instance.loop_id == id(&spec, "B"))
        .expect("finish closes B");
    assert_eq!(ids(&lineage(closed, &[], &events)), vec![0, 1]);
}

#[test]
fn lineage_stops_at_a_parent_it_cannot_find() {
    let spec = nested();
    let orphan = Instance {
        id: InstanceId::at(10),
        loop_id: id(&spec, "B"),
        ordinal_in_stream: 1,
        parent: Some(InstanceId::at(9)),
    };
    assert_eq!(lineage(orphan, &[], &[]), vec![orphan]);
}

/// The instance ids of the paths `loop_paths` returns for `wanted`.
fn path_ids(spec: &Spec, input: &[u8], wanted: &[usize]) -> Vec<Vec<usize>> {
    loop_paths(spec, segs(input), wanted)
        .iter()
        .map(|path| ids(path))
        .collect()
}

#[test]
fn loop_paths_come_in_the_order_asked() {
    assert_eq!(
        path_ids(&nested(), b"AA~BB~AE~", &[1, 0]),
        vec![vec![0, 1], vec![0]]
    );
}

#[test]
fn a_segment_that_closes_its_loop_keeps_that_loop_in_its_path() {
    assert_eq!(path_ids(&nested(), b"AA~BB~AE~", &[2]), vec![vec![0]]);
}

#[test]
fn an_unmatched_segment_has_the_path_of_the_loops_open_around_it() {
    assert_eq!(path_ids(&nested(), b"AA~ZZ~", &[1]), vec![vec![0]]);
}

#[test]
fn an_index_past_the_last_segment_has_an_empty_path_and_a_repeated_one_repeats() {
    assert_eq!(
        path_ids(&nested(), b"AA~BB~", &[1, 9, 1]),
        vec![vec![0, 1], vec![], vec![0, 1]]
    );
}

#[test]
fn an_event_names_its_instance() {
    let spec = nested();
    let at = |index: usize| Instance {
        id: InstanceId::at(index),
        loop_id: spec.roots()[0],
        ordinal_in_stream: index + 1,
        parent: None,
    };
    let (a, b) = (at(0), at(1));
    let opened = Event::LoopOpened {
        instance: a,
        implicit: false,
        segment: 0,
    };
    let captured = Event::Captured {
        instance: b,
        segment: 1,
        occurrence: None,
    };
    let events = [
        (opened, Some(a)),
        (Event::LoopClosed { instance: a }, Some(a)),
        (captured, Some(b)),
        (
            Event::Unmatched {
                segment: 2,
                within: Some(b),
            },
            Some(b),
        ),
        (
            Event::Unmatched {
                segment: 3,
                within: None,
            },
            None,
        ),
        (Event::Empty { segment: 4 }, None),
    ];
    for (event, expected) in events {
        assert_eq!(event.instance(), expected, "{event:?}");
    }
}

#[test]
fn a_named_lineage_gives_loop_names_and_ordinals() {
    let spec = nested();
    let mut engine = LoopEngine::new(&spec);
    let (events, open) = last_step(&mut engine, b"AA~BB~");
    let captured = events
        .iter()
        .find(|event| captured_instance(event).is_some())
        .expect("the step captures");
    assert_eq!(
        named_lineage(&spec, captured, &open, &events),
        vec![("A", 1), ("B", 1)]
    );
    let empty = Event::Empty { segment: 0 };
    assert!(named_lineage(&spec, &empty, &open, &events).is_empty());
}
