use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

fn criterion_benchmark(c: &mut Criterion) {
    let mut map = HashMap::new();
    for i in 0..64 {
        map.insert(i, i);
    }

    let mut map2 = indexmap::IndexMap::new();
    for i in 0..64 {
        map2.insert(i, i);
    }

    c.bench_function("std::map::iter", |b| b.iter(|| map.iter()));
    c.bench_function("indexmap::iter", |b| b.iter(|| map2.iter()));

    c.bench_function("std::map::found", |b| b.iter(|| map.get(&55)));
    c.bench_function("indexmap::found", |b| b.iter(|| map2.get(&55)));

    c.bench_function("std::map::notfound", |b| b.iter(|| map.get(&155)));
    c.bench_function("indexmap::notfound", |b| b.iter(|| map2.get(&155)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
