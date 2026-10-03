//! The loop engine: a state machine fed one segment at a time, driven by a spec.
//!
//! The engine keeps a stack of open loops. For each segment it tries, in
//! order: a trigger of a child of any open loop (innermost first) or of the
//! root; a segment some open loop holds (innermost first; the loop's end
//! segment also closes it); a trigger whose ancestors are not open, which
//! opens them implicitly; and otherwise the segment is unmatched. Events refer
//! to segments by index and are returned as a slice of a reused buffer.

use crate::segment::Segment;
use crate::spec::{LoopId, Spec};

/// What the engine observed while consuming one segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A loop started; `implicit` when it was opened to host a descendant
    /// whose ancestors were absent from the input.
    LoopOpened {
        /// The loop.
        id: LoopId,
        /// `true` when no segment opened it.
        implicit: bool,
    },
    /// A loop ended.
    LoopClosed {
        /// The loop.
        id: LoopId,
    },
    /// A segment belongs to the given loop.
    Captured {
        /// The loop.
        id: LoopId,
        /// Index of the segment in the stream.
        segment: usize,
    },
    /// No loop could hold the segment; the path is unchanged. The segment
    /// itself is found by index through [`Document::segment`] or
    /// [`Document::spans`]; its loop location is [`LoopEngine::path`] at the
    /// moment this event is returned (in a [`LoopTree`], the node whose
    /// `unmatched` list holds the index).
    ///
    /// [`Document::segment`]: crate::Document::segment
    /// [`Document::spans`]: crate::Document::spans
    /// [`LoopTree`]: crate::LoopTree
    Unmatched {
        /// Index of the segment in the stream.
        segment: usize,
    },
    /// The segment has no content (`~~` or trailing trivia).
    Empty {
        /// Index of the segment in the stream.
        segment: usize,
    },
}

/// Interprets a segment stream against a spec.
#[derive(Debug, Clone)]
pub struct LoopEngine<'s> {
    spec: &'s Spec,
    stack: Vec<LoopId>,
    events: Vec<Event>,
}

impl<'s> LoopEngine<'s> {
    /// An engine at the root, with nothing open.
    pub fn new(spec: &'s Spec) -> Self {
        Self {
            spec,
            stack: Vec::new(),
            events: Vec::new(),
        }
    }

