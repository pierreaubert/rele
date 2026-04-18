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
