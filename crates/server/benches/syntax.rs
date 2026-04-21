//! Syntax-highlighting benchmarks.
//!
//! Tree-sitter parse-from-scratch on various buffer sizes, plus the
//! visible-range query cost (what runs per render).
//!
//! Run with `cargo bench -p rele-server --bench syntax`.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rele_server::DocumentBuffer;
use rele_server::syntax::{Highlighter, TreeSitterHighlighter, TsLanguage};
use ropey::Rope;

fn markdown_fixture(n_sections: usize) -> Rope {
    let mut s = String::new();
    for i in 0..n_sections {
        s.push_str(&format!("# Heading {i}\n\n"));
        s.push_str("A paragraph with *italic*, **bold**, and `code`.\n\n");
        s.push_str("- list item one\n- list item two\n\n");
        s.push_str("```rust\nfn main() { let x = 1; }\n```\n\n");
        s.push_str("> a quote with [a link](https://example.com).\n\n");
    }
    Rope::from_str(&s)
}

fn rust_fixture(n_fns: usize) -> Rope {
    let mut s = String::new();
    s.push_str("use std::collections::HashMap;\nuse std::io;\n\n");
    for i in 0..n_fns {
        s.push_str(&format!(
            "/// Function {i}.\npub fn func_{i}(x: u32, y: &str) -> Option<String> {{\n    let s = format!(\"{{}}-{{}}\", x, y);\n    if x > 0 {{ Some(s) }} else {{ None }}\n}}\n\n"
        ));
    }
    Rope::from_str(&s)
}

fn bench_parse_markdown_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_parse_markdown_cold");
    for &n in &[10usize, 100, 1_000] {
        let rope = markdown_fixture(n);
        group.throughput(Throughput::Bytes(rope.len_bytes() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut h = TreeSitterHighlighter::new(TsLanguage::Markdown).unwrap();
                h.on_edit(&rope, &[], true);
                black_box(&h);
            });
        });
    }
    group.finish();
}

fn bench_parse_rust_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_parse_rust_cold");
    for &n in &[10usize, 100, 1_000] {
        let rope = rust_fixture(n);
        group.throughput(Throughput::Bytes(rope.len_bytes() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
                h.on_edit(&rope, &[], true);
                black_box(&h);
            });
        });
    }
    group.finish();
}

fn bench_highlight_range_warm(c: &mut Criterion) {
    // After one parse, repeated viewport queries should be cheap.
    let mut group = c.benchmark_group("tree_sitter_highlight_range_warm");
    for &n in &[100usize, 1_000] {
        let rope = markdown_fixture(n);
        let mut h = TreeSitterHighlighter::new(TsLanguage::Markdown).unwrap();
        h.on_edit(&rope, &[], true);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            // Query a typical viewport of 80 lines.
            b.iter(|| {
                let ranges = h.highlight_range(&rope, 0, 80);
                black_box(ranges);
            });
        });
    }
    group.finish();
}

/// Measures the cost of a single-character *incremental* reparse after a
/// previous parse. This is the per-keystroke cost we actually pay during
/// interactive editing — cold parse only happens on buffer open.
fn bench_incremental_parse_rust(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_incremental_rust_insert_char");
    for &n in &[100usize, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_batched(
                || {
                    let mut source = String::new();
                    for i in 0..n {
                        source.push_str(&format!(
                            "/// Function {i}.\npub fn func_{i}(x: u32) -> u32 {{ x + 1 }}\n\n"
                        ));
                    }
                    let mut doc = DocumentBuffer::from_text(&source);
                    let _ = doc.drain_changes();
                    let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
                    h.on_edit(doc.rope(), &[], true);
                    (doc, h)
                },
                |(mut doc, mut h)| {
                    let pos = doc.len_chars() / 2;
                    doc.insert(pos, "x");
                    let (changes, full) = doc.drain_changes();
                    h.on_edit(doc.rope(), &changes, full);
                    black_box(&h);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_incremental_parse_markdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_incremental_markdown_insert_char");
    for &n in &[100usize, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_batched(
                || {
                    let mut source = String::new();
                    for i in 0..n {
                        source
                            .push_str(&format!("# Heading {i}\n\nA paragraph with *italic*.\n\n"));
                    }
                    let mut doc = DocumentBuffer::from_text(&source);
                    let _ = doc.drain_changes();
                    let mut h = TreeSitterHighlighter::new(TsLanguage::Markdown).unwrap();
                    h.on_edit(doc.rope(), &[], true);
                    (doc, h)
                },
                |(mut doc, mut h)| {
                    let pos = doc.len_chars() / 2;
                    doc.insert(pos, "x");
                    let (changes, full) = doc.drain_changes();
                    h.on_edit(doc.rope(), &changes, full);
                    black_box(&h);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_markdown_cold,
    bench_parse_rust_cold,
    bench_highlight_range_warm,
    bench_incremental_parse_rust,
    bench_incremental_parse_markdown,
);
criterion_main!(benches);
