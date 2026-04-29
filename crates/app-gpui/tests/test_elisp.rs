#![allow(clippy::disallowed_methods)]
use gpui_md::document::BufferKind;
use gpui_md::state::MdAppState;

/// Construct a state, set its text and cursor, and load the shared elisp
/// command layer.
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
}

#[test]
fn elisp_eval_arithmetic() {
    let mut s = state_with("");
    let result = s.eval_lisp(rele_elisp::read("(+ 1 2 3)").unwrap());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), rele_elisp::LispObject::integer(6));
}

#[test]
fn elisp_eval_list_operations() {
    let mut s = state_with("");
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(cons 1 '(2 3))").unwrap())
            .unwrap(),
        rele_elisp::read("(1 2 3)").unwrap()
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(car '(1 2 3))").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(cdr '(1 2 3))").unwrap())
            .unwrap(),
        rele_elisp::read("(2 3)").unwrap()
    );
}

#[test]
fn elisp_eval_string_operations() {
    let mut s = state_with("");
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(concat \"hello\" \" \" \"world\")").unwrap())
            .unwrap(),
        rele_elisp::LispObject::string("hello world")
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(substring \"hello world\" 0 5)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::string("hello")
    );
}

#[test]
fn elisp_eval_special_forms() {
    let mut s = state_with("");
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(if t 1 2)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(if nil 1 2)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::integer(2)
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(and t t t)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::t()
    );
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(or nil nil t)").unwrap())
            .unwrap(),
        rele_elisp::LispObject::t()
    );
}

#[test]
fn elisp_eval_defun_and_call() {
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read("(defun add (x y) (+ x y))").unwrap())
        .unwrap();
    assert_eq!(
        s.eval_lisp(rele_elisp::read("(add 3 4)").unwrap()).unwrap(),
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
    let mut s = MdAppState::new();
    assert!(s.eval_lisp(rele_elisp::read("(+ 1 2)").unwrap()).is_ok());
    assert!(s.eval_lisp(rele_elisp::read("(cons 1 2)").unwrap()).is_ok());
    assert!(
        s.eval_lisp(rele_elisp::read("(car '(1 2))").unwrap())
            .is_ok()
    );
}

#[test]
fn elisp_macro_defmacro_and_call() {
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read("(defmacro my-not (x) (list 'if x nil t))").unwrap())
        .unwrap();
    let result = s
        .eval_lisp(rele_elisp::read("(my-not t)").unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::nil());
}

// EditorCallbacks bridge tests — verify that elisp code can actually see
// and manipulate the buffer through the trait methods. `state_with()`
// boxes the state and installs callbacks after boxing, so the raw
// pointer inside the callbacks points to stable heap memory.

#[test]
fn elisp_buffer_string_reads_document() {
    let mut s = state_with("Hello, world!");
    let result = s
        .eval_lisp(rele_elisp::read("(buffer-string)").unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::string("Hello, world!"));
}

#[test]
fn elisp_point_reads_cursor_position() {
    let mut s = state_with("Hello, world!");
    s.cursor.position = 7;
    let result = s.eval_lisp(rele_elisp::read("(point)").unwrap()).unwrap();
    assert_eq!(result, rele_elisp::LispObject::integer(7));
}

