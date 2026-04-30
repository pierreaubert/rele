use rele_elisp::Interpreter;
use rele_elisp::eval::bootstrap::make_stdlib_interp;

fn eval_princ(interp: &Interpreter, source: &str) -> String {
    interp
        .eval_source(source)
        .unwrap_or_else(|err| panic!("eval({source}) failed: {err:?}"))
        .princ_to_string()
}

#[test]
fn headless_window_display_primitives_survive_bootstrap() {
    let interp = make_stdlib_interp();
    let cases = [
        ("(force-mode-line-update)", "nil"),
        ("(display-graphic-p)", "nil"),
        ("(display-color-p)", "nil"),
        ("(display-images-p)", "nil"),
        ("(display-popup-menus-p)", "nil"),
        ("(window-prev-buffers)", "nil"),
        ("(window-dedicated-p)", "nil"),
        ("(windowp (select-window (selected-window)))", "t"),
        ("(frame-char-width)", "10"),
        ("(frame-char-height)", "20"),
        ("(window-pixel-width)", "800"),
        ("(frame-pixel-height)", "480"),
        ("(window-pixel-edges)", "(0 0 800 480)"),
        ("(window-edges)", "(0 0 80 24)"),
        ("(windowp (minibuffer-window))", "t"),
        ("(active-minibuffer-window)", "nil"),
        ("(x-display-list)", "nil"),
        ("(terminal-name)", "initial_terminal"),
        ("(terminal-live-p nil)", "t"),
        ("(frame-initial-p (car (terminal-list)))", "t"),
    ];

    for (source, expected) in cases {
        assert_eq!(eval_princ(&interp, source), expected, "{source}");
    }
}

#[test]
fn window_dedication_round_trips_on_single_headless_window() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (set-window-dedicated-p (selected-window) t)
               (window-dedicated-p))",
        ),
        "t",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (set-window-dedicated-p (selected-window) nil)
               (window-dedicated-p))",
        ),
        "nil",
    );
}

#[test]
fn frame_or_buffer_changed_tracks_headless_state() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(let ((state nil))
               (list (frame-or-buffer-changed-p 'state)
                     (vectorp state)
                     (frame-or-buffer-changed-p 'state)))",
        ),
        "(t t nil)",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(let ((state nil))
               (frame-or-buffer-changed-p 'state)
               (get-buffer-create \"frame-buffer-state-test\")
               (frame-or-buffer-changed-p 'state))",
        ),
        "t",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(let ((state nil))
               (frame-or-buffer-changed-p 'state)
               (with-temp-buffer
                 (frame-or-buffer-changed-p 'state)))",
        ),
        "nil",
    );
}

#[test]
fn window_text_pixel_size_uses_virtual_cell_metrics() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (erase-buffer)
               (insert \"abc\\ndefgh\")
               (window-text-pixel-size))",
        ),
        "(50 . 40)",
    );
    assert_eq!(
        eval_princ(&interp, "(window-text-pixel-size nil 1 4)"),
        "(30 . 20)",
    );
    assert_eq!(
        eval_princ(&interp, "(window-text-pixel-size nil nil nil 25 10)"),
        "(25 . 10)",
    );
}

#[test]
fn window_start_and_visibility_are_deterministic() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (erase-buffer)
               (insert \"abcdef\")
               (set-window-start (selected-window) 3)
               (window-start))",
        ),
        "3",
    );
    assert_eq!(eval_princ(&interp, "(pos-visible-in-window-p 2)"), "nil");
    assert_eq!(eval_princ(&interp, "(pos-visible-in-window-p 3)"), "t");

    let _ = interp.eval_source("(set-window-start (selected-window) 1)");
}

#[test]
fn bidi_override_scan_reports_buffer_position() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (erase-buffer)
               (insert \"int main() {
  bool isAdmin = false;
  /*\")
               (insert (string #x202e))
               (insert \" }\")
               (insert (string #x2066))
               (insert \"if (isAdmin)\")
               (insert (string #x2069))
               (insert \" \")
               (insert (string #x2066))
               (insert \" begin admins only */
  printf(\\\"You are an admin.\\\\n\\\");
  /* end admins only \")
               (insert (string #x202e))
               (insert \" { \")
               (insert (string #x2066))
               (insert \"*/
  return 0;
}\")
               (bidi-find-overridden-directionality (point-min) (point-max) nil))",
        ),
        "46",
    );
}

#[test]
fn font_spec_parses_common_font_names() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(font-get (font-spec :name \"Foo-12:weight=bold\") :family)"
        ),
        "Foo",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(font-get (font-spec :name \"Foo-12:weight=bold\") :size)"
        ),
        "12.0",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(font-get (font-spec :name \"Bar Semi-Bold Italic 10\") :weight)"
        ),
        "semi-bold",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(font-get (font-spec :name \"-GNU -FreeSans-semibold-italic-normal-*-*-*-*-*-*-0-iso10646-1\") :foundry)",
        ),
        "GNU ",
    );
}

#[test]
fn get_display_property_reads_display_specs() {
    let interp = make_stdlib_interp();
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (erase-buffer)
               (insert (propertize \"foo\" 'display '(height 2.0)))
               (get-display-property 2 'height))",
        ),
        "2.0",
    );
    assert_eq!(
        eval_princ(
            &interp,
            "(progn
               (erase-buffer)
               (insert (propertize \"foo\" 'display '((height 2.0) (space-width 20))))
               (get-display-property 2 'space-width))",
        ),
        "20",
    );
}
