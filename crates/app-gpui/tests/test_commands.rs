#![allow(clippy::disallowed_methods)]
//! Integration tests for the command registry.

use std::rc::Rc;

use gpui_md::commands::{Command, CommandArgs, CommandCategory, InteractiveSpec};
use gpui_md::minibuffer::MiniBufferPrompt;
use gpui_md::state::MdAppState;
use gpui_md::state::PendingMinibufferAction;

#[test]
fn builtin_commands_are_registered() {
    let state = MdAppState::new();
    assert!(state.commands.len() > 20);
    assert!(state.commands.get("forward-char").is_some());
    assert!(state.commands.get("backward-char").is_some());
    assert!(state.commands.get("find-file").is_some());
    assert!(state.commands.get("switch-to-buffer").is_some());
}

#[test]
fn run_forward_char_moves_cursor() {
    let mut state = MdAppState::new();
    state.document.set_text("hello world");
    state.cursor.position = 0;
    state.run_command_by_name("forward-char");
    assert_eq!(state.cursor.position, 1);
}

#[test]
fn run_forward_char_with_prefix_arg() {
    let mut state = MdAppState::new();
    state.document.set_text("hello world");
    state.cursor.position = 0;
    state.run_command_direct("forward-char", CommandArgs::default().with_prefix(5));
    assert_eq!(state.cursor.position, 5);
}

#[test]
fn register_custom_command_at_runtime() {
    let mut state = MdAppState::new();
    state.commands.register(Command {
        name: "insert-banner".to_string(),
        description: "Insert a banner".to_string(),
        category: CommandCategory::Custom,
        handler: Rc::new(|s, _| {
            s.insert_text("*** BANNER ***");
        }),
        interactive: InteractiveSpec::None,
    });
    state.document.set_text("");
    state.cursor.position = 0;
    state.run_command_by_name("insert-banner");
    assert_eq!(state.document.text(), "*** BANNER ***");
}

#[test]
fn run_unknown_command_is_noop() {
    let mut state = MdAppState::new();
    state.document.set_text("hello");
    state.cursor.position = 3;
    state.run_command_by_name("nonexistent-command");
    // Nothing changes
    assert_eq!(state.document.text(), "hello");
    assert_eq!(state.cursor.position, 3);
}

#[test]
fn switch_to_buffer_command_opens_minibuffer() {
    let mut state = MdAppState::new();
    assert!(!state.minibuffer.active);
    state.run_command_by_name("switch-to-buffer");
    assert!(state.minibuffer.active);
}

#[test]
fn registry_filter_returns_matches() {
    let state = MdAppState::new();
    let filtered = state.commands.filter("forward");
    assert!(filtered.contains(&"forward-char".to_string()));
    assert!(filtered.contains(&"forward-word".to_string()));
}

#[test]
fn toggle_preview_command_flips_flag() {
    // `toggle-preview` is an elisp defun in commands.el that calls
    // `editor--toggle-preview`. The defun + bridge are wired up in
    // `install_elisp_editor_callbacks`, so pin the state and install
    // before dispatching (same pattern as `undo_command_reverts_edit`).
    let mut state = Box::new(MdAppState::new());
    state.install_elisp_editor_callbacks();
    let before = state.show_preview;
    state.run_command_by_name("toggle-preview");
    assert_eq!(state.show_preview, !before);
}

#[test]
fn undo_command_reverts_edit() {
    // `undo` is a defun in commands.el that delegates to
    // `primitive-undo`. Another test in this binary may have called
    // `install_elisp_editor_callbacks` which globally populated the
    // obarray with the defun; without the bridge installed here the
    // defun's `primitive-undo` call would hit the stub editor. Pin
    // the state and install callbacks so dispatch works regardless
    // of test ordering.
    let mut state = Box::new(MdAppState::new());
    state.install_elisp_editor_callbacks();
    state.document.set_text("initial");
    state.cursor.position = 7;
    state.insert_text(" more");
    assert_eq!(state.document.text(), "initial more");
    state.run_command_by_name("undo");
    assert_eq!(state.document.text(), "initial");
}

