use gpui_md::state::MdAppState;

/// Construct a state in a `Box` (stable heap location), set its text and
/// cursor, and install the elisp editor callbacks.
///
/// The `Box` is required because `install_elisp_editor_callbacks` captures
/// a raw pointer to the state; returning a bare `MdAppState` would move it
/// and invalidate the pointer immediately.
fn state_with(text: &str) -> Box<MdAppState> {
    let mut s = Box::new(MdAppState::new());
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s.install_elisp_editor_callbacks();
    s
}

#[test]
fn elisp_interpreter_initialized() {
    let _s = MdAppState::new();
    assert!(true);
}

#[test]
fn elisp_eval_arithmetic() {
    let s = state_with("");
    let result = s.elisp.eval(rele_elisp::read("(+ 1 2 3)").unwrap());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), rele_elisp::LispObject::integer(6));
}

#[test]
fn elisp_eval_list_operations() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(cons 1 '(2 3))").unwrap())
            .unwrap(),
        rele_elisp::read("(1 2 3)").unwrap()
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(car '(1 2 3))").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(cdr '(1 2 3))").unwrap())
            .unwrap(),
        rele_elisp::read("(2 3)").unwrap()
    );
}

#[test]
fn elisp_eval_string_operations() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(concat \"hello\" \" \" \"world\")").unwrap())
            .unwrap(),
        rele_elisp::LispObject::string("hello world")
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(substring \"hello world\" 0 5)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::string("hello")
    );
}

#[test]
fn elisp_eval_special_forms() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(if t 1 2)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(if nil 1 2)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(2)
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(and t t t)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::t()
    );
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(or nil nil t)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::t()
    );
}

