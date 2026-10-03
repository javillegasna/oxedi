//! Criterion skeleton. Measures nothing meaningful yet — it exists so that
//! `cargo bench` is part of the workflow from commit zero (N4: perf-conscious).

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn smoke_benchmark(c: &mut Criterion) {
    c.bench_function("smoke_add", |b| b.iter(|| black_box(1) + black_box(1)));
}

criterion_group!(benches, smoke_benchmark);
criterion_main!(benches);
