//! Throughput of the tokenizer and the document index pass over the three
//! largest fixtures.

mod common;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use oxedi_core::{Document, Tokenizer};
use std::hint::black_box;

const FIXTURES: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
];

fn load(name: &str) -> Vec<u8> {
    common::load_from("tests/fixtures", name)
}

/// Times `work` over every fixture, one benchmark id per fixture name, with
/// the throughput in input bytes. Generic so each closure is inlined.
fn bench_fixtures<F: Fn(&[u8]) -> usize>(c: &mut Criterion, group: &str, work: F) {
    let mut group = c.benchmark_group(group);
    for name in FIXTURES {
        let bytes = load(name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| work(black_box(bytes)));
        });
    }
    group.finish();
}

fn tokenize_fixtures(c: &mut Criterion) {
    bench_fixtures(c, "tokenize", |bytes| {
        Tokenizer::new(bytes).expect("fixture has an ISA").count()
    });
}

fn index_fixtures(c: &mut Criterion) {
    bench_fixtures(c, "index", |bytes| {
        Document::parse(bytes).expect("fixture has an ISA").len()
    });
}

criterion_group! {
    name = benches;
    config = common::profiled();
    targets = tokenize_fixtures, index_fixtures
}
criterion_main!(benches);
