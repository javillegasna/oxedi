//! Throughput of the tokenizer and the document index pass over the three
//! largest fixtures, of the loop engine over the three largest samples in
//! bytes and in events, of the engine with the envelope checker over the
//! same samples, and of the whole processor (engine, checker, projector)
//! over them in bytes and in table rows.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use edi835_core::{Document, EnvelopeChecker, LoopEngine, Processor, Spec, Tokenizer};
use std::hint::black_box;

const FIXTURES: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
];

/// The three largest anonymized samples, the engine's workload.
const SAMPLES: &[&str] = &[
    "edi835_test_united.rmt",
    "edi835_test_versant.RMT",
    "edi835_test_eyemed.RMT",
];

fn load_from(dir: &str, name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(dir)
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn load(name: &str) -> Vec<u8> {
    load_from("tests/fixtures", name)
}

/// Runs the engine and the envelope checker over every segment and returns
/// how many diagnostics the checker raised.
fn run_check(spec: &Spec, bytes: &[u8]) -> usize {
    let mut engine = LoopEngine::new(spec);
    let mut checker = EnvelopeChecker::new(spec);
    let mut diagnostics = 0usize;
    for segment in Tokenizer::new(bytes).expect("sample has an ISA") {
        let events = engine.feed(&segment);
        diagnostics += checker.on(&segment, events).len();
    }
    engine.finish();
    diagnostics + checker.finish().len()
}

/// Indexes the bytes, runs the processor over the document and returns how
/// many table rows it produced.
fn run_process(spec: &Spec, bytes: &[u8]) -> usize {
    let document = Document::parse(bytes).expect("sample has an ISA");
    let (tables, _) = Processor::run(spec, &document);
    tables.iter().map(|table| table.len()).sum()
}

/// Runs the engine over every segment and returns how many events it emitted.
fn run_engine(spec: &Spec, bytes: &[u8]) -> usize {
    let mut engine = LoopEngine::new(spec);
    let mut events = 0usize;
    for segment in Tokenizer::new(bytes).expect("sample has an ISA") {
        events += engine.feed(&segment).len();
    }
    events + engine.finish().len()
}

fn tokenize_fixtures(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenize");
    for name in FIXTURES {
        let bytes = load(name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| {
                Tokenizer::new(black_box(bytes))
                    .expect("fixture has an ISA")
                    .count()
            });
        });
    }
    group.finish();
}

fn index_fixtures(c: &mut Criterion) {
    let mut group = c.benchmark_group("index");
    for name in FIXTURES {
        let bytes = load(name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| {
                Document::parse(black_box(&bytes[..]))
                    .expect("fixture has an ISA")
                    .len()
            });
        });
    }
    group.finish();
}

fn engine_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("engine");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_engine(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

fn engine_events_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("engine_events");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        let events = run_engine(&spec, &bytes);
        group.throughput(Throughput::Elements(events as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_engine(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

fn check_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("check");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_check(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

fn process_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("process");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_process(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

fn process_rows_samples(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let mut group = c.benchmark_group("process_rows");
    for name in SAMPLES {
        let bytes = load_from("tests/samples", name);
        let rows = run_process(&spec, &bytes);
        group.throughput(Throughput::Elements(rows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| run_process(&spec, black_box(bytes)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    tokenize_fixtures,
    index_fixtures,
    engine_samples,
    engine_events_samples,
    check_samples,
    process_samples,
    process_rows_samples
);
criterion_main!(benches);
