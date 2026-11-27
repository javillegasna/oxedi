//! The api's reads and batches equal the stages they run, on every fixture and sample.

mod common;

use std::sync::Arc;

use common::{NO_ISA_FIXTURE, all_files, load_fixture};
use oxedi_core::api::{
    BatchBy, Batches, ChosenSpec, DelimiterChoice, ReadOptions, SpecChoice, read, read_tables,
    read_tree,
};
use oxedi_core::{
    document::Document,
    engine::{Event, LoopEngine},
    mask::Mask,
    process::Processor,
    spec::Spec,
    spec_load::Builtin,
    tokenizer::Tokenizer,
    tree::LoopTree,
    vocabulary::DateMode,
};

type Options<'m> = ReadOptions<'m, Arc<Spec>>;

#[test]
fn read_with_the_defaults_is_the_processors_run() {
    for (name, bytes, _) in all_files().into_iter().filter(|f| f.0 != NO_ISA_FIXTURE) {
        let reading = read(bytes.as_slice(), Options::default()).expect(&name);
        let ChosenSpec::Builtin(chosen) = reading.spec else {
            panic!("{name}: the default choice is a built-in");
        };
        assert!(std::ptr::eq(
            chosen,
            Builtin::select(reading.document.segments())
        ));
        let expected = Processor::run(&chosen.spec, &reading.document, None);
        assert_eq!((reading.tables, reading.diagnostics), expected, "{name}");
    }
}

#[test]
fn a_given_spec_reads_a_4010_file_with_it() {
    let bytes = load_fixture("balanced_4010_sample.txt");
    let spec = Arc::new(Spec::builtin_835());
    let options = ReadOptions {
        spec: SpecChoice::Given(Arc::clone(&spec)),
        ..Options::default()
    };
    let reading = read(bytes.as_slice(), options).unwrap();
    assert!(matches!(reading.spec, ChosenSpec::Given(_)));
    let expected = Processor::run(&spec, &reading.document, None);
    assert_eq!((reading.tables, reading.diagnostics), expected);
}

#[test]
fn a_mask_reads_as_the_processor_masks() {
    let bytes = load_fixture("masked_findings_5010_sample.txt");
    let mask = Mask::new(&[7u8; 32], DateMode::Year).unwrap();
    let options = Options {
        mask: Some(&mask),
        ..Options::default()
    };
    let reading = read(bytes.as_slice(), options).unwrap();
    let expected = Processor::run(&Builtin::fallback().spec, &reading.document, Some(&mask));
    assert_eq!((reading.tables, reading.diagnostics), expected);
}

#[test]
fn given_delimiters_read_the_file_without_an_isa() {
    let files = all_files();
    let (_, bytes, delimiters) = files.iter().find(|f| f.0 == NO_ISA_FIXTURE).unwrap();
    let options = Options {
        delimiters: DelimiterChoice::Given(*delimiters),
        ..Options::default()
    };
    let reading = read(bytes.as_slice(), options).unwrap();
    let document = Document::with_delimiters(bytes.as_slice(), *delimiters).unwrap();
    let expected = Processor::run(&Builtin::fallback().spec, &document, None);
    assert_eq!((reading.tables, reading.diagnostics), expected);
}

#[test]
fn read_tables_fills_only_the_named_table() {
    let bytes = load_fixture("multi_claim_sample.txt");
    let document = Document::parse(bytes.as_slice()).unwrap();
    let spec = &Builtin::fallback().spec;
    let tables = read_tables(&document, spec, &["claims"], None).unwrap();
    assert_eq!(
        tables,
        Processor::run_tables(spec, &document, &["claims"], None).unwrap()
    );
    assert_eq!(
        read_tables(&document, spec, &["nope"], None)
            .unwrap_err()
            .name,
        "nope"
    );
}

