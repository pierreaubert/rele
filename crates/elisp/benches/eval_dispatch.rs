//! Benchmarks for the eval dispatch and special-form path.
//!
//! The costly part of `eval_inner` is the per-call string-ification of
//! the head symbol and the 200+-arm match that follows. These benches
//! measure shapes that exercise that path predominantly — they're the
//! targets of Phase A's dispatch-table refactor.
//!
//! Run with `cargo bench -p rele-elisp --bench eval_dispatch`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rele_elisp::{Interpreter, add_primitives, read};

/// A fresh interpreter with core primitives loaded. Created once per
/// bench group; each `iter` re-reads and re-evals the form.
fn fresh_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn bench_quote(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(quote (a b c))").unwrap();
    c.bench_function("eval_dispatch/quote", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_if_true(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(if t 1 2)").unwrap();
    c.bench_function("eval_dispatch/if_true", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_progn_three(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(progn 1 2 3)").unwrap();
    c.bench_function("eval_dispatch/progn_three", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_let_one(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(let ((x 1)) x)").unwrap();
    c.bench_function("eval_dispatch/let_one", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_cond_match_first(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(cond (t 1) (nil 2) (nil 3))").unwrap();
    c.bench_function("eval_dispatch/cond_match_first", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

criterion_group!(
    benches,
    bench_quote,
    bench_if_true,
    bench_progn_three,
    bench_let_one,
    bench_cond_match_first,
);
criterion_main!(benches);
