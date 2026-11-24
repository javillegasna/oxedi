//! The work done once per spec or per run rather than per segment: compiling
//! a built-in spec from its model, building the projector's plans and
//! building the write plan, for each built-in version.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxedi_core::spec_load::{Builtin, Version};
use oxedi_core::write::WritePlan;
use oxedi_core::{Delimiters, Projector, Spec};
use std::hint::black_box;

fn compile_builtins(c: &mut Criterion) {
    let delimiters = Delimiters::new(b'*', b':', b'~');
    let mut group = c.benchmark_group("compile");
    for version in Version::ALL {
        let builtin =
            Builtin::find("835", version).expect("the registry ships the 835 at every version");
        let model = builtin.model().expect("a built-in's model reads");
        let spec: &Spec = &builtin.spec;
        let name = version.text();
        group.bench_with_input(BenchmarkId::new("spec", name), &model, |b, model| {
            b.iter(|| {
                black_box(model)
                    .compile()
                    .expect("a built-in's model compiles")
            });
        });
        group.bench_with_input(BenchmarkId::new("projector", name), spec, |b, spec| {
            b.iter(|| Projector::new(black_box(spec), &delimiters));
        });
        group.bench_with_input(BenchmarkId::new("write_plan", name), spec, |b, spec| {
            b.iter(|| WritePlan::new(black_box(spec)));
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = common::profiled();
    targets = compile_builtins
}
criterion_main!(benches);
