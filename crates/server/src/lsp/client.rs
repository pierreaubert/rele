//! LSP client: manages a single language server process.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use lsp_types::{
    ClientCapabilities, CompletionParams, DocumentFormattingParams, FormattingOptions,
    GotoDefinitionParams, HoverParams, InitializeParams, InitializedParams, Position,
    ReferenceContext, ReferenceParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkspaceFolder,
};
use serde_json::Value;
use tokio::sync::{Mutex, mpsc, oneshot};

use super::config::LspServerConfig;
use super::events::LspEvent;
use super::protocol::{JsonRpcError, LspTransport, PendingRequests, reader_loop};

/// A running LSP client connected to a language server process.
pub struct LspClient {
    pub config: LspServerConfig,
    transport: Arc<LspTransport>,
    pending: PendingRequests,
    capabilities: Option<lsp_types::ServerCapabilities>,
    event_tx: mpsc::UnboundedSender<LspEvent>,
    _child: tokio::process::Child,
}

impl std::fmt::Debug for LspClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspClient")
            .field("config", &self.config.name)
            .field("transport", &self.transport)
            .finish()
    }
}

impl LspClient {
    /// Spawn the language server process and start the reader loop.
    pub async fn start(
        config: &LspServerConfig,
        event_tx: mpsc::UnboundedSender<LspEvent>,
    ) -> Result<Self, std::io::Error> {
        let mut child = tokio::process::Command::new(&config.command)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::other("failed to capture stdin")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            std::io::Error::other("failed to capture stdout")
        })?;

        let transport = Arc::new(LspTransport::new(stdin));
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        // Spawn the reader loop as a background task.
        let reader_pending = Arc::clone(&pending);
        let reader_event_tx = event_tx.clone();
        let server_name = config.name.clone();
        tokio::spawn(async move {
            reader_loop(stdout, reader_pending, reader_event_tx, server_name).await;
        });

        Ok(Self {
            config: config.clone(),
            transport,
            pending,
            capabilities: None,
            event_tx,
            _child: child,
        })
    }

    /// Send `initialize` request and `initialized` notification.
    pub async fn initialize(&mut self, root_uri: Uri) -> Result<(), Box<dyn std::error::Error>> {
        #[allow(deprecated)] // root_uri is deprecated but widely supported
        let params = InitializeParams {
            root_uri: Some(root_uri.clone()),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: "root".to_string(),
            }]),
            capabilities: ClientCapabilities::default(),
            ..InitializeParams::default()
        };

        let result = self.request("initialize", params).await?;
        let init_result: lsp_types::InitializeResult = serde_json::from_value(result)?;
        self.capabilities = Some(init_result.capabilities);

        // Send initialized notification.
        self.transport
            .send_notification("initialized", InitializedParams {})
            .await?;

        let _ = self.event_tx.send(LspEvent::ServerStarted {
            server_name: self.config.name.clone(),
        });

        Ok(())
    }

    /// Send `shutdown` request followed by `exit` notification.
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        // shutdown is a request with null params.
        let _ = self.request("shutdown", Value::Null).await;
        // exit is a notification.
        self.transport
            .send_notification("exit", Value::Null)
            .await?;
        Ok(())
    }

    /// Return the server's capabilities, if initialized.
    pub fn capabilities(&self) -> Option<&lsp_types::ServerCapabilities> {
        self.capabilities.as_ref()
    }

    // ---- Document synchronization ----

    /// Notify the server that a document was opened.
    pub async fn did_open(
        &self,
        uri: Uri,
        language_id: &str,
        version: i32,
        text: &str,
    ) -> Result<(), std::io::Error> {
        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version,
                text: text.to_string(),
            },
        };
        self.transport
            .send_notification("textDocument/didOpen", params)
            .await
    }

    /// Notify the server of document changes (incremental or full).
    pub async fn did_change(
        &self,
        uri: Uri,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<(), std::io::Error> {
        let params = lsp_types::DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: changes,
        };
        self.transport
            .send_notification("textDocument/didChange", params)
            .await
    }

    /// Notify the server that a document was saved.
    pub async fn did_save(&self, uri: Uri, text: Option<&str>) -> Result<(), std::io::Error> {
        let params = lsp_types::DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: text.map(str::to_string),
        };
        self.transport
            .send_notification("textDocument/didSave", params)
            .await
    }

    /// Notify the server that a document was closed.
    pub async fn did_close(&self, uri: Uri) -> Result<(), std::io::Error> {
        let params = lsp_types::DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };
        self.transport
            .send_notification("textDocument/didClose", params)
            .await
    }

    // ---- Requests (return request id; response comes via LspEvent) ----

    /// Request completions at a position. Response arrives as `LspEvent::CompletionResponse`.
    pub async fn completion(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };
        let id = self.transport.send_request("textDocument/completion", params).await?;

        // Spawn a task to wait for the response and convert it to an LspEvent.
        self.spawn_response_handler(id, move |result, event_tx| {
            let items = parse_completion_result(&result);
            let _ = event_tx.send(LspEvent::CompletionResponse {
                request_id: id,
                items,
            });
        })
        .await;

        Ok(id)
    }

    /// Request hover info at a position.
    pub async fn hover(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
        };
        let id = self.transport.send_request("textDocument/hover", params).await?;

        self.spawn_response_handler(id, move |result, event_tx| {
            let contents = serde_json::from_value::<lsp_types::Hover>(result)
                .ok()
                .map(|h| h.contents);
            let _ = event_tx.send(LspEvent::HoverResponse {
                request_id: id,
                contents,
            });
        })
        .await;

        Ok(id)
    }

    /// Request go-to-definition.
    pub async fn definition(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let id = self
            .transport
            .send_request("textDocument/definition", params)
            .await?;

        self.spawn_response_handler(id, move |result, event_tx| {
            let locations = parse_location_result(&result);
            let _ = event_tx.send(LspEvent::DefinitionResponse {
                request_id: id,
                locations,
            });
        })
        .await;

        Ok(id)
    }

    /// Request find-references.
    pub async fn references(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        };
        let id = self
            .transport
            .send_request("textDocument/references", params)
            .await?;

        self.spawn_response_handler(id, move |result, event_tx| {
            let locations: Vec<lsp_types::Location> =
                serde_json::from_value(result).unwrap_or_default();
            let _ = event_tx.send(LspEvent::ReferencesResponse {
                request_id: id,
                locations,
            });
        })
        .await;

        Ok(id)
    }

    /// Request document formatting.
    pub async fn formatting(
        &self,
        uri: Uri,
        options: FormattingOptions,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options,
            work_done_progress_params: Default::default(),
        };
        let id = self
            .transport
            .send_request("textDocument/formatting", params)
            .await?;

        self.spawn_response_handler(id, move |result, event_tx| {
            let edits: Vec<lsp_types::TextEdit> =
                serde_json::from_value(result).unwrap_or_default();
            let _ = event_tx.send(LspEvent::FormattingResponse {
                request_id: id,
                edits,
            });
        })
        .await;

        Ok(id)
    }

    // ---- Internal helpers ----

    /// Send a request and wait for the response value.
    async fn request<P: serde::Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = self.transport.send_request(method, params).await?;
        let (tx, rx) = oneshot::channel();
        {
            let mut pending_map = self.pending.lock().await;
            pending_map.insert(id, tx);
        }
        let result = rx.await??;
        Ok(result)
    }

    /// Register a oneshot in pending and spawn a task that waits for the
    /// response, then calls `handler` with the result.
    async fn spawn_response_handler<F>(&self, id: i64, handler: F)
    where
        F: FnOnce(Value, &mpsc::UnboundedSender<LspEvent>) + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending_map = self.pending.lock().await;
            pending_map.insert(id, tx);
        }
        let event_tx = self.event_tx.clone();
        let server_name = self.config.name.clone();
        tokio::spawn(async move {
            match rx.await {
                Ok(Ok(value)) => handler(value, &event_tx),
                Ok(Err(error)) => {
                    let _ = event_tx.send(LspEvent::Error {
                        message: format!("[{server_name}] request {id} error: {}", error.message),
                    });
                }
                Err(_) => {
                    // Oneshot dropped — server likely exited.
                }
            }
        });
    }
}

