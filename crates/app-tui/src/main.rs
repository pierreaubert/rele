mod commands;
mod state;
mod ui;
mod windows;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use log::info;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rele_server::CommandArgs;
use rele_server::minibuffer::MiniBufferPrompt;

use state::{PendingMiniBufferAction, TuiAppState};

fn main() -> Result<()> {
    env_logger::init();
    info!("rele-tui starting");

    let state = match std::env::args().nth(1) {
        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            TuiAppState::from_file(path, &content)
        }
        None => TuiAppState::new(),
    };

    run_tui(state)?;
    Ok(())
}

fn run_tui(mut state: TuiAppState) -> Result<()> {
    // Set up terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Wire elisp ⇄ editor bridge. Must happen after `state` is pinned
    // in place (the local variable is stack-pinned for the rest of
    // `run_tui`); the bridge captures a raw pointer that's only
    // dereferenced from this thread.
    state.install_elisp_editor_callbacks();

    let result = event_loop(&mut terminal, &mut state);

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut TuiAppState,
) -> Result<()> {
    // Start LSP for the initial file if one was opened.
    state.lsp_did_open();

    loop {
        terminal.draw(|frame| ui::draw(frame, state))?;

        // Clear message after rendering it once
        state.message = None;

        if state.should_quit {
            // Shut down LSP servers gracefully.
            if let Some(ref mut registry) = state.lsp_registry {
                registry.shutdown_all();
            }
            return Ok(());
        }

        // Process pending LSP events (non-blocking).
        state.poll_lsp_events();

        // Poll for terminal events with a timeout (allows responsive quit)
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            handle_key(state, key);
        }
    }
}

