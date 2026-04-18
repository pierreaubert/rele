//! Per-buffer LSP state tracking.

use lsp_types::{Diagnostic, Uri};

/// LSP-related state for a single open buffer.
#[derive(Debug, Clone)]
pub struct LspBufferState {
    /// Document URI as expected by the language server.
    pub uri: Uri,
    /// LSP language identifier (e.g. "rust", "markdown").
    pub language_id: String,
    /// Name of the server handling this buffer.
    pub server_name: String,
    /// Current diagnostics published by the server.
    pub diagnostics: Vec<Diagnostic>,
    /// Document version counter sent to the server (i32 per LSP spec).
    pub lsp_version: i32,
    /// Whether `textDocument/didOpen` has been successfully sent.
    /// If the server is still initializing when the buffer opens, we
    /// record the buffer here but defer the notification; once the server
    /// reports `ServerStarted`, we flush the `didOpen`.
    pub did_open_sent: bool,
}

impl LspBufferState {
    /// Create a new buffer state from a file path.
    pub fn new(uri: Uri, language_id: String, server_name: String) -> Self {
        Self {
            uri,
            language_id,
            server_name,
            diagnostics: Vec::new(),
            lsp_version: 0,
            did_open_sent: false,
        }
    }

    /// Increment the version and return the new value.
    pub fn next_version(&mut self) -> i32 {
        self.lsp_version += 1;
        self.lsp_version
    }
}
