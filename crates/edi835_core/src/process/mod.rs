//! One pass over a segment stream with every consumer of the engine's events.
//!
//! The processor feeds each segment to the loop engine and hands the events
//! it returns to the envelope checker and to the projector, so the loop
//! structure, the structural and element diagnostics and the table rows all
//! come from the same walk. [`Processor::run`] does the same over a whole
//! document and is the same code path.

use crate::check::EnvelopeChecker;
use crate::column::Tables;
use crate::delimiters::Delimiters;
use crate::diagnostic::Diagnostic;
use crate::document::Document;
use crate::engine::{Event, LoopEngine};
use crate::project::Projector;
use crate::segment::Segment;
use crate::spec::Spec;

/// What one segment, or the end of the stream, produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    events: Vec<Event>,
    diagnostics: Vec<Diagnostic>,
}

impl Output {
    /// The engine's events, in order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The envelope checker's diagnostics, then the projector's.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// The loop engine, the envelope checker and the projector, fed together.
#[derive(Debug, Clone)]
pub struct Processor<'s> {
    engine: LoopEngine<'s>,
    checker: EnvelopeChecker<'s>,
    projector: Projector<'s>,
    output: Output,
}

impl<'s> Processor<'s> {
    /// A processor at the root with empty tables; `delimiters` are those the
    /// segments were read with.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        Self {
            engine: LoopEngine::new(spec),
            checker: EnvelopeChecker::new(spec, delimiters),
            projector: Projector::new(spec, delimiters),
            output: Output::default(),
        }
    }

    /// Consumes one segment. The output is valid until the next call.
    pub fn feed(&mut self, segment: &Segment<'_>) -> &Output {
        self.output.events.clear();
        self.output.diagnostics.clear();
        let events = self.engine.feed(segment);
        self.output.events.extend_from_slice(events);
        self.output
            .diagnostics
            .extend_from_slice(self.checker.on(segment, events));
        self.output
            .diagnostics
            .extend_from_slice(self.projector.on(segment, events));
        &self.output
    }

    /// Closes every loop still open and appends the rows they were
    /// collecting. The processor is then back at the root, as a new one;
    /// take the tables before feeding it another stream.
    pub fn finish(&mut self) -> &Output {
        self.output.events.clear();
        self.output.diagnostics.clear();
        self.output.events.extend_from_slice(self.engine.finish());
        self.output
            .diagnostics
            .extend_from_slice(self.checker.finish());
        self.output
            .diagnostics
            .extend_from_slice(self.projector.finish());
        &self.output
    }

    /// The diagnostics of the latest `feed` or `finish`.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.output.diagnostics()
    }

    /// Moves every appended row out; see [`Projector::take_tables`]. A text
    /// column holds at most `i32::MAX` bytes until drained (a value past that
    /// is reported and its cell is null), so drain per transaction. Each
    /// stream's tables stand on their own: take them before feeding another
    /// stream, because `finish` restarts row numbers.
    pub fn take_tables(&mut self) -> Tables {
        self.projector.take_tables()
    }

    /// Processes a whole document: every segment, then `finish`. Returns
    /// the tables and every diagnostic in stream order.
    pub fn run(spec: &Spec, document: &Document<'_>) -> (Tables, Vec<Diagnostic>) {
        let mut processor = Processor::new(spec, document.delimiters());
        let mut diagnostics = Vec::new();
        for segment in document.segments() {
            diagnostics.extend_from_slice(processor.feed(&segment).diagnostics());
        }
        diagnostics.extend_from_slice(processor.finish().diagnostics());
        (processor.take_tables(), diagnostics)
    }
}

#[cfg(test)]
mod tests;