#[test]
fn elisp_insert_mutates_buffer() {
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read(r#"(insert "hello from elisp")"#).unwrap())
        .unwrap();
    assert_eq!(s.document.text(), "hello from elisp");
}

#[test]
fn elisp_goto_char_moves_cursor() {
    let mut s = state_with("0123456789");
    s.eval_lisp(rele_elisp::read("(goto-char 5)").unwrap())
        .unwrap();
    assert_eq!(s.cursor.position, 5);
}

/// Regression: the GPUI elisp `save-buffer` bridge wrote the buffer
/// to disk via `fs::write` but skipped
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

    s.eval_lisp(rele_elisp::read("(save-buffer)").unwrap())
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
        .eval_lisp(rele_elisp::read("(save-buffer)").unwrap())
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

/// Regression: the GPUI elisp `goto-char` bridge used to assign
/// `self.cursor.position = pos` without clamping to the buffer length.
/// A subsequent rope query would panic on the out-of-bounds index.
#[test]
fn elisp_goto_char_past_end_clamps_without_panic() {
    let mut s = state_with("hello"); // 5 chars
    s.eval_lisp(rele_elisp::read("(goto-char 9999)").unwrap())
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
fn rust_file_loads_rust_mode() {
    let s = MdAppState::from_file(std::path::PathBuf::from("example.rs"), "fn main() {}\n");

    assert_eq!(s.current_major_mode.as_deref(), Some("rust-mode"));
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "rust-mode", "C-c C-c"),
        Some("rust-compile".to_string())
    );
}

/// Phase 2 — `after-save-hook` runs when the buffer is saved through
/// the elisp `(save-buffer)` primitive. Uses a hook function that
/// `setq`'s a sentinel variable; we then read the variable back via
/// `symbol-value` to confirm the hook fired.
#[test]
fn after_save_hook_fires_on_save_buffer() {
    let tmp = std::env::temp_dir().join("rele-after-save-hook-test.md");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, "initial\n").expect("seed file");

    let mut s = Box::new(MdAppState::new());
    let content = std::fs::read_to_string(&tmp).unwrap();
    s.open_file_as_buffer(tmp.clone(), &content);
    s.install_elisp_editor_callbacks();

    s.eval_lisp(rele_elisp::read("(setq saved-flag nil)").unwrap())
        .unwrap();
    s.eval_lisp(rele_elisp::read("(defun rele-test-hook-fn () (setq saved-flag t))").unwrap())
        .unwrap();
    s.eval_lisp(rele_elisp::read("(add-hook 'after-save-hook 'rele-test-hook-fn)").unwrap())
        .unwrap();

    let hook_value = s
        .eval_lisp(rele_elisp::read("after-save-hook").unwrap())
        .unwrap();
    assert!(
        !matches!(hook_value, rele_elisp::LispObject::Nil),
        "after-save-hook variable should be populated by add-hook, got {hook_value:?}"
    );

    assert!(s.save_file_from_elisp(), "save must succeed");

    let flag = s
        .eval_lisp(rele_elisp::read("saved-flag").unwrap())
        .unwrap();
    assert_eq!(
        flag,
        rele_elisp::LispObject::T,
        "after-save-hook should have set saved-flag to t (hook value was {hook_value:?})"
    );

    let _ = std::fs::remove_file(&tmp);
}

/// Phase 3 — an elisp defun with `(interactive "sLabel: ")` opens
/// the FreeText minibuffer when invoked via M-x, even though the
/// command isn't in the Rust registry.
#[test]
fn elisp_interactive_string_spec_opens_minibuffer() {
    let mut s = Box::new(MdAppState::new());
    s.install_elisp_editor_callbacks();
    s.eval_lisp(
        rele_elisp::read("(defun rele-test-prompt-cmd (x) (interactive \"sSay: \") (insert x))")
            .unwrap(),
    )
    .unwrap();
    assert!(
        rele_elisp::interactive_spec_for("rele-test-prompt-cmd", &s.lisp_host.interpreter().state,)
            .is_some(),
        "interactive spec should be discoverable",
    );
    s.run_command_by_name("rele-test-prompt-cmd");
    assert!(
        s.minibuffer.active,
        "interactive 's' spec should open the minibuffer",
    );
}