/// Regression: `run_command_by_name` used to early-return when the
/// name wasn't in the Rust registry, even if the user had defined a
/// matching elisp defun. That made elisp-only commands unreachable
/// via M-x. The dispatch path now also consults the obarray.
#[test]
fn run_command_by_name_dispatches_elisp_only_defun() {
    let mut state = MdAppState::new();
    state.install_elisp_editor_callbacks();
    state.document.set_text("abc");
    state.cursor.position = 0;

    state
        .eval_lisp_source("(defun gpui-elisp-only-jump (&optional _n) (goto-char 2))")
        .expect("defun eval");

    // Name is NOT in the Rust registry.
    assert!(state.commands.get("gpui-elisp-only-jump").is_none());

    state.run_command_by_name("gpui-elisp-only-jump");
    assert_eq!(state.cursor.position, 2);
}

#[test]
fn run_command_by_name_dispatches_elisp_kill_region() {
    let mut state = MdAppState::new();
    state.install_elisp_editor_callbacks();
    state.document.set_text("abcdef");
    state.cursor.anchor = Some(1);
    state.cursor.position = 4;

    state.run_command_by_name("kill-region");
    assert_eq!(state.document.text(), "aef");
    assert_eq!(state.cursor.position, 1);

    state.run_command_by_name("yank");
    assert_eq!(state.document.text(), "abcdef");
}

#[test]
fn run_command_by_name_dispatches_elisp_kill_rectangle() {
    let mut state = MdAppState::new();
    state.install_elisp_editor_callbacks();
    state.document.set_text("abcd\nefgh\nijkl");
    state.cursor.anchor = Some(1);
    state.cursor.position = 13;

    state.run_command_by_name("kill-rectangle");
    assert_eq!(state.document.text(), "ad\neh\nil");

    state.cursor.position = 1;
    state.run_command_by_name("yank-rectangle");
    assert_eq!(state.document.text(), "abcd\nefgh\nijkl");
}

#[test]
fn command_completion_candidates_include_elisp_defuns() {
    let mut state = MdAppState::new();
    state.install_elisp_editor_callbacks();
    state
        .eval_lisp_source("(defun gpui-elisp-only-complete (&optional _n) t)")
        .expect("defun eval");

    let candidates = state.command_completion_candidates();

    assert!(candidates.iter().any(|name| name == "forward-char"));
    assert!(candidates.iter().any(|name| name == "rust-mode"));
    assert!(
        candidates
            .iter()
            .any(|name| name == "gpui-elisp-only-complete")
    );
}

#[test]
fn run_command_by_name_runs_autoloaded_rust_mode() {
    let mut state = MdAppState::new();

    state.run_command_by_name("rust-mode");

    assert_eq!(state.current_major_mode.as_deref(), Some("rust-mode"));
}

#[test]
fn command_minibuffer_completes_elisp_only_defun() {
    let mut state = MdAppState::new();
    state.install_elisp_editor_callbacks();
    state
        .eval_lisp_source("(defun gpui-elisp-only-complete (&optional _n) t)")
        .expect("defun eval");

    state.minibuffer_start_command();
    for ch in "gpui-elisp-only-c".chars() {
        state.minibuffer.add_char(ch);
        state.minibuffer_refresh_completions();
    }
    state.minibuffer_complete();

    assert_eq!(state.minibuffer.input, "gpui-elisp-only-complete");
}

#[test]
fn find_file_minibuffer_completes_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("notes.md"), "hello").expect("write file");

    let mut state = MdAppState::new();
    state.minibuffer_open(
        MiniBufferPrompt::FindFile {
            base_dir: dir.path().to_path_buf(),
        },
        Vec::new(),
        PendingMinibufferAction::OpenFile,
    );
    state.minibuffer.add_char('n');
    state.minibuffer_refresh_completions();
    state.minibuffer_complete();

    assert_eq!(state.minibuffer.input, "notes.md");
}