    /// The spec in use.
    pub fn spec(&self) -> &'s Spec {
        self.spec
    }

    /// Open loops, outermost first.
    pub fn path(&self) -> &[LoopId] {
        &self.stack
    }

    /// Consumes one segment and returns what happened. The slice is valid
    /// until the next call.
    pub fn feed(&mut self, segment: &Segment<'_>) -> &[Event] {
        self.events.clear();
        let index = segment.index;
        if segment.is_empty() {
            self.events.push(Event::Empty { segment: index });
            return &self.events;
        }

        for depth in (0..=self.stack.len()).rev() {
            let parent = depth.checked_sub(1).map(|i| self.stack[i]);
            if let Some(child) = self.spec.matching_child(parent, segment) {
                self.close_to(depth);
                self.open(child, false);
                self.capture(child, index);
                return &self.events;
            }
        }

        for depth in (1..=self.stack.len()).rev() {
            let id = self.stack[depth - 1];
            let def = self.spec.get(id);
            if def.accepts(segment.id) {
                let closes = def.end.as_deref() == Some(segment.id);
                self.close_to(depth);
                self.capture(id, index);
                if closes {
                    self.close_to(depth - 1);
                }
                return &self.events;
            }
        }

        if let Some(target) = self.spec.matching_any(segment) {
            let chain = self.spec.ancestors(target);
            // Keep the stack down to the nearest open ancestor; everything in
            // the chain after it is missing and opens implicitly.
            let (depth, first_missing) = chain
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, ancestor)| {
                    self.stack
                        .iter()
                        .position(|open| open == ancestor)
                        .map(|pos| (pos + 1, i + 1))
                })
                .unwrap_or((0, 0));
            self.close_to(depth);
            for &ancestor in &chain[first_missing..] {
                self.open(ancestor, true);
            }
            self.open(target, false);
            self.capture(target, index);
            return &self.events;
        }

        self.events.push(Event::Unmatched { segment: index });
        &self.events
    }

    /// Closes every open loop, innermost first. Call once the stream ends.
    /// The engine is then back at the root: feeding it again behaves like a
    /// fresh engine.
    pub fn finish(&mut self) -> &[Event] {
        self.events.clear();
        self.close_to(0);
        &self.events
    }

    fn open(&mut self, id: LoopId, implicit: bool) {
        self.stack.push(id);
        self.events.push(Event::LoopOpened { id, implicit });
    }

    fn capture(&mut self, id: LoopId, segment: usize) {
        self.events.push(Event::Captured { id, segment });
    }

    fn close_to(&mut self, depth: usize) {
        while self.stack.len() > depth {
            if let Some(id) = self.stack.pop() {
                self.events.push(Event::LoopClosed { id });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Spec, Tokenizer};

    const TINY: &str = r#"{"name":"t","loops":{
        "A":{"trigger":{"segment":"AA"},"segments":["A1"],"end":"AE"},
        "B":{"parent":"A","trigger":{"segment":"BB"},"segments":["B1"]},
        "C":{"parent":"B","trigger":{"segment":"CC","where":{"1":"X"}},"segments":["C1"]}
    }}"#;

    fn spec() -> Spec {
        Spec::from_json(TINY).unwrap()
    }

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

    fn id(spec: &Spec, name: &str) -> LoopId {
        spec.loop_id(name).unwrap()
    }

    #[test]
    fn trigger_opens_child_and_captures_it() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~");
        assert_eq!(
            events,
            vec![
                Event::LoopOpened {
                    id: id(&spec, "A"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "A"),
                    segment: 0
                }
            ]
        );
        assert_eq!(path, vec!["A"]);
    }

    #[test]
    fn segment_in_current_loop_is_captured() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~B1~");
        assert_eq!(
            events,
            vec![Event::Captured {
                id: id(&spec, "B"),
                segment: 2
            }]
        );
        assert_eq!(path, vec!["A", "B"]);
    }

    #[test]
    fn sibling_trigger_closes_and_reopens() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~B1~BB~");
        assert_eq!(
            events,
            vec![
                Event::LoopClosed { id: id(&spec, "B") },
                Event::LoopOpened {
                    id: id(&spec, "B"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "B"),
                    segment: 3
                },
            ]
        );
        assert_eq!(path, vec!["A", "B"]);
    }

    #[test]
    fn segment_of_an_outer_loop_pops_the_inner() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~CC*X~A1~");
        assert_eq!(
            events,
            vec![
                Event::LoopClosed { id: id(&spec, "C") },
                Event::LoopClosed { id: id(&spec, "B") },
                Event::Captured {
                    id: id(&spec, "A"),
                    segment: 3
                },
            ]
        );
        assert_eq!(path, vec!["A"]);
    }

    #[test]
    fn end_segment_captures_then_closes() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~AE~");
        assert_eq!(
            events,
            vec![
                Event::LoopClosed { id: id(&spec, "B") },
                Event::Captured {
                    id: id(&spec, "A"),
                    segment: 2
                },
                Event::LoopClosed { id: id(&spec, "A") },
            ]
        );
        assert!(path.is_empty());
    }

    #[test]
    fn unknown_segment_is_unmatched_and_keeps_the_path() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~ZZ*1~");
        assert_eq!(events, vec![Event::Unmatched { segment: 2 }]);
        assert_eq!(path, vec!["A", "B"]);
        let (events, path) = run(&spec, b"ZZ~");
        assert_eq!(events, vec![Event::Unmatched { segment: 0 }]);
        assert!(path.is_empty());
    }

    #[test]
    fn trigger_condition_must_match() {
        let spec = spec();
        let (events, path) = run(&spec, b"AA~BB~CC*Y~");
        assert_eq!(
            events,
            vec![Event::Unmatched { segment: 2 }],
            "CC*Y triggers nothing and B does not hold CC"
        );
        assert_eq!(path, vec!["A", "B"]);
        let (events, path) = run(&spec, b"AA~BB~CC*X~");
        assert_eq!(
            events[0],
            Event::LoopOpened {
                id: id(&spec, "C"),
                implicit: false
            }
        );
        assert_eq!(path, vec!["A", "B", "C"]);
    }

    #[test]
    fn missing_ancestors_open_implicitly() {
        let spec = spec();
        let (events, path) = run(&spec, b"CC*X~");
        assert_eq!(
            events,
            vec![
                Event::LoopOpened {
                    id: id(&spec, "A"),
                    implicit: true
                },
                Event::LoopOpened {
                    id: id(&spec, "B"),
                    implicit: true
                },
                Event::LoopOpened {
                    id: id(&spec, "C"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "C"),
                    segment: 0
                },
            ]
        );
        assert_eq!(path, vec!["A", "B", "C"]);
        let (events, _) = run(&spec, b"AA~CC*X~");
        assert_eq!(
            events[0],
            Event::LoopOpened {
                id: id(&spec, "B"),
                implicit: true
            },
            "only the missing ancestor is implicit"
        );
    }

    #[test]
    fn finish_closes_everything_innermost_first() {
        let spec = spec();
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
        let spec = spec();
        let (events, path) = run(&spec, b"AA~\n");
        assert_eq!(events, vec![Event::Empty { segment: 1 }]);
        assert_eq!(path, vec!["A"]);
    }

    #[test]
    fn a_reachable_trigger_wins_over_capture_in_the_current_loop() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "A":{"trigger":{"segment":"AA"}},
                "B":{"parent":"A","trigger":{"segment":"BB"}},
                "C":{"parent":"B","trigger":{"segment":"CC"},"segments":["N1"]},
                "D":{"parent":"A","trigger":{"segment":"N1"}}
            }}"#,
        )
        .unwrap();
        let (events, path) = run(&spec, b"AA~BB~CC~N1~");
        assert_eq!(
            events,
            vec![
                Event::LoopClosed { id: id(&spec, "C") },
                Event::LoopClosed { id: id(&spec, "B") },
                Event::LoopOpened {
                    id: id(&spec, "D"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "D"),
                    segment: 3
                },
            ]
        );
        assert_eq!(path, vec!["A", "D"]);
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
        let (events, path) = run(&spec, b"AA~BB~TT~");
        assert_eq!(
            events,
            vec![
                Event::LoopClosed { id: id(&spec, "B") },
                Event::LoopOpened {
                    id: id(&spec, "P"),
                    implicit: true
                },
                Event::LoopOpened {
                    id: id(&spec, "T"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "T"),
                    segment: 2
                },
            ]
        );
        assert_eq!(path, vec!["A", "P", "T"]);
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
        let (events, path) = run(&spec, b"AA~BB~XX~");
        assert_eq!(
            events,
            vec![
                Event::LoopOpened {
                    id: id(&spec, "Y"),
                    implicit: false
                },
                Event::Captured {
                    id: id(&spec, "Y"),
                    segment: 2
                },
            ]
        );
        assert_eq!(path, vec!["A", "B", "Y"]);
    }

    #[test]
    fn an_end_segment_listed_by_an_inner_loop_is_captured_there_and_closes_nothing() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "A":{"trigger":{"segment":"AA"},"end":"AE"},
                "B":{"parent":"A","trigger":{"segment":"BB"},"segments":["AE"]}
            }}"#,
        )
        .unwrap();
        let (events, path) = run(&spec, b"AA~BB~AE~");
        assert_eq!(
            events,
            vec![Event::Captured {
                id: id(&spec, "B"),
                segment: 2
            }]
        );
        assert_eq!(path, vec!["A", "B"]);
    }

    #[test]
    fn feeding_after_finish_behaves_like_a_fresh_engine() {
        let spec = spec();
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
    fn feed_returns_the_events_of_that_call_only() {
        let spec = spec();
        let mut engine = LoopEngine::new(&spec);
        let segments = segs(b"AA~A1~");
        assert_eq!(engine.feed(&segments[0]).len(), 2);
        assert_eq!(engine.feed(&segments[1]).len(), 1);
    }
}
