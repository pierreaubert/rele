//! R24 — regression tests for the dominant "wrong type argument:
//! expected number" cluster in the round-2 baseline (70 hits in
//! `test/src/buffer-tests.el`).
//!
//! Root cause: overlay primitives (`make-overlay`, `overlay-start`,
//! `overlay-end`, `move-overlay`, `overlay-buffer`, `overlay-put`, ...)
//! were stubbed to return `nil`. Test macros like `deftest-moving-insert-1`
//! compare the positions with integers via `(= N (overlay-start ov))`,
//! so every one of those 70 variants tripped "expected number" on nil.
//!
//! Fix: back the overlay primitives with a real `buffer::Registry::overlays`
//! table and expose them as `(overlay . <id>)` cons cells. Start/end
//! now return honest integers, `move-overlay` mutates them, and
//! `delete-overlay` marks the entry detached without dropping it
//! (so `overlayp` still holds).

use rele_elisp::{add_primitives, read, Interpreter, LispObject};

fn interp() -> Interpreter {
    let mut i = Interpreter::new();
    add_primitives(&mut i);
    i
}

fn eval_in(interp: &Interpreter, src: &str) -> LispObject {
    let form = read(src).expect("reader");
    interp
        .eval(form)
        .unwrap_or_else(|e| panic!("eval of {src}: {e:?}"))
}

/// Round-trip: `make-overlay` + `overlay-start` / `overlay-end` return
/// the original positions. Previously returned `nil`.
#[test]
fn r24_make_overlay_returns_positions() {
    let i = interp();
    eval_in(&i, "(setq ov (make-overlay 10 20))");
    assert_eq!(
        eval_in(&i, "(overlay-start ov)").as_integer(),
        Some(10),
        "overlay-start must return 10"
    );
    assert_eq!(
        eval_in(&i, "(overlay-end ov)").as_integer(),
        Some(20),
        "overlay-end must return 20"
    );
}

/// Reduced repro of `test-move-overlay-1`. The test body:
///     (move-overlay ov 50 60)
///     (should (= 50 (overlay-start ov)))
///     (should (= 60 (overlay-end ov)))
/// was one of the 70 failures in buffer-tests.el because
/// `overlay-start` returned nil, so `=` signalled "expected number".
#[test]
fn r24_move_overlay_matches_test_move_overlay_1() {
    let i = interp();
    let result = eval_in(
        &i,
        "(progn (setq ov (make-overlay 1 100))
                (move-overlay ov 50 60)
                (list (overlay-start ov) (overlay-end ov)))",
    );
    let xs: Vec<_> = {
        let mut out = Vec::new();
        let mut cur = result;
        while let Some((car, cdr)) = cur.destructure_cons() {
            out.push(car);
            cur = cdr;
        }
        out
    };
    assert_eq!(xs.len(), 2);
    assert_eq!(xs[0].as_integer(), Some(50));
    assert_eq!(xs[1].as_integer(), Some(60));
}

/// Reduced repro of `deftest-moving-insert-1 A`. The macro expands to
///     (test-with-overlay-in-buffer (ov 10 20)
///       (should (= 10 (overlay-start ov)))
///       (should (= 20 (overlay-end ov))))
/// Before R24, all 30+ A..f variants hit "expected number" on the
/// first `=`.
#[test]
fn r24_equal_predicate_on_overlay_positions() {
    let i = interp();
    eval_in(&i, "(setq ov (make-overlay 10 20))");
    assert!(
        !eval_in(&i, "(= 10 (overlay-start ov))").is_nil(),
        "(= 10 (overlay-start ov)) must be t"
    );
    assert!(
        !eval_in(&i, "(= 20 (overlay-end ov))").is_nil(),
        "(= 20 (overlay-end ov)) must be t"
    );
}

/// `overlayp` now returns `t` for real overlays, `nil` otherwise —
/// previously stubbed to `ignore` and returned nil unconditionally.
#[test]
fn r24_overlayp_distinguishes_overlays() {
    let i = interp();
    assert!(
        !eval_in(&i, "(overlayp (make-overlay 1 5))").is_nil(),
        "overlayp on a real overlay must be t"
    );
    assert!(
        eval_in(&i, "(overlayp nil)").is_nil(),
        "overlayp on nil must be nil"
    );
    assert!(
        eval_in(&i, "(overlayp '(1 . 2))").is_nil(),
        "overlayp on plain cons must be nil"
    );
    assert!(
        eval_in(&i, "(overlayp 42)").is_nil(),
        "overlayp on integer must be nil"
    );
}

/// `delete-overlay` detaches: start/end become nil but `overlayp`
/// still holds (matches real Emacs).
#[test]
fn r24_delete_overlay_detaches_but_keeps_identity() {
    let i = interp();
    eval_in(&i, "(setq ov (make-overlay 1 5))");
    eval_in(&i, "(delete-overlay ov)");
    assert!(
        !eval_in(&i, "(overlayp ov)").is_nil(),
        "overlayp on a deleted overlay must still be t"
    );
    assert!(
        eval_in(&i, "(overlay-start ov)").is_nil(),
        "start of detached overlay must be nil"
    );
}

/// `overlay-put` + `overlay-get` round-trip a property. Previously
/// `overlay-put` returned nil and `overlay-get` always returned nil,
/// so any property-based test failed immediately.
#[test]
fn r24_overlay_put_get_roundtrip() {
    let i = interp();
    let v = eval_in(
        &i,
        "(progn (setq ov (make-overlay 1 5))
                (overlay-put ov 'face 'highlight)
                (overlay-get ov 'face))",
    );
    assert_eq!(
        v.as_symbol().as_deref(),
        Some("highlight"),
        "overlay-put/get must round-trip"
    );
}
