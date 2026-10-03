//! Throughput of the tokenizer and the document index pass over the three
//! largest fixtures, of the loop engine over the three largest samples in
//! bytes and in events, of the engine with the envelope checker over the
//! same samples, of the engine over claim fragments without their envelope
//! (implicit ancestors against the same segments under an explicit `LX`), and
//! of the whole processor (engine, checker, projector) over the samples in
//! bytes and in table rows.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use edi835_core::{Delimiters, Document, EnvelopeChecker, LoopEngine, Processor, Spec, Tokenizer};
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
    let delimiters = Delimiters::from_isa(bytes).expect("sample has an ISA");
    let mut checker = EnvelopeChecker::new(spec, &delimiters);
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

/// The segments of each claim in `multi_claim_sample.txt`, from its `CLP` up
/// to the next `CLP`, `LX` or `SE`.
fn claim_segments() -> Vec<Vec<String>> {
    let text = String::from_utf8(load_from("tests/fixtures", "multi_claim_sample.txt"))
        .expect("fixture is ASCII");
    let mut claims: Vec<Vec<String>> = Vec::new();
    for segment in text.split('~').map(str::trim).filter(|s| !s.is_empty()) {
        match segment.split('*').next() {
            Some("CLP") => claims.push(vec![segment.to_string()]),
            Some("SE") => break,
            Some("LX") => {}
            _ => {
                if let Some(claim) = claims.last_mut() {
                    claim.push(segment.to_string());
                }
            }
        }
    }
    claims
}

/// `repeats` copies of `unit`, each segment ended with `~`, and how many
/// segments that is.
fn repeated(unit: &[String], repeats: usize) -> (Vec<u8>, usize) {
    let mut bytes = Vec::new();
    for _ in 0..repeats {
        for segment in unit {
            bytes.extend_from_slice(segment.as_bytes());
            bytes.push(b'~');
        }
    }
    (bytes, unit.len() * repeats)
}

/// Runs the engine over `bytes` split with the 835 delimiters.
fn run_engine_plain(spec: &Spec, bytes: &[u8]) -> usize {
    let delimiters = Delimiters::new(b'*', b':', b'~');
    let mut engine = LoopEngine::new(spec);
    let mut events = 0usize;
    for segment in Tokenizer::with_delimiters(bytes, delimiters) {
        events += engine.feed(&segment).len();
    }
    events + engine.finish().len()
}

/// Fragments without their envelope. In the `claims` variant each repetition
/// is every claim of the fixture followed by a `PLB`, and only its first `CLP`
/// opens the missing ancestors implicitly; in the `pair` variant each
/// repetition is one `CLP` and one `PLB`, which opens them. Each `implicit`
/// input is paired with a `baseline` of the same segments preceded by the `LX`
/// that holds them; both are measured per segment of the implicit unit, so the
/// baseline's extra `LX` is not counted as work.
fn engine_fragment(c: &mut Criterion) {
    let spec = Spec::builtin_835();
    let plb = "PLB*1234*20240101*WO:ABC*1".to_string();
    let mut claims: Vec<String> = Vec::new();
    for claim in claim_segments() {
        claims.extend(claim);
    }
    claims.push(plb.clone());
    let pair = vec!["CLP*1*1*100*80".to_string(), plb];
    let mut group = c.benchmark_group("engine_fragment");
    for (label, unit, repeats) in [("claims", claims, 2000), ("pair", pair, 20000)] {
        let implicit_len = unit.len();
        let mut held = vec!["LX*1".to_string()];
        held.extend(unit.iter().cloned());
        for (kind, unit) in [("implicit", &unit), ("baseline", &held)] {
            let (bytes, _) = repeated(unit, repeats);
            group.throughput(Throughput::Elements((implicit_len * repeats) as u64));
            group.bench_with_input(BenchmarkId::new(label, kind), &bytes, |b, bytes| {
                b.iter(|| run_engine_plain(&spec, black_box(bytes)))
            });
        }
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
    engine_fragment,
    check_samples,
    process_samples,
    process_rows_samples
);
criterion_main!(benches);
