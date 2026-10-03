//! Tokenizer throughput over the three largest fixtures. No threshold: this
//! records a baseline.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use edi835_core::{Document, Tokenizer};
use std::hint::black_box;

const FIXTURES: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
];

fn load(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
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

criterion_group!(benches, tokenize_fixtures, index_fixtures);
criterion_main!(benches);