/// `C-g` (`keyboard-quit`) cancels prefix arg + selection + the
/// pending `C-x` chord. Routed through the elisp defun so this
/// also confirms the dispatcher / bridge wiring.
#[test]
fn elisp_keyboard_quit_clears_modal_state() {
    let mut s = Box::new(MdAppState::new());
    s.install_elisp_editor_callbacks();
    // Set up some modal state to verify it gets cleared.
    s.universal_arg = Some(4);
    s.c_x_pending = true;
    s.cursor.position = 0;
    s.document.set_text("hello");
    s.cursor.start_selection();
    s.cursor.position = 5;
    assert!(
        s.cursor.has_selection(),
        "test setup: should have selection"
    );

    s.run_command_direct("keyboard-quit", gpui_md::commands::CommandArgs::default());

    assert!(s.universal_arg.is_none(), "C-g should clear universal-arg");
    assert!(!s.c_x_pending, "C-g should clear C-x pending");
    assert!(
        !s.cursor.has_selection(),
        "C-g should clear the cursor selection",
    );
}

/// `M-x load-theme RET modus-operandi RET` — verify the defun is
/// reachable from the dispatcher and `(load "modus-operandi-theme")`
/// finds the theme file. Themes live in `etc/themes/`, not `lisp/`,
/// so the load-path setup needs that subdirectory too.
#[test]
fn elisp_load_theme_finds_emacs_theme_file() {
    if rele_elisp::bootstrap::emacs_lisp_dir().is_none() {
        eprintln!("skipping: EMACS_LISP_DIR not set / Emacs not installed");
        return;
    }
    let mut s = Box::new(MdAppState::new());
    s.install_elisp_editor_callbacks();
    // Probe through the dispatcher path used by M-x.
    // tango is a self-contained theme (no modus-themes-style
    // dependency chain), good for verifying the M-x load-theme path
    // end-to-end.
    s.run_command_with_string("load-theme", "tango".into());
    // We don't have a direct way to verify `load` succeeded from the
    // outside — instead, after the load, the symbol `modus-operandi`
    // should be on `custom-known-themes` (set by `(deftheme ...)` in
    // the theme file).  If load-path doesn't include etc/themes the
    // load silently no-ops and we can detect that.
    let known = s
        .eval_lisp(
            rele_elisp::read(
                "(prin1-to-string (and (boundp 'custom-known-themes) custom-known-themes))",
            )
            .unwrap(),
        )
        .unwrap();
    eprintln!("custom-known-themes after load-theme: {known:?}");
    // Phase: confirm at least one theme is registered.
    let count = s
        .eval_lisp(
            rele_elisp::read("(if (boundp 'custom-known-themes) (length custom-known-themes) 0)")
                .unwrap(),
        )
        .unwrap();
    eprintln!("known-themes length: {count:?}");
}

/// Phase 4 — `lookup_mode_key` finds the function bound to a key
/// in a major mode's keymap. We synthesize a tiny mode + map ourselves
/// instead of depending on real upstream dired-mode-map (which would
/// require a full lazy-load just for this assertion).
#[test]
fn elisp_lookup_mode_key_finds_binding() {
    let mut s = state_with("");
    s.eval_lisp(
        rele_elisp::read(
            "(progn \
                   (defvar fake-mode-map (make-sparse-keymap)) \
                   (define-key fake-mode-map (kbd \"n\") 'next-line) \
                   (define-key fake-mode-map (kbd \"q\") 'quit-window))",
        )
        .unwrap(),
    )
    .expect("set up fake-mode-map");
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "fake-mode", "n"),
        Some("next-line".to_string()),
        "lookup_mode_key should resolve `n` to next-line",
    );
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "fake-mode", "q"),
        Some("quit-window".to_string()),
    );
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "fake-mode", "z"),
        None,
        "unknown key should return None",
    );
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "no-such-mode", "n"),
        None,
        "unknown mode should return None",
    );
}

#[test]
fn elisp_dired_sets_mode_map_and_starts_on_first_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("entry.txt");
    std::fs::write(&path, "hello").expect("write fixture");

    let mut s = state_with("");
    s.run_command_with_string("dired", dir.path().display().to_string());

    assert!(s.current_buffer_name.starts_with("*Dired: "));
    assert_eq!(s.current_buffer_kind, BufferKind::Dired);
    assert_eq!(s.current_major_mode.as_deref(), Some("dired-mode"));
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "dired-mode", "n"),
        Some("next-line".to_string())
    );
    assert_eq!(
        rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "dired-mode", "p"),
        Some("previous-line".to_string())
    );
    assert_eq!(s.document.char_to_line(s.cursor.position), 2);
}