fn handle_key(state: &mut TuiAppState, key: KeyEvent) {
    // Mini-buffer intercept (C-x C-f, C-x b, …). Top priority: when the
    // minibuffer is active, keystrokes go to it instead of the buffer.
    if state.minibuffer.active {
        handle_minibuffer_key(state, key);
        return;
    }

    if state.query_replace.is_some() {
        handle_query_replace_key(state, key);
        return;
    }

    // Handle C-x r prefix (rectangle commands).
    if state.c_x_r_pending {
        state.c_x_r_pending = false;
        handle_cx_r_key(state, key);
        return;
    }

    // Handle C-x prefix
    if state.c_x_pending {
        state.c_x_pending = false;
        handle_cx_key(state, key);
        return;
    }

    // Handle M-g prefix (goto chords: M-g n, M-g p, …).
    if state.m_g_pending {
        state.m_g_pending = false;
        handle_mg_key(state, key);
        return;
    }

    // Handle Meta (Escape) prefix
    if state.meta_pending {
        state.meta_pending = false;
        handle_meta_key(state, key);
        return;
    }

    // User keybindings via `(global-set-key "C-X" 'cmd)` win over the
    // hard-coded match below. Only single Ctrl-letter bindings today;
    // chord / Meta forms can be added when a user actually wants them.
    if let Some(cmd) = lookup_user_global_binding(key) {
        state.run_command(&cmd, CommandArgs::default());
        return;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        // ---- Ctrl bindings ----
        KeyCode::Char('f') if ctrl => state.run_command("forward-char", CommandArgs::default()),
        KeyCode::Char('b') if ctrl => state.run_command("backward-char", CommandArgs::default()),
        KeyCode::Char('n') if ctrl => state.run_command("next-line", CommandArgs::default()),
        KeyCode::Char('p') if ctrl => state.run_command("previous-line", CommandArgs::default()),
        KeyCode::Char('a') if ctrl => {
            state.run_command("move-beginning-of-line", CommandArgs::default());
        }
        KeyCode::Char('e') if ctrl => {
            state.run_command("move-end-of-line", CommandArgs::default());
        }
        KeyCode::Char('d') if ctrl => state.run_command("delete-char", CommandArgs::default()),
        KeyCode::Char('k') if ctrl => state.run_command("kill-line", CommandArgs::default()),
        KeyCode::Char('s') if ctrl => {
            state.minibuffer_open(
                MiniBufferPrompt::FreeText {
                    label: "Search".to_string(),
                },
                Vec::new(),
                PendingMiniBufferAction::RunCommandWithInput {
                    name: "search-forward".to_string(),
                },
            );
        }
        KeyCode::Char('r') if ctrl => {
            state.minibuffer_open(
                MiniBufferPrompt::FreeText {
                    label: "Search backward".to_string(),
                },
                Vec::new(),
                PendingMiniBufferAction::RunCommandWithInput {
                    name: "search-backward".to_string(),
                },
            );
        }
        KeyCode::Char('w') if ctrl => state.run_command("kill-region", CommandArgs::default()),
        KeyCode::Char('y') if ctrl => state.run_command("yank", CommandArgs::default()),
        KeyCode::Char(' ') if ctrl => state.run_command("set-mark", CommandArgs::default()),
        KeyCode::Null if ctrl => state.run_command("set-mark", CommandArgs::default()),
        KeyCode::Char('g') if ctrl => {
            // C-g — keyboard-quit. Routes through the elisp defun so
            // `~/.gpui-md.el` users can advise / hook the behaviour.
            // The defun bottoms out in `editor--keyboard-quit` which
            // calls `TuiAppState::keyboard_quit` (cancel-flag flip,
            // prefix clear, selection clear, minibuffer cancel,
            // "Quit" message).
            state.run_command("keyboard-quit", CommandArgs::default());
        }
        KeyCode::Char('x') if ctrl => {
            state.c_x_pending = true;
        }
        KeyCode::Char('/') if ctrl => state.run_command("undo", CommandArgs::default()),
        // C-v — forward one screenful (Emacs: `scroll-up-command`, the
        // display scrolls up so you see later content).
        KeyCode::Char('v') if ctrl => {
            state.run_command("scroll-up-command", CommandArgs::default());
        }

        // ---- Alt bindings ----
        // Terminals that send Alt+key as a single event land here. We
        // route to `handle_meta_key` so the same Alt table is shared
        // with the Esc-prefix path above.
        KeyCode::Char(_) if alt => {
            handle_meta_key(state, key);
        }

        // ---- Arrow keys ----
        KeyCode::Up => state.run_command("previous-line", CommandArgs::default()),
        KeyCode::Down => state.run_command("next-line", CommandArgs::default()),
        KeyCode::Left => state.run_command("backward-char", CommandArgs::default()),
        KeyCode::Right => state.run_command("forward-char", CommandArgs::default()),
        KeyCode::Home => state.run_command("move-beginning-of-line", CommandArgs::default()),
        KeyCode::End => state.run_command("move-end-of-line", CommandArgs::default()),
        // PageUp moves back a screenful (like M-v), PageDown forward
        // (like C-v). Bindings were swapped before — the Emacs command
        // names carry the direction (`scroll-up-command` = move
        // forward), so we pick the matching one for each key.
        KeyCode::PageUp => state.run_command("scroll-down-command", CommandArgs::default()),
        KeyCode::PageDown => state.run_command("scroll-up-command", CommandArgs::default()),

        // ---- Editing keys ----
        KeyCode::Backspace => {
            state.run_command("delete-backward-char", CommandArgs::default());
        }
        KeyCode::Delete => state.run_command("delete-char", CommandArgs::default()),
        KeyCode::Enter => state.run_command("newline", CommandArgs::default()),
        KeyCode::Tab => state.insert_text("\t"),

        // ---- Escape as Meta prefix ----
        KeyCode::Esc => {
            state.meta_pending = true;
        }

        // ---- Self-inserting characters ----
        KeyCode::Char(c) if !ctrl && !alt => {
            state.insert_text(&c.to_string());
        }

        _ => {}
    }
}

fn handle_query_replace_key(state: &mut TuiAppState, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('g') if ctrl => state.query_replace_cancel(),
        KeyCode::Esc | KeyCode::Char('q') => state.query_replace_cancel(),
        KeyCode::Char('y') | KeyCode::Char(' ') => state.query_replace_accept_current(false),
        KeyCode::Char('.') => state.query_replace_accept_current(true),
        KeyCode::Char('n') | KeyCode::Backspace | KeyCode::Delete => {
            state.query_replace_skip_current();
        }
        KeyCode::Char('!') => state.query_replace_replace_all_remaining(),
        _ => {
            state.message = Some("Query replace: y n ! . q".to_string());
        }
    }
}

