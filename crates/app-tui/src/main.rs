mod commands;
mod state;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute};
use log::info;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rele_server::CommandArgs;

use state::TuiAppState;

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
    loop {
        terminal.draw(|frame| ui::draw(frame, state))?;

        // Clear message after rendering it once
        state.message = None;

        if state.should_quit {
            return Ok(());
        }

        // Poll for events with a timeout (allows responsive quit)
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            handle_key(state, key);
        }
    }
}

fn handle_key(state: &mut TuiAppState, key: KeyEvent) {
    // Handle C-x prefix
    if state.c_x_pending {
        state.c_x_pending = false;
        handle_cx_key(state, key);
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
            state.message = Some("Quit".to_string());
        }
        KeyCode::Char('x') if ctrl => {
            state.c_x_pending = true;
        }
        KeyCode::Char('/') if ctrl => state.run_command("undo", CommandArgs::default()),
        KeyCode::Char('v') if ctrl => {
            state.run_command("scroll-down-command", CommandArgs::default());
        }

        // ---- Alt bindings ----
        KeyCode::Char('<') if alt => {
            state.run_command("beginning-of-buffer", CommandArgs::default());
        }
        KeyCode::Char('>') if alt => {
            state.run_command("end-of-buffer", CommandArgs::default());
        }
        KeyCode::Char('v') if alt => {
            state.run_command("scroll-up-command", CommandArgs::default());
        }

        // ---- Arrow keys ----
        KeyCode::Up => state.run_command("previous-line", CommandArgs::default()),
        KeyCode::Down => state.run_command("next-line", CommandArgs::default()),
        KeyCode::Left => state.run_command("backward-char", CommandArgs::default()),
        KeyCode::Right => state.run_command("forward-char", CommandArgs::default()),
        KeyCode::Home => state.run_command("move-beginning-of-line", CommandArgs::default()),
        KeyCode::End => state.run_command("move-end-of-line", CommandArgs::default()),
        KeyCode::PageUp => state.run_command("scroll-up-command", CommandArgs::default()),
        KeyCode::PageDown => state.run_command("scroll-down-command", CommandArgs::default()),

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
        KeyCode::Char('s') if ctrl => state.run_command("save-buffer", CommandArgs::default()),
        KeyCode::Char('c') if ctrl => {
            state.run_command("save-buffers-kill-emacs", CommandArgs::default());
        }
        KeyCode::Char('u') => state.run_command("undo", CommandArgs::default()),
        _ => {
            state.message = Some(format!("C-x {} is undefined", format_key(key)));
        }
    }
}

/// Handle the key after Escape (Meta) prefix.
fn handle_meta_key(state: &mut TuiAppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('<') => {
            state.run_command("beginning-of-buffer", CommandArgs::default());
        }
        KeyCode::Char('>') => {
            state.run_command("end-of-buffer", CommandArgs::default());
        }
        KeyCode::Char('v') => {
            state.run_command("scroll-up-command", CommandArgs::default());
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
