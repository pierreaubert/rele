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
        KeyCode::Char('g') if ctrl => {
            // C-g — cancel prefix / clear message
            state.universal_arg = None;
            state.c_x_pending = false;
            state.meta_pending = false;
            state.m_g_pending = false;
            state.message = Some("Quit".to_string());
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

/// Handle the second key after C-x prefix.
fn handle_cx_key(state: &mut TuiAppState, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        // C-x C-s — save current buffer.
        KeyCode::Char('s') if ctrl => state.run_command("save-buffer", CommandArgs::default()),
        // C-x C-c — save and quit.
        KeyCode::Char('c') if ctrl => {
            state.run_command("save-buffers-kill-emacs", CommandArgs::default());
        }
        // C-x u — undo.
        KeyCode::Char('u') => state.run_command("undo", CommandArgs::default()),
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
        KeyCode::Char('b') => {
            let candidates = state.buffer_names();
            state.minibuffer_open(
                MiniBufferPrompt::SwitchBuffer,
                candidates,
                PendingMiniBufferAction::SwitchBuffer,
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
        _ => {
            state.message = Some(format!("C-x {} is undefined", format_key(key)));
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
        KeyCode::Backspace => state.minibuffer.backspace(),
        KeyCode::Delete => state.minibuffer.delete_forward(),
        KeyCode::Left => state.minibuffer.move_left(),
        KeyCode::Right => state.minibuffer.move_right(),
        KeyCode::Home => state.minibuffer.move_to_start(),
        KeyCode::End => state.minibuffer.move_to_end(),
        // Completion list navigation.
        KeyCode::Down => state.minibuffer.select_next(),
        KeyCode::Up => state.minibuffer.select_prev(),
        KeyCode::Tab => state.minibuffer.complete(),
        // History (M-p / M-n).
        KeyCode::Char('p') if alt => state.minibuffer.history_prev(),
        KeyCode::Char('n') if alt => state.minibuffer.history_next(),
        // Self-insert (no modifiers, or only shift for uppercase).
        KeyCode::Char(c) if !ctrl && !alt => state.minibuffer.add_char(c),
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
        // M-x — execute-extended-command. Opens the minibuffer with
        // command-name completion over the full command registry.
        KeyCode::Char('x') => {
            let candidates: Vec<String> = state.commands.names().to_vec();
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
