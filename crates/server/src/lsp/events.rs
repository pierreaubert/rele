//! LSP event types sent from background I/O tasks to the UI thread.

use lsp_types::{CompletionItem, Diagnostic, HoverContents, Location, MarkedString, TextEdit, Uri};

/// Convert LSP `HoverContents` (several possible shapes) to a single string
/// suitable for display in the editor UI. Used by both GPUI and TUI clients
/// to avoid duplicating the match arms.
pub fn hover_contents_to_string(contents: HoverContents) -> String {
    fn marked_to_string(s: MarkedString) -> String {
        match s {
            MarkedString::String(s) => s,
            MarkedString::LanguageString(ls) => ls.value,
        }
    }
    match contents {
        HoverContents::Scalar(s) => marked_to_string(s),
        HoverContents::Array(arr) => arr
            .into_iter()
            .map(marked_to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        HoverContents::Markup(m) => m.value,
    }
}

/// An event produced by an LSP client and consumed by the editor UI.
#[derive(Debug)]
pub enum LspEvent {
    /// Server published new diagnostics for a document.
    Diagnostics {
        uri: Uri,
        diagnostics: Vec<Diagnostic>,
        version: Option<i32>,
    },

    /// Response to a completion request.
    CompletionResponse {
        request_id: i64,
        items: Vec<CompletionItem>,
    },

    /// Response to a hover request.
    HoverResponse {
        request_id: i64,
        contents: Option<HoverContents>,
    },

    /// Response to a go-to-definition request.
    DefinitionResponse {
        request_id: i64,
        locations: Vec<Location>,
    },

    /// Response to a find-references request.
    ReferencesResponse {
        request_id: i64,
        locations: Vec<Location>,
    },

    /// Response to a formatting request.
    FormattingResponse {
        request_id: i64,
        edits: Vec<TextEdit>,
    },

    /// A language server has finished initialization.
    ServerStarted { server_name: String },

    /// A language server process exited.
    ServerExited {
        server_name: String,
        status: Option<i32>,
    },

    /// A protocol-level or transport error.
    Error { message: String },
}
