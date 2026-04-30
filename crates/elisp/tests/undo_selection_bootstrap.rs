//! Bootstrap coverage for C-level mark/selection variables used by undo ERT.

use rele_elisp::eval::bootstrap::{load_full_bootstrap, make_stdlib_interp};
use rele_elisp::{Interpreter, LispObject, read};

fn eval(interp: &Interpreter, src: &str) -> LispObject {
    let form = read(src).unwrap_or_else(|err| panic!("read {src:?}: {err}"));
    interp
        .eval(form)
        .unwrap_or_else(|err| panic!("eval {src:?}: {err:?}"))
}

fn assert_symbol_value(interp: &Interpreter, name: &str, expected: LispObject) {
    assert_eq!(
        eval(interp, &format!("(boundp '{name})")),
        LispObject::t(),
        "{name} should be bound"
    );
    assert_eq!(
        eval(interp, &format!("(symbol-value '{name})")),
        expected,
        "{name} has the wrong bootstrap value"
    );
}

#[test]
fn undo_selection_policy_variables_have_emacs_defaults() {
    let interp = make_stdlib_interp();

    for (name, expected) in [
        ("select-active-regions", LispObject::t()),
        ("saved-region-selection", LispObject::nil()),
        ("post-select-region-hook", LispObject::nil()),
        ("tty-select-active-regions", LispObject::nil()),
        ("deactivate-mark", LispObject::nil()),
        ("mark-active", LispObject::nil()),
        ("mark-even-if-inactive", LispObject::t()),
    ] {
        assert_symbol_value(&interp, name, expected);
    }

    assert_symbol_value(
        &interp,
        "selection-inhibit-update-commands",
        read("(handle-switch-frame handle-select-window)").unwrap(),
    );

    assert_eq!(
        eval(&interp, "(functionp region-extract-function)"),
        LispObject::t(),
        "region-extract-function should be callable during mark deactivation"
    );
}

#[test]
fn mark_command_loop_variables_are_special_and_buffer_local() {
    let interp = make_stdlib_interp();

    for name in [
        "select-active-regions",
        "saved-region-selection",
        "selection-inhibit-update-commands",
        "post-select-region-hook",
        "tty-select-active-regions",
        "deactivate-mark",
        "mark-active",
        "mark-even-if-inactive",
        "region-extract-function",
        "region-insert-function",
        "transient-mark-mode",
    ] {
        assert_eq!(
            eval(&interp, &format!("(special-variable-p '{name})")),
            LispObject::t(),
            "{name} should be dynamically scoped"
        );
    }

    for name in ["mark-active", "deactivate-mark"] {
        assert_eq!(
            eval(&interp, &format!("(get '{name} 'variable-buffer-local)")),
            LispObject::t(),
            "{name} should become buffer-local when set"
        );
    }
}

#[test]
fn full_bootstrap_keeps_mark_primitives_wired_to_buffer_state() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(progn
                 (erase-buffer)
                 (transient-mark-mode 1)
                 (insert "abc")
                 (push-mark 1 t t)
                 (and (region-active-p)
                      (= (region-beginning) 1)
                      (= (region-end) 4)))"#,
        ),
        LispObject::t(),
        "full bootstrap should not replace primitive mark state wiring"
    );
}

#[test]
fn undo_restores_insert_delete_groups() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (undo-boundary)
                 (insert "One")
                 (undo-boundary)
                 (insert " Zero")
                 (undo-boundary)
                 (push-mark nil t)
                 (delete-region (save-excursion
                                  (forward-word -1)
                                  (point))
                                (point))
                 (undo-boundary)
                 (beginning-of-line)
                 (insert "Zero")
                 (undo-boundary)
                 (undo)
                 (let ((after-first (buffer-string)))
                   (undo-more 2)
                   (list after-first (buffer-string))))"#,
        )
        .princ_to_string(),
        "(\"One Zero\" \"\")"
    );
}

#[test]
fn undo_restores_unmodified_flag() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (insert "1")
                 (undo-boundary)
                 (set-buffer-modified-p nil)
                 (insert "2")
                 (undo)
                 (not (buffer-modified-p)))"#,
        ),
        LispObject::t()
    );
}

#[test]
fn enable_multibyte_characters_tracks_buffer_state() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (list enable-multibyte-characters
                       (progn
                         (set-buffer-multibyte nil)
                         enable-multibyte-characters)
                       (progn
                         (set-buffer-multibyte t)
                         enable-multibyte-characters)))"#,
        )
        .princ_to_string(),
        "(t nil t)"
    );
}

#[test]
fn primitive_undo_reports_emacs_type_error_data() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(condition-case err
                   (primitive-undo nil nil)
                 (wrong-type-argument err))"#,
        )
        .princ_to_string(),
        "(wrong-type-argument number-or-marker-p nil)"
    );
}

