// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use costree::scanner::{self, SearchOptions};
use criterion::{Criterion, criterion_group, criterion_main};

/// A flat index of `n` synthetic entries spread across 1000 directories,
/// roughly the shape issue #1 was worried about (large real `$HOME` trees
/// scan to 500k-1M+ entries).
fn build_index(n: usize) -> scanner::SearchIndex {
    (0..n)
        .map(|i| {
            let name = format!("file_{i}.txt");
            let path = PathBuf::from(format!("/synthetic/dir_{}/{name}", i % 1000));
            (path, name)
        })
        .collect()
}

fn search_benchmark(c: &mut Criterion) {
    let index = build_index(500_000);

    // Matches a small, roughly-constant-size subset regardless of index
    // size — isolates the per-entry matching cost from the ancestor-walk
    // cost, since only a handful of ancestors ever get inserted. Uncapped:
    // with so few matches, the cap never has anything to do.
    let narrow = scanner::compile_search_regex("file_999", SearchOptions::default()).unwrap();
    c.bench_function("search_narrow_500k", |b| {
        b.iter(|| scanner::search_index(std::hint::black_box(&index), &narrow, usize::MAX));
    });

    // Matches every entry. Uncapped, this is the worst case for the
    // ancestor-walk path — layered on top of (not instead of) the matching
    // cost above. Capped at MAX_SEARCH_RESULTS, it's what actually runs in
    // the app: the view only ever renders that many rows regardless, so
    // this is the number that matters for how search *feels*.
    let broad = scanner::compile_search_regex("file_", SearchOptions::default()).unwrap();
    c.bench_function("search_broad_500k_uncapped", |b| {
        b.iter(|| scanner::search_index(std::hint::black_box(&index), &broad, usize::MAX));
    });
    c.bench_function("search_broad_500k_capped", |b| {
        b.iter(|| scanner::search_index(std::hint::black_box(&index), &broad, scanner::MAX_SEARCH_RESULTS));
    });
}

criterion_group!(benches, search_benchmark);
criterion_main!(benches);
