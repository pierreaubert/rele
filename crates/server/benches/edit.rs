//! Document edit benchmarks.
//!
//! Measures the cost of the operations that run on every keystroke:
//! single-char insert, large-paste insert, remove, drain_changes, snapshot.
//! Also benchmarks cached vs. uncached `word_count`.
//!
//! Run with `cargo bench -p rele-server --bench edit`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rele_server::DocumentBuffer;

/// Build a buffer of `n` lines of Lorem-ish content.
fn fixture(n_lines: usize) -> DocumentBuffer {
    let line = "The quick brown fox jumps over the lazy dog.\n";
    let mut s = String::with_capacity(line.len() * n_lines);
    for _ in 0..n_lines {
        s.push_str(line);
    }
    DocumentBuffer::from_text(&s)
}

fn bench_insert_single_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single_char");
    for &size in &[100usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut buf = fixture(size);
            let mut pos = buf.len_chars() / 2;
            b.iter(|| {
                buf.insert(black_box(pos), black_box("x"));
                pos += 1;
            });
        });
    }
    group.finish();
}

fn bench_insert_paste(c: &mut Criterion) {
    let paste_10kb: String = "a".repeat(10_000);
    let mut group = c.benchmark_group("insert_paste_10kb");
    for &size in &[100usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut buf = fixture(size);
            let pos = buf.len_chars() / 2;
            b.iter(|| {
                buf.insert(black_box(pos), black_box(&paste_10kb));
            });
        });
    }
    group.finish();
}

fn bench_remove_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_range_100");
    for &size in &[100usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut buf = fixture(size);
            let start = buf.len_chars() / 2;
            let mut end = start + 100;
            b.iter(|| {
                // Re-insert every iteration so the buffer doesn't shrink away.
                buf.insert(start, &"x".repeat(100));
                buf.remove(black_box(start), black_box(end));
                let _ = end;
                end = start + 100;
            });
        });
    }
    group.finish();
}

fn bench_drain_changes(c: &mut Criterion) {
    // Simulates what happens between LSP syncs: N edits accumulate, then
    // drain_changes emits the journal for the server.
    c.bench_function("drain_changes_100_edits", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let mut buf = fixture(1_000);
                let base = buf.len_chars() / 2;
                for i in 0..100 {
                    buf.insert(base + i, "x");
                }
                let _ = black_box(buf.drain_changes());
            }
            start.elapsed()
        });
    });
}

fn bench_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_clone");
    for &size in &[100usize, 10_000, 100_000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let buf = fixture(size);
            b.iter(|| {
                let _ = black_box(buf.snapshot());
            });
        });
    }
    group.finish();
}

fn bench_word_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("word_count");
    for &size in &[100usize, 10_000, 100_000] {
        // "Cold" — fresh buffer every iteration, cache cannot be consulted.
        group.bench_with_input(
            BenchmarkId::new("cold", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || fixture(size),
                    |buf| black_box(buf.word_count()),
                    criterion::BatchSize::SmallInput,
                );
            },
        );
        // "Warm" — same buffer, so every iteration hits the cache.
        group.bench_with_input(
            BenchmarkId::new("warm", size),
            &size,
            |b, &size| {
                let buf = fixture(size);
                buf.word_count(); // prime cache
                b.iter(|| black_box(buf.word_count()));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_single_char,
    bench_insert_paste,
    bench_remove_range,
    bench_drain_changes,
    bench_snapshot,
    bench_word_count,
);
criterion_main!(benches);
