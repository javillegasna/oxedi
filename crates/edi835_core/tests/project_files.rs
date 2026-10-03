//! Tables and diagnostics of the processor over every real-shaped file.

mod common;

use edi835_core::{Document, Event, Processor, Spec, Tokenizer};

/// Fed segment by segment from a tokenizer and drained after each
/// transaction, the processor gives the same rows and diagnostics as one run
/// over the whole document.
#[test]
fn draining_after_each_transaction_adds_up_to_one_run_over_the_document() {
    let spec = Spec::builtin_835();
    let transaction = spec.loop_id("transaction").unwrap();
    for (name, bytes, delims) in common::all_files() {
        let document = Document::with_delimiters(&bytes[..], delims);
        let (whole, diagnostics) = Processor::run(&spec, &document);

        let mut processor = Processor::new(&spec, &delims);
        let mut drained = Vec::new();
        let mut incremental = Vec::new();
        for segment in Tokenizer::with_delimiters(&bytes, delims) {
            let output = processor.feed(&segment);
            incremental.extend_from_slice(output.diagnostics());
            if output
                .events()
                .contains(&Event::LoopClosed { id: transaction })
            {
                drained.push(processor.take_tables());
            }
        }
        incremental.extend_from_slice(processor.finish().diagnostics());
        drained.push(processor.take_tables());

        assert_eq!(incremental, diagnostics, "{name}");
        assert_eq!(whole.len(), 5, "{name}");
        for table in &whole {
            let pieces: Vec<String> = drained
                .iter()
                .flat_map(|tables| common::table_rows(tables.get(table.name()).unwrap()))
                .collect();
            assert_eq!(
                pieces,
                common::table_rows(table),
                "{name}: {}",
                table.name()
            );
        }
    }
}