/// Handle the second key after C-x prefix.
/// Translate a `KeyEvent` into the canonical `"C-x"` form Emacs
/// users pass to `(global-set-key)`, so we can look up a custom
/// binding. Returns `None` for any key shape we don't yet handle —
/// today only single Ctrl-letter keys (`Ctrl+letter`).
fn lookup_user_global_binding(key: KeyEvent) -> Option<String> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    if !c.is_ascii_alphabetic() {
        return None;
    }
    // Canonical lower-case form; mirrors Emacs `\C-x` notation.
    let key_str = format!("C-{}", c.to_ascii_lowercase());
    rele_elisp::lookup_global_key(&key_str)
}

fn handle_cx_key(state: &mut TuiAppState, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        // C-x C-s — save current buffer.
        KeyCode::Char('s') if ctrl => state.run_command("save-buffer", CommandArgs::default()),
        // C-x C-c — save file buffers and quit.
        KeyCode::Char('c') if ctrl => {
            state.run_command("save-buffers-kill-terminal", CommandArgs::default());
        }
        // C-x u — undo.
        KeyCode::Char('u') => state.run_command("undo", CommandArgs::default()),
        // C-x B — show the buffer list. Terminals differ here:
        // some report Shift-b as Char('B'), others as Char('b') +
        // SHIFT.
        KeyCode::Char('B') => {
            state.run_command("list-buffers", CommandArgs::default());
        }
        KeyCode::Char('b') if shift => {
            state.run_command("list-buffers", CommandArgs::default());
        }
        // C-x C-f — find file (open the minibuffer with a file-path prompt).
        KeyCode::Char('f') if ctrl => {
            #[allow(clippy::disallowed_methods)] // user action on launch
            let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            state.minibuffer_open(
                MiniBufferPrompt::FindFile { base_dir },
                Vec::new(),
                PendingMiniBufferAction::FindFile,
            );
        }
        // C-x b — switch to buffer (name completion from open buffers).
        KeyCode::Char('b') if !shift => {
            let candidates = state.buffer_names();
            state.minibuffer_open(
                MiniBufferPrompt::SwitchBuffer,
                candidates,
                PendingMiniBufferAction::SwitchBuffer,
            );
        }
        // C-x d — dired.
        KeyCode::Char('d') => {
            state.minibuffer_open(
                MiniBufferPrompt::FreeText {
                    label: "Dired directory".to_string(),
                },
                Vec::new(),
                PendingMiniBufferAction::Dired,
            );
        }
        // C-x k — kill buffer. Empty input kills the current one.
        KeyCode::Char('k') => {
            let candidates = state.buffer_names();
            state.minibuffer_open(
                MiniBufferPrompt::KillBuffer,
                candidates,
                PendingMiniBufferAction::KillBuffer,
            );
        }
        // C-x 2 / 3 / 0 / 1 / o — window management (Emacs convention).
        KeyCode::Char('2') => {
            state.run_command("split-window-below", CommandArgs::default());
        }
        KeyCode::Char('3') => {
            state.run_command("split-window-right", CommandArgs::default());
        }
        KeyCode::Char('0') => {
            state.run_command("delete-window", CommandArgs::default());
        }
        KeyCode::Char('1') => {
            state.run_command("delete-other-windows", CommandArgs::default());
        }
        KeyCode::Char('o') => {
            state.run_command("other-window", CommandArgs::default());
        }
        // C-x r — rectangle command prefix.
        KeyCode::Char('r') => {
            state.c_x_r_pending = true;
        }
        _ => {
            state.message = Some(format!("C-x {} is undefined", format_key(key)));
        }
    }
}