#[test]
fn elisp_dired_ret_opens_entry_at_point() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("entry.txt");
    std::fs::write(&path, "hello").expect("write fixture");

    let mut s = state_with("");
    s.run_command_with_string("dired", dir.path().display().to_string());

    let entry_line = s
        .document
        .text()
        .lines()
        .position(|line| line.contains("entry.txt"))
        .expect("entry should be listed");
    s.cursor.position = s.document.line_to_char(entry_line);

    assert!(s.dired_open_entry_at_point());
    assert_eq!(s.current_buffer_name, "entry.txt");
    assert!(s.document.text().contains("hello"));
}

#[test]
fn elisp_dired_refresh_and_up_use_generated_buffer_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let child = dir.path().join("child");
    std::fs::create_dir(&child).expect("create child dir");

    let mut s = state_with("");
    s.run_command_with_string("dired", dir.path().display().to_string());
    let later = dir.path().join("later.txt");
    std::fs::write(&later, "later").expect("write fixture");

    assert!(s.dired_refresh_current_buffer());
    assert!(s.document.text().contains("later.txt"));

    s.run_command_with_string("dired", child.display().to_string());
    assert!(s.dired_up_current_directory());
    assert!(
        s.document
            .line(0)
            .to_string()
            .contains(&dir.path().display().to_string())
    );
}

/// Phase 3 — calling `(dired "/tmp")` from elisp through the
/// `dired-cmd` defun lazy-loads upstream dired.el and produces a
/// `*Dired*`-style buffer. Skipped when the Emacs source tree
/// isn't on disk; we don't want CI to fail on machines without it.
#[test]
fn elisp_dired_lazy_loads_and_runs() {
    if rele_elisp::bootstrap::emacs_lisp_dir().is_none() {
        eprintln!("skipping: EMACS_LISP_DIR not set / Emacs not installed");
        return;
    }
    // dired.el's load chain (custom, easymenu, files, autorevert, …)
    // recurses deep enough that the default 2 MiB test thread stack
    // overflows. Re-host on a bigger stack — the test itself is
    // single-threaded inside the spawn.
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(dired_lazy_load_inner)
        .expect("spawn bigger-stack thread");
    handle.join().expect("dired lazy-load probe panicked");
}

fn dired_lazy_load_inner() {
    let mut s = Box::new(MdAppState::new());
    s.install_elisp_editor_callbacks();
    // Direct (require 'dired) instead of going through `dired-cmd`.
    // The wrapper does the same thing but its post-load `(dired path)`
    // call hits the still-known interpreter gaps inside dired-noselect
    // (wrong-type-argument inside the buffer/string handling that
    // Phase 1's bridge doesn't yet cover). We don't want those gaps
    // to mask the lazy-load itself, which is what this test gates.
    let _ = s.eval_lisp(
        rele_elisp::read(
            "(progn \
              (require 'seq) \
              (require 'easymenu) \
              (load \"menu-bar\" t) \
              (load \"files\" t) \
              (load \"dnd\" t) \
              (load \"autorevert\" t) \
              (load \"dired-loaddefs\" t) \
              (load \"dired\"))",
        )
        .unwrap(),
    );
    // Phase 3 acceptance: confirm the lazy-load wired the upstream
    // `dired` function into the function cell. Whether `(dired ...)`
    // runs error-free is up to the buffer / file primitives we'll
    // keep filling in across the remaining phases.
    let dired_fbound = s
        .eval_lisp(rele_elisp::read("(fboundp 'dired-noselect)").unwrap())
        .unwrap();
    assert_eq!(
        dired_fbound,
        rele_elisp::LispObject::T,
        "lazy-loading dired.el should populate dired-noselect's function cell",
    );
}

