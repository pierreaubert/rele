//! JSON-RPC transport layer for LSP communication.
//!
//! Handles Content-Length framed message reading/writing over stdin/stdout
//! of a language server child process.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, mpsc, oneshot};

use super::events::LspEvent;

/// A JSON-RPC request or notification message.
#[derive(Debug, Serialize)]
struct JsonRpcMessage {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// A JSON-RPC response message.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub method: Option<String>,
    pub params: Option<Value>,
}

/// A JSON-RPC error object.
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

/// Handles writing JSON-RPC messages to a language server's stdin.
pub struct LspTransport {
    writer: Arc<Mutex<ChildStdin>>,
    next_id: std::sync::atomic::AtomicI64,
}

impl LspTransport {
    pub fn new(stdin: ChildStdin) -> Self {
        Self {
            writer: Arc::new(Mutex::new(stdin)),
            next_id: std::sync::atomic::AtomicI64::new(1),
        }
    }

    /// Send a JSON-RPC request and return its id.
    pub async fn send_request<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<i64, std::io::Error> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let msg = JsonRpcMessage {
            jsonrpc: "2.0",
            id: Some(id),
            method: method.to_string(),
            params: Some(serde_json::to_value(params).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?),
        };
        self.write_message(&msg).await?;
        Ok(id)
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    pub async fn send_notification<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<(), std::io::Error> {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0",
            id: None,
            method: method.to_string(),
            params: Some(serde_json::to_value(params).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?),
        };
        self.write_message(&msg).await
    }

    async fn write_message(&self, msg: &JsonRpcMessage) -> Result<(), std::io::Error> {
        let body = serde_json::to_string(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut writer = self.writer.lock().await;
        writer.write_all(header.as_bytes()).await?;
        writer.write_all(body.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }
}

impl std::fmt::Debug for LspTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspTransport")
            .field("next_id", &self.next_id)
            .finish()
    }
}

/// Map of pending request id -> oneshot sender for the response.
pub type PendingRequests = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, JsonRpcError>>>>>;

/// Read Content-Length framed JSON-RPC messages from the server's stdout.
///
/// Dispatches:
/// - Responses (have `id`): sent to the matching pending-request oneshot channel.
/// - Notifications (have `method`, no `id`): converted to `LspEvent` and sent
///   to the event channel.
///
/// Runs until the stdout stream is closed (server exit).
pub async fn reader_loop(
    stdout: ChildStdout,
    pending: PendingRequests,
    event_tx: mpsc::UnboundedSender<LspEvent>,
    server_name: String,
) {
    let mut reader = BufReader::new(stdout);

    loop {
        // Parse headers.
        let content_length = match read_content_length(&mut reader).await {
            Ok(Some(len)) => len,
            Ok(None) => break, // EOF
            Err(e) => {
                let _ = event_tx.send(LspEvent::Error {
                    message: format!("[{server_name}] header parse error: {e}"),
                });
                break;
            }
        };

        // Read body.
        let mut body = vec![0u8; content_length];
        if let Err(e) = reader.read_exact(&mut body).await {
            let _ = event_tx.send(LspEvent::Error {
                message: format!("[{server_name}] body read error: {e}"),
            });
            break;
        }

        let msg: JsonRpcResponse = match serde_json::from_slice(&body) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[{server_name}] failed to parse message: {e}");
                continue;
            }
        };

        // Response (has id, no method).
        if let Some(id) = msg.id
            && msg.method.is_none()
        {
            let mut pending_map = pending.lock().await;
            if let Some(tx) = pending_map.remove(&id) {
                let result = if let Some(error) = msg.error {
                    Err(error)
                } else {
                    Ok(msg.result.unwrap_or(Value::Null))
                };
                let _ = tx.send(result);
            }
            continue;
        }

        // Server-initiated notification or request.
        if let Some(method) = &msg.method {
            dispatch_notification(method, msg.params.as_ref(), &event_tx);
        }
    }

    // Server stdout closed — report exit.
    let _ = event_tx.send(LspEvent::ServerExited {
        server_name,
        status: None,
    });
}

/// Parse `Content-Length:` headers. Returns `None` on EOF.
async fn read_content_length(
    reader: &mut BufReader<ChildStdout>,
) -> Result<Option<usize>, std::io::Error> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Empty line separates headers from body.
            return Ok(content_length);
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            content_length = Some(len);
        }
        // Ignore other headers (e.g. Content-Type).
    }
}

/// Convert a server notification into an `LspEvent`.
fn dispatch_notification(
    method: &str,
    params: Option<&Value>,
    event_tx: &mpsc::UnboundedSender<LspEvent>,
) {
    match method {
        "textDocument/publishDiagnostics" => {
            if let Some(params) = params
                && let Ok(diag) =
                    serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(params.clone())
            {
                let _ = event_tx.send(LspEvent::Diagnostics {
                    uri: diag.uri,
                    diagnostics: diag.diagnostics,
                    version: diag.version,
                });
            }
        }
        "window/logMessage" | "window/showMessage" => {
            if let Some(params) = params
                && let Some(message) = params.get("message").and_then(Value::as_str)
            {
                log::info!("[lsp] {method}: {message}");
            }
        }
        _ => {
            log::debug!("[lsp] unhandled notification: {method}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_request_serialization() {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0",
            id: Some(1),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({"rootUri": "file:///tmp"})),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn json_rpc_notification_no_id() {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0",
            id: None,
            method: "initialized".to_string(),
            params: Some(serde_json::json!({})),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn error_response_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(2));
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
    }
}