fn handle_cx_r_key(state: &mut TuiAppState, key: KeyEvent) {
    match key.code {
        // C-x r k/y/d/o/c/t — rectangle management.
        KeyCode::Char('k') => state.run_command("kill-rectangle", CommandArgs::default()),
        KeyCode::Char('y') => state.run_command("yank-rectangle", CommandArgs::default()),
        KeyCode::Char('d') => state.run_command("delete-rectangle", CommandArgs::default()),
        KeyCode::Char('o') => state.run_command("open-rectangle", CommandArgs::default()),
        KeyCode::Char('c') => state.run_command("clear-rectangle", CommandArgs::default()),
        KeyCode::Char('t') => {
            state.minibuffer_open(
                MiniBufferPrompt::FreeText {
                    label: "String rectangle".to_string(),
                },
                Vec::new(),
                PendingMiniBufferAction::RunCommandWithInput {
                    name: "string-rectangle".to_string(),
                },
            );
        }
        _ => {
            state.message = Some(format!("C-x r {} is undefined", format_key(key)));
        }
    }
}

/// Handle the second key after `M-g` prefix (Emacs "goto" chord).
/// Currently mapped: `n` → next LSP diagnostic, `p` → previous.
fn handle_mg_key(state: &mut TuiAppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('n') => {
            state.run_command("lsp-next-diagnostic", CommandArgs::default());
        }
        KeyCode::Char('p') => {
            state.run_command("lsp-prev-diagnostic", CommandArgs::default());
        }
        _ => {
            state.message = Some(format!("M-g {} is undefined", format_key(key)));
        }
    }
}

/// Handle a keystroke while the mini-buffer is active. Routes character
/// input into the minibuffer, maps control keys to state machine
/// operations (backspace, delete, nav, completion, history, submit,
/// cancel).
fn handle_minibuffer_key(state: &mut TuiAppState, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        // C-g / Esc — cancel.
        KeyCode::Char('g') if ctrl => state.minibuffer_cancel(),
        KeyCode::Esc => state.minibuffer_cancel(),
        // Enter — submit.
        KeyCode::Enter => state.minibuffer_submit(),
        // Editing.
        KeyCode::Backspace => {
            state.minibuffer.backspace();
            state.minibuffer_refresh_completions();
        }
        KeyCode::Delete => {
            state.minibuffer.delete_forward();
            state.minibuffer_refresh_completions();
        }
        KeyCode::Left => state.minibuffer.move_left(),
        KeyCode::Right => state.minibuffer.move_right(),
        KeyCode::Home => state.minibuffer.move_to_start(),
        KeyCode::End => state.minibuffer.move_to_end(),
        // Completion list navigation.
        KeyCode::Down => state.minibuffer.select_next(),
        KeyCode::Up => state.minibuffer.select_prev(),
        KeyCode::Tab => state.minibuffer_complete(),
        KeyCode::BackTab => state.minibuffer.select_prev(),
        // History (M-p / M-n).
        KeyCode::Char('p') if alt => {
            state.minibuffer.history_prev();
            state.minibuffer_refresh_completions();
        }
        KeyCode::Char('n') if alt => {
            state.minibuffer.history_next();
            state.minibuffer_refresh_completions();
        }
        // Self-insert (no modifiers, or only shift for uppercase).
        KeyCode::Char(c) if !ctrl && !alt => {
            state.minibuffer.add_char(c);
            state.minibuffer_refresh_completions();
        }
        _ => {}
    }
}

