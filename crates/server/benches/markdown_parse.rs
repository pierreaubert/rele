//! Markdown parse benchmarks.
//!
//! The preview pane re-parses on every edit that flags the preview dirty;
//! keeping this fast is critical for responsive editing in large docs.
//!
//! Run with `cargo bench -p rele-server --bench markdown_parse`.

use std::hint::black_box;

use comrak::Arena;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rele_server::markdown::parse_markdown;

fn fixture(n_sections: usize) -> String {
    let mut s = String::new();
    for i in 0..n_sections {
        s.push_str(&format!("# Heading {i}\n\n"));
        s.push_str("A paragraph with *italic*, **bold**, and `code` spans.\n\n");
        s.push_str("- list item one\n- list item two\n- list item three\n\n");
        s.push_str("```rust\nfn main() { println!(\"hi\"); }\n```\n\n");
        s.push_str("> blockquote with inline `code` and a [link](https://example.com).\n\n");
        s.push_str("| col1 | col2 |\n| ---- | ---- |\n| a    | b    |\n\n");
    }
    s
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_markdown");
    for &n in &[10usize, 100, 1_000] {
        let text = fixture(n);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &text, |b, text| {
            b.iter(|| {
                let arena = Arena::new();
                let _ = black_box(parse_markdown(&arena, text));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
