use rele_elisp::{LispObject, bootstrap::make_stdlib_interp, read};

fn eval_string(source: &str) -> String {
    let interp = make_stdlib_interp();
    interp
        .eval(read(source).expect(source))
        .expect(source)
        .princ_to_string()
}

#[test]
fn coding_system_eol_type_reports_known_variants() {
    let result = eval_string(
        "(let ((base (coding-system-eol-type 'utf-8)))
           (list (coding-system-eol-type nil)
                 (coding-system-eol-type 'utf-8-unix)
                 (coding-system-eol-type 'utf-8-dos)
                 (coding-system-eol-type 'utf-8-mac)
                 (coding-system-eol-type 'no-conversion)
                 (vectorp base)
                 (aref base 1)
                 (coding-system-eol-type 'rele-no-such-coding-system)))",
    );
    assert_eq!(result, "(0 0 1 2 0 t utf-8-dos nil)");
}

#[test]
fn find_file_name_handler_matches_dynamic_alist() {
    let result = eval_string(
        "(let ((file-name-handler-alist '((\"^/tmp\" . rele-handler))))
           (list (find-file-name-handler \"/tmp/example\" 'insert-file-contents)
                 (find-file-name-handler \"/var/example\" 'insert-file-contents)
                 (let ((inhibit-file-name-handlers '(rele-handler))
                       (inhibit-file-name-operation 'insert-file-contents))
                   (find-file-name-handler \"/tmp/example\" 'insert-file-contents))))",
    );
    assert_eq!(result, "(rele-handler nil nil)");
}

#[test]
fn delete_directory_internal_removes_empty_directory() {
    let dir = format!("tmp/rele-delete-directory-internal-{}", std::process::id());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");

    let interp = make_stdlib_interp();
    let source = format!("(delete-directory-internal {dir:?})");
    let result = interp.eval(read(&source).expect("delete-directory-internal form"));
    assert_eq!(
        result.expect("delete-directory-internal"),
        LispObject::nil()
    );
    assert!(!std::path::Path::new(&dir).exists());
}

#[test]
fn delete_file_internal_removes_regular_file() {
    let file = format!("tmp/rele-delete-file-internal-{}", std::process::id());
    let _ = std::fs::remove_file(&file);
    std::fs::write(&file, "data").expect("create test file");

    let interp = make_stdlib_interp();
    let source = format!("(delete-file-internal {file:?})");
    let result = interp.eval(read(&source).expect("delete-file-internal form"));
    assert_eq!(result.expect("delete-file-internal"), LispObject::nil());
    assert!(!std::path::Path::new(&file).exists());
}

#[test]
fn coding_and_unibyte_primitives_are_not_stubbed_defaults() {
    assert_eq!(
        eval_string("(detect-coding-string (string ?a ?\\0 ?b))"),
        "(no-conversion undecided)",
    );
    assert_eq!(
        eval_string("(multibyte-string-p (string-as-unibyte \"é\"))"),
        "nil",
    );
    assert_eq!(eval_string("(unibyte-char-to-multibyte 65)"), "65");
}

#[test]
fn describe_buffer_bindings_writes_header_and_visible_menu_items() {
    assert_eq!(
        eval_string(
            "(with-temp-buffer
               (define-key global-map (kbd \"C-c C-l r\")
                 `(menu-item \"2\" identity :filter ,(lambda (cmd) cmd)))
               (describe-buffer-bindings (current-buffer))
               (goto-char (point-min))
               (list (not (null (search-forward \"key             binding\" nil t)))
                     (not (null (search-forward \"C-c C-l r\" nil t)))))",
        ),
        "(t t)",
    );
}

#[test]
fn describe_buffer_bindings_handles_cyclic_prefix_maps() {
    assert_eq!(
        eval_string(
            "(let ((global-map (make-sparse-keymap)))
               (define-key global-map (kbd \"C-c\") (cons \"Prefix\" global-map))
               (with-temp-buffer
                 (describe-buffer-bindings (current-buffer))
                 (goto-char (point-min))
                 (not (null (search-forward \"key             binding\" nil t)))))",
        ),
        "t",
    );
}

#[test]
fn copy_keymap_and_equal_handle_cyclic_keymaps() {
    assert_eq!(
        eval_string(
            "(let ((map (make-sparse-keymap)))
               (setcdr map (cons map nil))
               (equal (copy-keymap map) map))",
        ),
        "t",
    );
}

#[test]
fn read_string_runs_minibuffer_setup_hook() {
    assert_eq!(
        eval_string(
            "(catch 'done
               (let ((hook (lambda () (throw 'done 'ran))))
                 (unwind-protect
                     (progn
                       (add-hook 'minibuffer-setup-hook hook)
                       (read-string \"prompt: \"))
                   (remove-hook 'minibuffer-setup-hook hook))))",
        ),
        "ran",
    );
}