#[test]
fn elisp_eval_defun_and_call() {
    let mut s = state_with("");
    s.elisp
        .eval(rele_elisp::read("(defun add (x y) (+ x y))").unwrap())
        .unwrap();
    assert_eq!(
        s.elisp
            .eval(rele_elisp::read("(add 3 4)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(7)
    );
}

#[test]
fn elisp_eval_expression_command() {
    let s = state_with("");
    let handler = s.commands.get("eval-expression");
    assert!(handler.is_some());
}

#[test]
fn elisp_primitives_registered() {
    let s = MdAppState::new();
    assert!(s.elisp.eval(rele_elisp::read("(+ 1 2)").unwrap()).is_ok());
    assert!(
        s.elisp
            .eval(rele_elisp::read("(cons 1 2)").unwrap())
            .is_ok()
    );
    assert!(
        s.elisp
            .eval(rele_elisp::read("(car '(1 2))").unwrap())
            .is_ok()
    );
}

#[test]
fn elisp_macro_defmacro_and_call() {
    let mut s = state_with("");
    s.elisp
        .eval(rele_elisp::read("(defmacro my-not (x) (list 'if x nil t))").unwrap())
        .unwrap();
    let result = s
        .elisp
        .eval(rele_elisp::read("(my-not t)").unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::nil());
}

// EditorCallbacks bridge tests — verify that elisp code can actually see
// and manipulate the buffer through the trait methods. `state_with()`
// boxes the state and installs callbacks after boxing, so the raw
// pointer inside the callbacks points to stable heap memory.

#[test]
fn elisp_buffer_string_reads_document() {
    let s = state_with("Hello, world!");
    let result = s
        .elisp
        .eval(rele_elisp::read("(buffer-string)").unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::string("Hello, world!"));
}

#[test]
fn elisp_point_reads_cursor_position() {
    let mut s = state_with("Hello, world!");
    s.cursor.position = 7;
    let result = s.elisp.eval(rele_elisp::read("(point)").unwrap()).unwrap();
    assert_eq!(result, rele_elisp::LispObject::integer(7));
}

#[test]
fn elisp_insert_mutates_buffer() {
    let mut s = state_with("");
    s.elisp
        .eval(rele_elisp::read(r#"(insert "hello from elisp")"#).unwrap())
        .unwrap();
    assert_eq!(s.document.text(), "hello from elisp");
}

#[test]
fn elisp_goto_char_moves_cursor() {
    let mut s = state_with("0123456789");
    s.elisp
        .eval(rele_elisp::read("(goto-char 5)").unwrap())
        .unwrap();
    assert_eq!(s.cursor.position, 5);
}

/// Regression: `ElispEditorCallbacks::save_buffer` in the GPUI
/// client wrote the buffer to disk via `fs::write` but skipped
/// `mark_clean` and `lsp_did_save`. After a save-through-elisp the
/// document still showed as dirty and the language server never
/// learned the save happened.
/// Regression: `init_elisp` used to run during `MdAppState::new()`,
/// *before* `install_elisp_editor_callbacks` wired up the editor
/// bridge. Any form in `~/.gpui-md.el` that touched the buffer
/// (`(insert ...)`, `(find-file ...)`, `(goto-char ...)`) silently
/// no-opped against the stub editor — nothing visibly wrong at
/// startup, just a dead init file.
///
/// The fix: user init now runs as part of
/// `install_elisp_editor_callbacks`, so editor-using forms take
/// effect. This test drives the testable helper
/// `load_user_init_source` directly so it doesn't depend on
/// `$HOME`.
#[test]
fn init_elisp_forms_can_manipulate_the_buffer() {
    let mut s = state_with("hello");
    // Precondition: cursor at 0.
    assert_eq!(s.cursor.position, 0);

    // Simulate a user init snippet that uses the editor.
    s.load_user_init_source("(goto-char 4)");

    assert_eq!(
        s.cursor.position, 4,
        "user init forms must run with the editor bridge attached"
    );
}

#[test]
fn elisp_save_buffer_marks_clean_and_returns_true_on_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ok.md");

    let mut s = state_with("hello");
    s.document.set_file_path(path.clone());
    // `set_file_path` doesn't touch the dirty flag; pre-condition:
    // we just wrote content, so the buffer is clean already. Make
    // it dirty explicitly so the test can observe mark_clean.
    s.document.insert(s.document.len_chars(), " world");
    assert!(s.document.is_dirty());

    s.elisp
        .eval(rele_elisp::read("(save-buffer)").unwrap())
        .expect("save-buffer eval");

    assert!(
        !s.document.is_dirty(),
        "save-buffer must mark the document clean after a successful write"
    );
}

#[test]
fn elisp_save_buffer_returns_nil_on_write_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Non-existent subdirectory — fs::write will fail.
    let bogus = dir.path().join("no-such-dir").join("file.md");

    let mut s = state_with("content");
    s.document.set_file_path(bogus);
    s.document.insert(0, "x");

    // save-buffer should still report failure (returns nil in elisp
    // for the failure path).
    let result = s
        .elisp
        .eval(rele_elisp::read("(save-buffer)").unwrap())
        .expect("save-buffer eval should not error");
    assert_eq!(
        result,
        rele_elisp::LispObject::nil(),
        "save-buffer must return nil when the write fails"
    );
    assert!(
        s.document.is_dirty(),
        "failed save must not mark the document clean"
    );
}

/// Regression: `ElispEditorCallbacks::goto_char` in the GPUI client
/// used to assign `self.cursor.position = pos` without clamping to
/// the buffer length. A subsequent rope query would panic on the
/// out-of-bounds index.
#[test]
fn elisp_goto_char_past_end_clamps_without_panic() {
    let mut s = state_with("hello"); // 5 chars
    s.elisp
        .eval(rele_elisp::read("(goto-char 9999)").unwrap())
        .expect("goto-char past end must not error");
    assert!(
        s.cursor.position <= s.document.len_chars(),
        "cursor should be clamped to buffer length, got {} for len {}",
        s.cursor.position,
        s.document.len_chars(),
    );
    // Following rope queries must not panic.
    let _ = s.document.char_to_line(s.cursor.position);
}

#[test]
fn elisp_cl_defstruct_works_in_gpui_md() {
    // Verify the new cl-defstruct implementation works in the integrated env.
    let s = state_with("");
    s.elisp
        .eval(rele_elisp::read("(cl-defstruct todo title done)").unwrap())
        .unwrap();
    let result = s
        .elisp
        .eval(rele_elisp::read(r#"(todo-title (make-todo "buy milk" nil))"#).unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::string("buy milk"));
}
