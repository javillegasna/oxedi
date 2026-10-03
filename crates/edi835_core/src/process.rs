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
            checker: EnvelopeChecker::new(spec),
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

    /// Moves every appended row out; see [`Projector::take_tables`].
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
mod tests {
    use super::*;
    use crate::{SnipLevel, Tokenizer};

    const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240101*1200*^*00501*000000001*0*P*>~";

    fn delimiters() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    /// A complete interchange holding the given transactions, each written
    /// between its `ST` and its `SE`.
    fn interchange(transactions: &[&str]) -> String {
        let mut text = format!("{ISA}GS*HP*SENDER*RECEIVER*20240101*1200*7*X*005010X221A1~");
        for (i, body) in transactions.iter().enumerate() {
            let count = body.matches('~').count() + 2;
            text.push_str(&format!(
                "ST*835*{:04}~{body}SE*{count}*{:04}~",
                i + 1,
                i + 1
            ));
        }
        text.push_str(&format!("GE*{}*7~IEA*1*000000001~", transactions.len()));
        text
    }

    const CLAIM: &str = "BPR*I*10*C*CHK~TRN*1*1~LX*1~CLP*C1*1*10*10~SVC*HC:99213*10*10~";

    #[test]
    fn one_feed_returns_the_events_and_every_diagnostic_of_its_segment() {
        let spec = Spec::builtin_835();
        let mut processor = Processor::new(&spec, &delimiters());
        let input = "ST*835*0001~CLP*C1*1*12A*0~";
        let segments: Vec<_> = Tokenizer::with_delimiters(input.as_bytes(), delimiters()).collect();
        processor.feed(&segments[0]);
        let output = processor.feed(&segments[1]);
        let names: Vec<String> = output
            .events()
            .iter()
            .map(|event| match *event {
                Event::LoopOpened { id, implicit, .. } => {
                    format!(
                        "open{} {}",
                        if implicit { "!" } else { "" },
                        spec.loop_name(id)
                    )
                }
                Event::Captured { id, segment } => format!("cap {} #{segment}", spec.loop_name(id)),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(names, vec!["open! 2000", "open 2100", "cap 2100 #1"]);
        let levels: Vec<SnipLevel> = output.diagnostics().iter().map(|d| d.level).collect();
        assert_eq!(levels, vec![SnipLevel::L1, SnipLevel::L2]);
        assert_eq!(
            output.diagnostics()[1].to_string(),
            "SNIP 2 · element CLP03 (total_claim_charge_amount) is not a valid R (decimal, scale 2) · segment #1, element 3 · at interchange#1/group#1/transaction#1/2000#1/2100#1 · datum \"12A\""
        );
        let latest = output.diagnostics().to_vec();
        assert_eq!(processor.diagnostics(), latest);
    }

    #[test]
    fn finishing_reports_what_is_left_open_and_appends_its_rows() {
        let spec = Spec::builtin_835();
        let mut processor = Processor::new(&spec, &delimiters());
        let input = format!("{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~{CLAIM}");
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            processor.feed(&segment);
        }
        let output = processor.finish();
        assert_eq!(output.events().len(), 6, "six loops close");
        assert_eq!(
            output.diagnostics().len(),
            3,
            "three envelopes lack their end"
        );
        let tables = processor.take_tables();
        for name in ["payments", "claims", "services"] {
            assert_eq!(tables.get(name).unwrap().len(), 1, "{name}");
        }
    }

    #[test]
    fn run_over_a_document_is_feeding_every_segment_then_finishing() {
        let spec = Spec::builtin_835();
        let input = interchange(&[CLAIM, "BPR*I*1*C*CHK~TRN*1*2~ZZZ~"]);
        let document = Document::with_delimiters(input.as_bytes(), delimiters());
        let (tables, diagnostics) = Processor::run(&spec, &document);
        let mut processor = Processor::new(&spec, &delimiters());
        let mut by_hand = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            by_hand.extend_from_slice(processor.feed(&segment).diagnostics());
        }
        by_hand.extend_from_slice(processor.finish().diagnostics());
        assert_eq!(diagnostics, by_hand);
        assert_eq!(tables, processor.take_tables());
        assert_eq!(diagnostics.len(), 1, "the unknown ZZZ");
        assert_eq!(tables.get("payments").unwrap().len(), 2);
    }

    #[test]
    fn tables_drained_after_each_transaction_add_up_to_one_run() {
        let spec = Spec::builtin_835();
        let input = interchange(&[CLAIM, CLAIM, CLAIM]);
        let transaction = spec.loop_id("transaction").unwrap();
        let mut processor = Processor::new(&spec, &delimiters());
        let mut drained = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let closes = processor
                .feed(&segment)
                .events()
                .contains(&Event::LoopClosed { id: transaction });
            if closes {
                drained.push(processor.take_tables());
            }
        }
        processor.finish();
        drained.push(processor.take_tables());
        let document = Document::with_delimiters(input.as_bytes(), delimiters());
        let (whole, _) = Processor::run(&spec, &document);
        for table in &whole {
            let rows = |tables: &Tables| -> Vec<String> {
                let part = tables.get(table.name()).unwrap();
                (0..part.len())
                    .map(|row| {
                        part.columns()
                            .iter()
                            .map(|(_, column)| column.render(row).unwrap())
                            .collect::<Vec<_>>()
                            .join("|")
                    })
                    .collect()
            };
            let pieces: Vec<String> = drained.iter().flat_map(rows).collect();
            assert_eq!(pieces, rows(&whole), "{}", table.name());
        }
        assert_eq!(
            drained
                .iter()
                .map(|t| t.get("claims").unwrap().len())
                .collect::<Vec<_>>(),
            vec![1, 1, 1, 0]
        );
    }
}
