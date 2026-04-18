//! Buffer search benchmarks.
//!
//! Exercises the literal and regex search paths used by isearch.
//!
//! Run with `cargo bench -p rele-server --bench search`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rele_server::emacs::search::{MatchData, SearchDirection, re_search, search_literal};
use rele_server::{DocumentBuffer, EditorCursor};

fn fixture(n_lines: usize, needle_every: usize) -> DocumentBuffer {
    let mut s = String::new();
    for i in 0..n_lines {
        if i % needle_every == 0 {
            s.push_str("xyzNEEDLExyz this is the marked line\n");
        } else {
            s.push_str("The quick brown fox jumps over the lazy dog.\n");
        }
    }
    DocumentBuffer::from_text(&s)
}

fn bench_literal_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_literal_forward");
    for &size in &[1_000usize, 10_000, 100_000] {
        let doc = fixture(size, 101);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched_ref(
                || (EditorCursor::new(), MatchData::default()),
                |(cursor, match_data)| {
                    cursor.position = 0;
                    let _ = black_box(search_literal(
                        "NEEDLE",
                        &doc,
                        cursor,
                        SearchDirection::Forward,
                        None,
                        match_data,
                    ));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_literal_backward(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_literal_backward");
    for &size in &[1_000usize, 10_000, 100_000] {
        let doc = fixture(size, 101);
        let end = doc.len_chars();
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched_ref(
                || (EditorCursor::new(), MatchData::default()),
                |(cursor, match_data)| {
                    cursor.position = end;
                    let _ = black_box(search_literal(
                        "NEEDLE",
                        &doc,
                        cursor,
                        SearchDirection::Backward,
                        None,
                        match_data,
                    ));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_regex_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("re_search_forward");
    for &size in &[1_000usize, 10_000, 100_000] {
        let doc = fixture(size, 101);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched_ref(
                || (EditorCursor::new(), MatchData::default()),
                |(cursor, match_data)| {
                    cursor.position = 0;
                    let _ = black_box(re_search(
                        r"NEEDLE\S*",
                        &doc,
                        cursor,
                        SearchDirection::Forward,
                        None,
                        match_data,
                    ));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_literal_forward,
    bench_literal_backward,
    bench_regex_forward,
);
criterion_main!(benches);
