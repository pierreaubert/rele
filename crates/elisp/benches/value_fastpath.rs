//! Benchmarks for the Value-native fast path in `primitives_value.rs`.
//!
//! These shapes are the target of Phase B's fast-path expansion: `car`,
//! `cdr`, `consp`, `stringp`, etc. are now dispatched without an
//! `obj_to_value` round-trip. Before / after numbers belong in
//! CHANGELOG.md under Performance baseline.
//!
//! Run with `cargo bench -p rele-elisp --bench value_fastpath`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use rele_elisp::{add_primitives, read, Interpreter};

fn fresh_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn bench_arith_plus(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(+ 1 2 3)").unwrap();
    c.bench_function("value_fastpath/arith_plus", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_cmp_lt(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(< 1 2 3)").unwrap();
    c.bench_function("value_fastpath/cmp_lt", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

fn bench_type_integerp(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(integerp 42)").unwrap();
    c.bench_function("value_fastpath/type_integerp", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

/// `car` now fast-paths — this bench should move after Phase B lands.
fn bench_car(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(car '(1 2 3))").unwrap();
    c.bench_function("value_fastpath/car", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

/// `cdr` — same.
fn bench_cdr(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(cdr '(1 2 3))").unwrap();
    c.bench_function("value_fastpath/cdr", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

/// `consp` type predicate over a cons literal.
fn bench_consp(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(consp '(1))").unwrap();
    c.bench_function("value_fastpath/consp", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

/// `symbolp` on a quoted symbol — should hit the fast path.
fn bench_symbolp(c: &mut Criterion) {
    let interp = fresh_interp();
    let form = read("(symbolp 'foo)").unwrap();
    c.bench_function("value_fastpath/symbolp", |b| {
        b.iter(|| {
            let r = interp.eval(black_box(form.clone())).unwrap();
            black_box(r);
        });
    });
}

criterion_group!(
    benches,
    bench_arith_plus,
    bench_cmp_lt,
    bench_type_integerp,
    bench_car,
    bench_cdr,
    bench_consp,
    bench_symbolp,
);
criterion_main!(benches);
