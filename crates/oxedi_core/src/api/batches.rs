//! The batches of a stream, cut where an instance of a loop closes.

use crate::column::Tables;
use crate::diagnostic::Diagnostic;
use crate::mask::Mask;
use crate::process::Processor;
use crate::spec::{BatchLoopError, LoopId, Spec};
use crate::tokenizer::Tokenizer;

/// The loop whose closing ends a batch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BatchBy {
    /// The envelope loop (a loop with a control) nested deepest; the first in
    /// spec order on a tie.
    #[default]
    InnermostEnvelope,
    /// The loop of this name.
    Loop(String),
}

impl BatchBy {
    /// The loop of `spec` this names.
    pub fn resolve(&self, spec: &Spec) -> Result<LoopId, BatchLoopError> {
        match self {
            BatchBy::Loop(name) => spec.loop_id(name).ok_or_else(|| BatchLoopError::Unknown {
                by: name.clone(),
                loops: spec.loops().iter().map(|l| l.name.clone()).collect(),
            }),
            BatchBy::InnermostEnvelope => {
                innermost_envelope(spec).ok_or(BatchLoopError::NoEnvelope)
            }
        }
    }
}

/// The deepest loop with a control, the first in spec order on a tie.
fn innermost_envelope(spec: &Spec) -> Option<LoopId> {
    let depth = |mut parent: Option<LoopId>| {
        let mut depth = 0;
        while let Some(up) = parent {
            depth += 1;
            parent = spec.loops().get(up.index()).and_then(|l| l.parent);
        }
        depth
    };
    let enveloped = spec
        .loops()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.control.is_some());
    // Reversed, so the last maximum is the first in spec order.
    enveloped
        .rev()
        .max_by_key(|(_, l)| depth(l.parent))
        .map(|(index, _)| LoopId(index))
}

/// The tables and diagnostics produced since the previous batch.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// The rows appended since the previous batch.
    pub tables: Tables,
    /// The diagnostics emitted since the previous batch, in stream order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Batch {
    /// `true` when it holds no row and no diagnostic.
    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(|table| table.is_empty()) && self.diagnostics.is_empty()
    }
}

/// Reads a stream and yields a batch each time an instance of the chosen loop
/// closes, and one more at the end when it holds a row or a diagnostic.
#[derive(Debug)]
pub struct Batches<'a> {
    segments: Tokenizer<'a>,
    processor: Processor<'a>,
    by: LoopId,
    done: bool,
}

impl<'a> Batches<'a> {
    /// Batches of `segments` read with `spec`, cut as `by` says.
    pub fn new(
        spec: &'a Spec,
        segments: Tokenizer<'a>,
        by: &BatchBy,
        mask: Option<&'a Mask>,
    ) -> Result<Self, BatchLoopError> {
        let by = by.resolve(spec)?;
        let processor = Processor::new(spec, segments.delimiters()).with_mask(mask);
        Ok(Self {
            segments,
            processor,
            by,
            done: false,
        })
    }
}

impl Iterator for Batches<'_> {
    type Item = Batch;

    fn next(&mut self) -> Option<Batch> {
        if self.done {
            return None;
        }
        let mut diagnostics = Vec::new();
        for segment in self.segments.by_ref() {
            let output = self.processor.feed(&segment);
            diagnostics.extend_from_slice(output.diagnostics());
            if output.events().iter().any(|event| event.closes(self.by)) {
                let tables = self.processor.take_tables();
                return Some(Batch {
                    tables,
                    diagnostics,
                });
            }
        }
        self.done = true;
        diagnostics.extend_from_slice(self.processor.finish().diagnostics());
        let tables = self.processor.take_tables();
        Some(Batch {
            tables,
            diagnostics,
        })
        .filter(|batch| !batch.is_empty())
    }
}