#[test]
fn combine_change_calls_records_one_undo_entry() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (insert "A")
                 (undo-boundary)
                 (insert "B")
                 (undo-boundary)
                 (insert "C")
                 (undo-boundary)
                 (insert " ")
                 (undo-boundary)
                 (insert "D")
                 (undo-boundary)
                 (insert "E")
                 (undo-boundary)
                 (insert "F")
                 (let ((before (length buffer-undo-list)))
                   (goto-char (point-min))
                   (combine-change-calls (point-min) (point-max)
                     (re-search-forward "ABC ")
                     (replace-match "Z "))
                   (list before
                         (length buffer-undo-list)
                         (buffer-string))))"#,
        )
        .princ_to_string(),
        "(14 15 \"Z DEF\")"
    );
}

#[test]
fn marker_adjustments_survive_delete_and_undo() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (insert "abcdefg")
                 (undo-boundary)
                 (let ((m (make-marker)))
                   (set-marker m 2 (current-buffer))
                   (goto-char (point-min))
                   (funcall-interactively 'delete-forward-char 3)
                   (undo-boundary)
                   (let ((after-delete (marker-position m)))
                     (undo)
                     (list after-delete (marker-position m)))))"#,
        )
        .princ_to_string(),
        "(1 2)"
    );
}

#[test]
fn moved_marker_adjustment_is_not_replayed_on_undo() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (insert "abcdefghijk")
                 (undo-boundary)
                 (let ((m (make-marker)))
                   (set-marker m 2 (current-buffer))
                   (goto-char (point-min))
                   (funcall-interactively 'delete-forward-char 3)
                   (undo-boundary)
                   (set-marker m 4)
                   (undo)
                   (marker-position m)))"#,
        ),
        LispObject::integer(7)
    );
}

#[test]
fn insertion_type_marker_is_restored_after_deleted_text_undo() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (buffer-enable-undo)
                 (insert "abcdefg")
                 (undo-boundary)
                 (let ((m (make-marker)))
                   (set-marker-insertion-type m t)
                   (set-marker m (point-min) (current-buffer))
                   (delete-region (point-min) (+ 2 (point-min)))
                   (undo-boundary)
                   (let ((after-delete (marker-position m)))
                     (undo)
                     (list after-delete
                           (marker-position m)
                           (marker-insertion-type m)))))"#,
        )
        .princ_to_string(),
        "(1 1 t)"
    );
}

#[test]
fn forward_char_updates_stub_buffer_point() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (insert "abcd")
                 (goto-char 2)
                 (forward-char 2)
                 (point))"#,
        ),
        LispObject::integer(4)
    );
}

#[test]
fn goto_char_returns_stub_buffer_point() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (insert "abc")
                 (goto-char 2))"#,
        ),
        LispObject::integer(2)
    );
}

#[test]
fn char_equal_respects_dynamic_case_fold_search() {
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(list (let ((case-fold-search nil)) (char-equal ?a ?A))
                     (let ((case-fold-search t)) (char-equal ?a ?A)))"#,
        )
        .princ_to_string(),
        "(nil t)"
    );
}

#[test]
fn gap_position_tracks_stub_buffer_point() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (insert "abc")
                 (goto-char 2)
                 (insert "Z")
                 (list (gap-position) (gap-size) (point)))"#,
        )
        .princ_to_string(),
        "(3 0 3)"
    );
}

#[test]
fn compare_buffer_substrings_reports_signed_mismatch_position() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(let ((buf1 (generate-new-buffer " *cmp-1*"))
                     (buf2 (generate-new-buffer " *cmp-2*")))
                 (unwind-protect
                     (progn
                       (with-current-buffer buf1 (insert "abc"))
                       (with-current-buffer buf2 (insert "abd"))
                       (list (compare-buffer-substrings buf1 1 4 buf1 1 4)
                             (compare-buffer-substrings buf1 1 4 buf2 1 4)))
                   (kill-buffer buf1)
                   (kill-buffer buf2)))"#,
        )
        .princ_to_string(),
        "(0 -3)"
    );
}

#[test]
fn subst_char_in_region_rewrites_stub_buffer_text() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (insert "ababa")
                 (subst-char-in-region (point-min) (point-max) ?b ?x)
                 (buffer-string))"#,
        ),
        LispObject::string("axaxa")
    );
}

#[test]
fn replace_region_contents_preserves_surrounding_markers_and_point() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    assert_eq!(
        eval(
            &interp,
            r#"(with-temp-buffer
                 (insert "here is some text")
                 (let ((m5n (copy-marker 6 nil))
                       (m5a (copy-marker 6 t))
                       (m6n (copy-marker 7 nil))
                       (m6a (copy-marker 7 t))
                       (m7n (copy-marker 8 nil))
                       (m7a (copy-marker 8 t)))
                   (replace-region-contents 6 8 "be")
                   (list (buffer-string)
                         (point)
                         (marker-position m5n)
                         (marker-position m5a)
                         (marker-position m6n)
                         (<= 6 (marker-position m6a) 8)
                         (marker-position m7n)
                         (marker-position m7a))))"#,
        )
        .princ_to_string(),
        "(\"here be some text\" 18 6 6 6 t 8 8)"
    );
}