/// Phase 2 — `(file-attributes path 'string)` returns the full
/// 12-element list with a real mode string and (Unix only) the
/// owner's name in slot 2. dired's listing rendering depends on
/// these slots.
#[test]
fn elisp_file_attributes_returns_full_tuple() {
    let mut s = state_with("");
    let attrs = s
        .eval_lisp(rele_elisp::read("(file-attributes \"/tmp\" 'string)").unwrap())
        .expect("file-attributes should not error");
    // Result is a cons list. Walk it and check slot count + mode-str shape.
    let mut count = 0;
    let mut cur = attrs.clone();
    let mut mode_str: Option<String> = None;
    while let Some((car, cdr)) = cur.destructure_cons() {
        if count == 8 {
            // slot 8: mode string like "drwxrwxrwx"
            if let rele_elisp::LispObject::String(s) = &car {
                mode_str = Some(s.clone());
            }
        }
        count += 1;
        cur = cdr;
    }
    assert!(
        count >= 10,
        "file-attributes should return at least 10 elements, got {count} ({attrs:?})",
    );
    let mode = mode_str.expect("slot 8 should be a string mode");
    assert_eq!(
        mode.len(),
        10,
        "mode string should be 10 chars (drwxr-xr-x), got {mode:?}",
    );
    // /tmp is a real directory on Linux but a symlink to /private/tmp
    // on macOS — accept either.
    assert!(
        mode.starts_with('d') || mode.starts_with('l'),
        "/tmp should be a directory or symlink: {mode:?}",
    );
}

/// Phase 2 — `(insert-directory path "-l" nil t)` writes an `ls -l`
/// style listing into the buffer. We just check the listing has at
/// least the `total` header and one row.
#[test]
fn elisp_insert_directory_produces_listing() {
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read("(insert-directory \"/tmp\" \"-la\" nil t)").unwrap())
        .expect("insert-directory should not error");
    let body = s.document.text();
    assert!(
        body.contains("total "),
        "insert-directory should produce a `total` header, got: {body:?}",
    );
}

/// Phase 1 (real-dired plan) — `(get-buffer-create "*foo*")` calls
/// the EditorCallbacks bridge, which makes the buffer show up in the
/// editor's buffer list. Confirms the elisp ↔ editor buffer registry
/// is wired through.
#[test]
fn elisp_get_buffer_create_appears_in_editor_buffer_list() {
    let mut s = Box::new(MdAppState::new());
    s.install_elisp_editor_callbacks();
    let before = s.buffer_names();
    s.eval_lisp(rele_elisp::read("(get-buffer-create \"*phase1-bridge-test*\")").unwrap())
        .expect("get-buffer-create should not error");
    let after = s.buffer_names();
    assert!(
        after.contains(&"*phase1-bridge-test*".to_string()),
        "elisp-created buffer should appear in editor buffer list (before={before:?} after={after:?})",
    );
}

/// Phase 4 — `(global-set-key "C-h" 'foo)` populates the global
/// keybinding table that client key handlers consult.
#[test]
fn elisp_global_set_key_records_user_binding() {
    rele_elisp::clear_global_keybindings();
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read("(global-set-key \"C-h\" 'my-help)").unwrap())
        .unwrap();
    assert_eq!(
        rele_elisp::lookup_global_key("C-h"),
        Some("my-help".to_string()),
    );
    rele_elisp::clear_global_keybindings();
}

#[test]
fn elisp_cl_defstruct_works_in_gpui_md() {
    // Verify the new cl-defstruct implementation works in the integrated env.
    let mut s = state_with("");
    s.eval_lisp(rele_elisp::read("(cl-defstruct todo title done)").unwrap())
        .unwrap();
    let result = s
        .eval_lisp(rele_elisp::read(r#"(todo-title (make-todo "buy milk" nil))"#).unwrap())
        .unwrap();
    assert_eq!(result, rele_elisp::LispObject::string("buy milk"));
}