/// Handle the key after Escape (Meta) prefix.
///
/// Also supports native Alt-modified keys (Alt-x works as well as Esc-x
/// on terminals that send them as a single event) — caller dispatches
/// here from either the Esc prefix path or an Alt key press.
fn handle_meta_key(state: &mut TuiAppState, key: KeyEvent) {
    match key.code {
        // M-< / M-> — buffer start / end.
        KeyCode::Char('<') => {
            state.run_command("beginning-of-buffer", CommandArgs::default());
        }
        KeyCode::Char('>') => {
            state.run_command("end-of-buffer", CommandArgs::default());
        }
        // M-v — backward one screenful.
        KeyCode::Char('v') => {
            state.run_command("scroll-down-command", CommandArgs::default());
        }
        // M-d / M-DEL — word kill commands.
        KeyCode::Char('d') => {
            state.run_command("kill-word", CommandArgs::default());
        }
        KeyCode::Backspace => {
            state.run_command("backward-kill-word", CommandArgs::default());
        }
        // M-w / M-y — kill-ring copy and yank-pop.
        KeyCode::Char('w') => {
            state.run_command("kill-ring-save", CommandArgs::default());
        }
        KeyCode::Char('y') => {
            state.run_command("yank-pop", CommandArgs::default());
        }
        // M-% — query-replace. The TUI collects both strings, then
        // enters a one-key confirmation loop.
        KeyCode::Char('%') => {
            state.minibuffer_open(
                MiniBufferPrompt::FreeText {
                    label: "Query replace".to_string(),
                },
                Vec::new(),
                PendingMiniBufferAction::RunCommandWithInputs {
                    name: "query-replace".to_string(),
                    prompts: vec!["Query replace with".to_string()],
                    values: Vec::new(),
                },
            );
        }
        // M-x — execute-extended-command. Opens the minibuffer with
        // command-name completion over the full command registry.
        KeyCode::Char('x') => {
            let candidates = state.command_completion_candidates();
            state.minibuffer_open(
                rele_server::minibuffer::MiniBufferPrompt::Command,
                candidates,
                PendingMiniBufferAction::RunCommand,
            );
        }
        // M-g — goto prefix: next key picks the chord (M-g n = next
        // diagnostic, M-g p = previous).
        KeyCode::Char('g') => {
            state.m_g_pending = true;
        }
        _ => {
            state.message = Some(format!("ESC {} is undefined", format_key(key)));
        }
    }
}