/// The node of each segment as the viewer finds it: a stack of open node
/// numbers (the root 0, then each loop opened in order), the owner being the
/// stack's top when the segment's first event arrives.
fn owners_by_stack(spec: &Spec, document: &Document<'_>) -> Vec<usize> {
    let mut engine = LoopEngine::new(spec);
    let (mut open, mut next) = (vec![0usize], 1usize);
    let mut owner = vec![None; document.len()];
    let mut fold = |events: &[Event]| {
        for event in events {
            match *event {
                Event::LoopOpened { .. } => {
                    open.push(next);
                    next += 1;
                }
                Event::LoopClosed { .. } if open.len() > 1 => {
                    open.pop();
                }
                Event::Captured { segment, .. }
                | Event::Unmatched { segment, .. }
                | Event::Empty { segment } => {
                    owner[segment].get_or_insert(open[open.len() - 1]);
                }
                Event::LoopClosed { .. } => {}
            }
        }
    };
    for segment in document.segments() {
        fold(engine.feed(&segment));
    }
    fold(engine.finish());
    owner.into_iter().map(|node| node.unwrap_or(0)).collect()
}

#[test]
fn every_segment_has_the_node_the_viewer_finds() {
    for (name, bytes, delimiters) in all_files() {
        let document = Document::with_delimiters(bytes.as_slice(), delimiters).unwrap();
        let spec = &Builtin::select(document.segments()).spec;
        let tree = LoopTree::build(spec, document.segments());
        let found: Vec<usize> = (0..document.len()).map(|i| tree.owner(i).index()).collect();
        assert_eq!(found, owners_by_stack(spec, &document), "{name}");
    }
}

#[test]
fn every_node_has_the_viewers_ordinal() {
    for (name, bytes, delimiters) in all_files() {
        let document = Document::with_delimiters(bytes.as_slice(), delimiters).unwrap();
        let spec = &Builtin::select(document.segments()).spec;
        let tree = LoopTree::build(spec, document.segments());
        let mut seen = vec![0usize; spec.loops().len()];
        for node in tree.nodes() {
            let expected = node.loop_id.map(|id| {
                seen[id.index()] += 1;
                seen[id.index()]
            });
            assert_eq!(node.ordinal, expected, "{name}");
        }
    }
}

#[test]
fn batches_together_are_one_run() {
    for (name, bytes, _) in all_files().into_iter().filter(|f| f.0 != NO_ISA_FIXTURE) {
        let document = Document::parse(bytes.as_slice()).unwrap();
        let spec = &Builtin::select(document.segments()).spec;
        let (tables, diagnostics) = Processor::run(spec, &document, None);
        let segments = Tokenizer::new(&bytes).unwrap();
        let batches: Vec<_> = Batches::new(spec, segments, &BatchBy::default(), None)
            .unwrap()
            .collect();
        let found: Vec<_> = batches.iter().flat_map(|b| b.diagnostics.clone()).collect();
        assert_eq!(found, diagnostics, "{name}");
        for table in tables.iter() {
            let rows: usize = batches
                .iter()
                .map(|b| b.tables.get(table.name()).map_or(0, |t| t.len()))
                .sum();
            assert_eq!(rows, table.len(), "{name}: {}", table.name());
        }
        assert!(
            !batches.iter().any(|b| b.is_empty()),
            "{name}: an empty batch"
        );
    }
}

#[test]
fn batches_by_the_interchange_are_one() {
    let bytes = load_fixture("emedny_sample.txt");
    let spec = &Builtin::fallback().spec;
    let by = BatchBy::Loop("interchange".into());
    let segments = Tokenizer::new(&bytes).unwrap();
    assert_eq!(Batches::new(spec, segments, &by, None).unwrap().count(), 1);
}

#[test]
fn read_tree_is_read_and_the_built_tree() {
    for (name, bytes, _) in all_files().into_iter().filter(|f| f.0 != NO_ISA_FIXTURE) {
        let read_only = read(bytes.as_slice(), Options::default()).unwrap();
        let (reading, tree) = read_tree(bytes.as_slice(), Options::default()).unwrap();
        assert_eq!(reading.tables, read_only.tables, "{name}");
        assert_eq!(reading.diagnostics, read_only.diagnostics, "{name}");
        let spec = reading.spec.spec();
        assert_eq!(
            tree,
            LoopTree::build(spec, reading.document.segments()),
            "{name}"
        );
    }
}