/// Parse a completion response (can be a list or a `CompletionList`).
fn parse_completion_result(value: &Value) -> Vec<lsp_types::CompletionItem> {
    // Try as CompletionList first.
    if let Ok(list) = serde_json::from_value::<lsp_types::CompletionList>(value.clone()) {
        return list.items;
    }
    // Try as Vec<CompletionItem>.
    serde_json::from_value::<Vec<lsp_types::CompletionItem>>(value.clone()).unwrap_or_default()
}

/// Parse a definition response (can be a single Location, Vec<Location>,
/// or Vec<LocationLink>).
fn parse_location_result(value: &Value) -> Vec<lsp_types::Location> {
    // Try as single Location.
    if let Ok(loc) = serde_json::from_value::<lsp_types::Location>(value.clone()) {
        return vec![loc];
    }
    // Try as Vec<Location>.
    if let Ok(locs) = serde_json::from_value::<Vec<lsp_types::Location>>(value.clone()) {
        return locs;
    }
    // Try as Vec<LocationLink> and convert.
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(value.clone()) {
        return links
            .into_iter()
            .map(|link| lsp_types::Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect();
    }
    Vec::new()
}

// Implement From for the oneshot error to make `?` work in request().
impl From<JsonRpcError> for Box<dyn std::error::Error> {
    fn from(e: JsonRpcError) -> Self {
        Box::new(std::io::Error::other(format!(
            "LSP error {}: {}",
            e.code, e.message
        )))
    }
}