fn format_key(key: KeyEvent) -> String {
    match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "RET".to_string(),
        KeyCode::Esc => "ESC".to_string(),
        KeyCode::Backspace => "DEL".to_string(),
        KeyCode::Tab => "TAB".to_string(),
        _ => format!("{:?}", key.code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cx_cc_quits_from_initial_scratch_buffer() {
        let mut state = TuiAppState::new();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        assert!(state.c_x_pending);

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );

        assert!(!state.c_x_pending);
        assert!(state.should_quit);
    }

    #[test]
    fn cx_d_opens_dired_minibuffer() {
        let mut state = TuiAppState::new();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        );

        assert!(state.minibuffer.active);
        assert!(matches!(
            state.pending_minibuffer_action,
            Some(PendingMiniBufferAction::Dired)
        ));
    }

    #[test]
    fn cx_b_opens_switch_buffer_minibuffer() {
        let mut state = TuiAppState::new();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        );

        assert!(state.minibuffer.active);
        assert!(matches!(
            state.pending_minibuffer_action,
            Some(PendingMiniBufferAction::SwitchBuffer)
        ));
    }

    #[test]
    fn tab_completes_active_minibuffer_candidate() {
        let mut state = TuiAppState::new();
        state.get_or_create_named_buffer("notes.md");

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );
        handle_key(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(state.minibuffer.input, "notes.md");
    }

    #[test]
    fn cx_capital_b_shows_buffer_list() {
        let mut state = TuiAppState::new();
        state.get_or_create_named_buffer("notes.md");
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT),
        );

        assert_eq!(state.current_buffer_name, "*Buffer List*");
        assert_eq!(
            state.current_buffer_kind,
            rele_server::BufferKind::BufferList
        );
        assert!(state.document.text().contains("notes.md"));
    }

    #[test]
    fn cx_shift_b_shows_buffer_list_when_terminal_reports_lowercase() {
        let mut state = TuiAppState::new();
        state.get_or_create_named_buffer("notes.md");
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::SHIFT),
        );

        assert_eq!(state.current_buffer_name, "*Buffer List*");
        assert!(!state.minibuffer.active);
        assert!(state.document.text().contains("notes.md"));
    }

    #[test]
    fn ctrl_w_and_ctrl_y_route_through_elisp_kill_ring() {
        let mut state = TuiAppState::new();
        state.insert_text("abcdef");
        state.cursor.anchor = Some(1);
        state.cursor.position = 4;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.document.text(), "aef");
        assert_eq!(state.cursor.position, 1);

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.document.text(), "abcdef");
    }

    #[test]
    fn meta_w_copies_region_and_ctrl_y_yanks_it() {
        let mut state = TuiAppState::new();
        state.insert_text("abcdef");
        state.cursor.anchor = Some(1);
        state.cursor.position = 4;
        state.install_elisp_editor_callbacks();

        handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
        );
        assert_eq!(state.document.text(), "abcdef");
        assert!(!state.cursor.has_selection());

        state.cursor.position = state.document.len_chars();
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.document.text(), "abcdefbcd");
    }

    #[test]
    fn ctrl_space_sets_mark() {
        let mut state = TuiAppState::new();
        state.insert_text("abcdef");
        state.cursor.position = 3;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        );

        assert_eq!(state.cursor.anchor, Some(3));
    }

    #[test]
    fn ctrl_space_then_movement_creates_region_for_ctrl_w() {
        let mut state = TuiAppState::new();
        state.insert_text("abcdef");
        state.cursor.position = 1;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.cursor.selection(), Some((1, 3)));

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.document.text(), "adef");

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.document.text(), "abcdef");
    }

    #[test]
    fn cx_r_k_and_y_manage_rectangles_through_elisp() {
        let mut state = TuiAppState::new();
        state.insert_text("abcd\nefgh\nijkl");
        state.cursor.anchor = Some(1);
        state.cursor.position = 13;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );
        assert!(state.c_x_r_pending);
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        );

        assert_eq!(state.document.text(), "ad\neh\nil");
        assert_eq!(
            state.kill_ring.rectangle_buffer.as_ref(),
            Some(&vec!["bc".to_string(), "fg".to_string(), "jk".to_string()])
        );

        state.cursor.position = 1;
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );

        assert_eq!(state.document.text(), "abcd\nefgh\nijkl");
    }

    #[test]
    fn cx_r_t_prompts_and_runs_string_rectangle() {
        let mut state = TuiAppState::new();
        state.insert_text("abcde\nfghij\nklmno");
        state.cursor.anchor = Some(1);
        state.cursor.position = 15;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );

        assert!(state.minibuffer.active);
        assert!(matches!(
            state.pending_minibuffer_action,
            Some(PendingMiniBufferAction::RunCommandWithInput { ref name })
                if name == "string-rectangle"
        ));

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert_eq!(state.document.text(), "aXXde\nfXXij\nkXXno");
    }

    #[test]
    fn ctrl_s_searches_forward_through_elisp() {
        let mut state = TuiAppState::new();
        state.insert_text("alpha beta beta");
        state.cursor.position = 0;
        state.install_elisp_editor_callbacks();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        );
        for ch in "beta".chars() {
            handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            );
        }
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert_eq!(state.cursor.position, 10);
        assert_eq!(state.search_match_data.groups, vec![Some((6, 10))]);
    }

    #[test]
    fn search_backward_literal_moves_to_previous_match() {
        let mut state = TuiAppState::new();
        state.insert_text("alpha beta alpha");
        state.cursor.position = state.document.len_chars();

        assert!(state.search_backward_literal("alpha"));

        assert_eq!(state.cursor.position, 11);
    }

    #[test]
    fn meta_percent_query_replace_confirms_each_match() {
        let mut state = TuiAppState::new();
        state.insert_text("foo bar foo");
        state.cursor.position = 0;
        state.install_elisp_editor_callbacks();

        handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('%'), KeyModifiers::NONE),
        );
        for ch in "foo".chars() {
            handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            );
        }
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        for ch in "baz".chars() {
            handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            );
        }
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert!(state.query_replace.is_some());

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );

        assert!(state.query_replace.is_none());
        assert_eq!(state.document.text(), "baz bar foo");
    }

    #[test]
    fn query_replace_regexp_expands_captures() {
        let mut state = TuiAppState::new();
        state.insert_text("foo1 bar2");
        state.cursor.position = 0;
        state.install_elisp_editor_callbacks();

        state.query_replace_regexp_start(
            "\\([a-z]+\\)\\([0-9]\\)".to_string(),
            "\\2-\\1".to_string(),
        );

        assert!(state.query_replace.is_some());

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        );

        assert!(state.query_replace.is_none());
        assert_eq!(state.document.text(), "1-foo 2-bar");
    }
}
