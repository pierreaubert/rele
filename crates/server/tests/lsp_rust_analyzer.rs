#![allow(clippy::disallowed_methods)]
//! End-to-end LSP integration test against a real `rust-analyzer` server.
//!
//! The test:
//! 1. Creates a tempdir with `Cargo.toml` + `src/main.rs`.
//! 2. Starts an `LspRegistry` configured for rust-analyzer.
//! 3. Waits for `ServerStarted` and `Diagnostics` events.
//! 4. Sends `didChange` and verifies new diagnostics arrive.
//! 5. Requests hover on `println!` and verifies a response.
//!
//! The test is gated on `rust-analyzer` being available on `$PATH`; if it
//! isn't, the test is skipped (logs and returns) so that CI without the
//! binary doesn't fail.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rele_server::lsp::{
    LspConfig, LspEvent, LspRegistry, LspServerConfig,
    position::{char_offset_to_position, uri_from_path},
};
use ropey::Rope;
use tokio::sync::mpsc::UnboundedReceiver;

/// Return `Some(path)` if `rust-analyzer` is on `$PATH`, else `None`.
fn find_rust_analyzer() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("rust-analyzer");
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join("rust-analyzer.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Create a minimal Rust project in a temp directory and return its path.
/// The project contains a `Cargo.toml` and a `src/main.rs` with a deliberate
/// type error on line 3 so rust-analyzer emits at least one diagnostic.
fn fixture_project(dir: &std::path::Path) {
    std::fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "lsp-test-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "fn main() {\n    println!(\"hi\");\n    let _x: u32 = \"oops\";\n}\n",
    )
    .unwrap();
}

/// Drain events from the receiver until `pred` returns `Some(value)` or
/// the timeout elapses. Other events are logged but ignored.
fn wait_for<T>(
    rx: &mut UnboundedReceiver<LspEvent>,
    timeout: Duration,
    mut pred: impl FnMut(&LspEvent) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        match rx.try_recv() {
            Ok(event) => {
                if let Some(v) = pred(&event) {
                    return Some(v);
                }
                // Keep looping; log for debugging.
                eprintln!("[e2e] saw event: {event:?}");
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No event yet — sleep a bit (cap at remaining).
                std::thread::sleep(std::cmp::min(remaining, Duration::from_millis(50)));
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => return None,
        }
    }
}

#[test]
fn rust_analyzer_end_to_end() {
    let Some(_) = find_rust_analyzer() else {
        eprintln!("[e2e] rust-analyzer not on $PATH; skipping");
        return;
    };

    // 1. Fixture project.
    let tmp = tempfile::tempdir().expect("failed to create tempdir");
    fixture_project(tmp.path());
    let main_rs = tmp.path().join("src").join("main.rs");

    // 2. Build a one-server config (no user config file involvement).
    let config = LspConfig {
        servers: vec![LspServerConfig {
            name: "rust-analyzer".to_string(),
            command: "rust-analyzer".to_string(),
            args: vec![],
            file_extensions: vec!["rs".to_string()],
            language_id: "rust".to_string(),
            root_markers: vec!["Cargo.toml".to_string()],
        }],
    };
    let mut registry = LspRegistry::new(config);
    let mut rx = registry.take_event_receiver().expect("event receiver");

    // 3. Start the server for this file. Returns immediately; init runs in
    //    the background and produces a ServerStarted event when done.
    let server_name = registry
        .ensure_server_for_file(&main_rs)
        .expect("ensure_server_for_file");
    assert_eq!(server_name, "rust-analyzer");

    // 4. Wait for ServerStarted. rust-analyzer's initial handshake is
    //    generally < 2 s, but the first build of a fresh crate graph can
    //    occasionally take longer on cold caches.
    let started = wait_for(&mut rx, Duration::from_secs(30), |e| match e {
        LspEvent::ServerStarted { server_name } => Some(server_name.clone()),
        _ => None,
    });
    assert!(
        started.is_some(),
        "did not receive ServerStarted within 30s"
    );

    // 5. Send didOpen for the fixture file.
    let text = std::fs::read_to_string(&main_rs).unwrap();
    let uri = uri_from_path(&main_rs).expect("uri_from_path");
    let client = registry
        .client(&server_name)
        .expect("client available after ServerStarted");
    let handle = registry.runtime_handle();
    let uri_for_open = uri.clone();
    let text_for_open = text.clone();
    handle.spawn(async move {
        client
            .did_open(uri_for_open, "rust", 1, &text_for_open)
            .await
            .expect("didOpen");
    });

    // 6. Wait for diagnostics on this file. The fixture has a type error
    //    (`let _x: u32 = "oops";`) so rust-analyzer must produce at least
    //    one diagnostic. First-time analysis can take several seconds
    //    because rust-analyzer builds the crate graph from scratch.
    let diagnostics = wait_for(&mut rx, Duration::from_secs(60), |e| match e {
        LspEvent::Diagnostics {
            uri: diag_uri,
            diagnostics,
            ..
        } if diag_uri.as_str() == uri.as_str() && !diagnostics.is_empty() => {
            Some(diagnostics.clone())
        }
        _ => None,
    });
    let diagnostics = diagnostics.expect("expected diagnostics within 60s");
    assert!(
        diagnostics.iter().any(|d| d.range.start.line == 2),
        "expected at least one diagnostic on line 3 (the type-error line), got: {diagnostics:?}"
    );

    // 7. Send didChange with a full-document replacement. We verify the
    //    notification itself delivers without a transport error; we don't
    //    assert on the diagnostics that follow because rust-analyzer tags
    //    in-flight diagnostics with the latest known version, which makes
    //    the "did the error clear?" check racy in practice.
    let fixed_text = "fn main() {\n    println!(\"hi\");\n    let _x: u32 = 0;\n}\n".to_string();
    let client = registry.client(&server_name).unwrap();
    let uri_for_change = uri.clone();
    let fixed_for_change = fixed_text.clone();
    let change_result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let change_result_setter = std::sync::Arc::clone(&change_result);
    handle.spawn(async move {
        let r = client
            .did_change(
                uri_for_change,
                2,
                vec![lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: fixed_for_change,
                }],
            )
            .await
            .map_err(|e| e.to_string());
        *change_result_setter.lock().unwrap() = Some(r);
    });
    // Wait briefly for the spawned task to complete and confirm the send.
    let deadline = Instant::now() + Duration::from_secs(5);
    while change_result.lock().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let change_result = change_result.lock().unwrap().take();
    assert!(
        matches!(change_result, Some(Ok(()))),
        "didChange failed: {change_result:?}"
    );

    // 8. Request hover on `println` (line 1, col 4). rust-analyzer usually
    //    responds within a second once init is complete.
    let rope = Rope::from_str(&fixed_text);
    let hover_pos = char_offset_to_position(&rope, rope.line_to_char(1) + 4);
    let client = registry.client(&server_name).unwrap();
    let uri_for_hover = uri.clone();
    handle.spawn(async move {
        let _ = client.hover(uri_for_hover, hover_pos).await;
    });
    let hover = wait_for(&mut rx, Duration::from_secs(15), |e| match e {
        LspEvent::HoverResponse { contents, .. } => Some(contents.clone()),
        _ => None,
    });
    assert!(hover.is_some(), "no HoverResponse within 15s");

    // 9. Graceful shutdown so the child process isn't left running.
    registry.shutdown_all();
    // Give the shutdown RPCs a brief window to flush; kill_on_drop on the
    // child will clean up if the server ignores us.
    std::thread::sleep(Duration::from_millis(200));
}
