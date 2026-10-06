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
        /// `true` when no segment of its own opened it.
        implicit: bool,
        /// Index of the trigger segment that caused the opening: the loop's
        /// own trigger, or for an implicit opening the trigger of the
        /// descendant that needed it.
        segment: usize,
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
    /// Scratch for the ancestor chain of an implicit open.
    chain: Vec<LoopId>,
}

impl<'s> LoopEngine<'s> {
    /// An engine at the root, with nothing open.
    pub fn new(spec: &'s Spec) -> Self {
        Self {
            spec,
            stack: Vec::new(),
            events: Vec::new(),
            chain: Vec::new(),
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
                self.open(child, false, index);
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
            let mut chain = std::mem::take(&mut self.chain);
            self.spec.ancestors_into(target, &mut chain);
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
                self.open(ancestor, true, index);
            }
            self.chain = chain;
            self.open(target, false, index);
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

    fn open(&mut self, id: LoopId, implicit: bool, segment: usize) {
        self.stack.push(id);
        self.events.push(Event::LoopOpened {
            id,
            implicit,
            segment,
        });
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
mod tests;
