//! R22 regression: `overlays-at` returned a nil list for overlays that
//! the Emacs `buffer-tests--overlays-at-1-{B,C,D,…}` fixtures expected
//! to be present, because `make-overlay` / `overlays-at` were pure
//! nil-returning stubs.
//!
//! The round-2 baseline showed 31 failures of
//! `((equal (length ovl) (length (quote (a)))))` — all reducing to the
//! same root cause: `(overlays-at POINT)` returned `nil` instead of a
//! one-element list containing the overlay previously created with
//! `(make-overlay 10 20)`.
//!
//! Fixture (reduced from `test/src/buffer-tests.el`):
//! ```elisp
//! (with-temp-buffer
//!   (insert (make-string 100 ?\s))
//!   (overlay-put (make-overlay 10 20) 'tag 'a)
//!   (overlays-at POINT))
//! ```
//! Emacs semantics: an overlay `[S, E)` covers position `P` iff
//! `S <= P < E`. So `POINT` = 10, 15, 19 all match; `POINT` = 20 does
//! not. These correspond to the -B / -C / -D variants (and -E).

use rele_elisp::{Interpreter, LispObject, add_primitives, primitives_modules, read};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    primitives_modules::register(&mut interp);
    interp
}

fn eval_int(interp: &Interpreter, src: &str) -> i64 {
    let expr = read(src).unwrap_or_else(|e| panic!("read {src:?}: {e:?}"));
    let val = interp
        .eval(expr)
        .unwrap_or_else(|e| panic!("eval {src:?}: {e:?}"));
    val.as_integer()
        .unwrap_or_else(|| panic!("expected integer from {src:?}, got {val:?}"))
}

/// Evaluates the full `buffer-tests--overlays-at-1-{id}` fixture and
/// returns `(length (overlays-at POINT))`.
fn length_at(point: i64) -> i64 {
    let interp = make_interp();
    let src = format!(
        "(with-temp-buffer
           (insert (make-string 100 ?\\s))
           (overlay-put (make-overlay 10 20) (quote tag) (quote a))
           (length (overlays-at {point})))"
    );
    eval_int(&interp, &src)
}

#[test]
fn r22_overlays_at_variant_b_point_10_returns_length_1() {
    // B: point = start of the overlay's range; the overlay covers it.
    assert_eq!(length_at(10), 1);
}

#[test]
fn r22_overlays_at_variant_c_point_15_returns_length_1() {
    // C: point in the interior; obvious hit.
    assert_eq!(length_at(15), 1);
}

#[test]
fn r22_overlays_at_variant_d_point_19_returns_length_1() {
    // D: point at (end - 1); last covered position.
    assert_eq!(length_at(19), 1);
}

#[test]
fn r22_overlays_at_variant_e_point_20_returns_length_0() {
    // E (boundary check that protects against off-by-one going the
    // other way): point = end is *not* covered by a non-empty overlay.
    assert_eq!(length_at(20), 0);
}

#[test]
fn r22_overlays_at_variant_a_no_overlay_returns_length_0() {
    // A: buffer has text but no overlay created → always empty.
    let interp = make_interp();
    let src = "(with-temp-buffer
                 (insert (make-string 100 ?\\s))
                 (length (overlays-at 1)))";
    assert_eq!(eval_int(&interp, src), 0);
}

#[test]
fn r22_overlays_at_preserves_tag_via_overlay_get() {
    // The fixture also calls `(overlay-get ov 'tag)` on the result.
    // Make sure that round-trips through `overlay-put`.
    let interp = make_interp();
    let src = "(with-temp-buffer
                 (insert (make-string 100 ?\\s))
                 (overlay-put (make-overlay 10 20) (quote tag) (quote a))
                 (let ((ovl (overlays-at 15)))
                   (overlay-get (car ovl) (quote tag))))";
    let expr = read(src).unwrap();
    let val = interp.eval(expr).unwrap();
    assert_eq!(val, LispObject::symbol("a"));
}

#[test]
fn r22_overlayp_recognises_make_overlay_result() {
    let interp = make_interp();
    let src = "(with-temp-buffer
                 (insert (make-string 10 ?\\s))
                 (overlayp (make-overlay 1 5)))";
    let expr = read(src).unwrap();
    let val = interp.eval(expr).unwrap();
    assert_eq!(val, LispObject::t());
}

#[test]
fn r22_overlay_start_and_end_return_positions() {
    let interp = make_interp();
    let src = "(with-temp-buffer
                 (insert (make-string 100 ?\\s))
                 (let ((ov (make-overlay 10 20)))
                   (list (overlay-start ov) (overlay-end ov))))";
    let expr = read(src).unwrap();
    let val = interp.eval(expr).unwrap();
    assert_eq!(
        val,
        LispObject::cons(
            LispObject::integer(10),
            LispObject::cons(LispObject::integer(20), LispObject::nil()),
        )
    );
}

#[test]
fn r22_overlays_in_spans_range() {
    // Mirrors deftest-overlays-in-1 G (a 10 20): range (5, 25) includes
    // the overlay.
    let interp = make_interp();
    let src = "(with-temp-buffer
                 (insert (make-string 100 ?\\s))
                 (overlay-put (make-overlay 10 20) (quote tag) (quote a))
                 (length (overlays-in 5 25)))";
    assert_eq!(eval_int(&interp, src), 1);
}
