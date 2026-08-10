// SPDX-License-Identifier: MIT

use std::path::Path;

use costree::scanner;
use criterion::{Criterion, criterion_group, criterion_main};

/// Builds a synthetic tree under `root`: `breadth` subdirectories per level,
/// `depth` levels deep, each directory holding `files_per_dir` small files.
/// Sized to be big enough that parallel scanning actually shows up in the
/// numbers, but small enough to build (and scan) in well under a second on
/// CI hardware.
fn build_tree(root: &Path, depth: u32, breadth: u32, files_per_dir: u32) {
    std::fs::create_dir_all(root).expect("create synthetic dir");

    for i in 0..files_per_dir {
        std::fs::write(root.join(format!("file_{i}.bin")), [0u8; 256]).expect("write synthetic file");
    }

    if depth == 0 {
        return;
    }

    for i in 0..breadth {
        build_tree(&root.join(format!("dir_{i}")), depth - 1, breadth, files_per_dir);
    }
}

fn scan_benchmark(c: &mut Criterion) {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    build_tree(tempdir.path(), 3, 5, 5);

    let generation = scanner::next_generation();

    c.bench_function("scan_synthetic_tree", |b| {
        b.iter(|| scanner::scan(std::hint::black_box(tempdir.path()), generation));
    });
}

criterion_group!(benches, scan_benchmark);
criterion_main!(benches);
