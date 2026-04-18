//! LSP client infrastructure.
//!
//! This module provides a full Language Server Protocol client that can
//! communicate with any LSP-compliant language server over stdin/stdout.
//!
//! # Architecture
//!
//! - [`config`] — Server configuration and file-extension mapping.
//! - [`protocol`] — JSON-RPC transport (Content-Length framing).
//! - [`client`] — Per-server client: lifecycle, document sync, requests.
//! - [`registry`] — Manages multiple server instances, lazy startup.
//! - [`events`] — Event types sent from I/O tasks to the UI thread.
//! - [`buffer_state`] — Per-buffer LSP metadata (URI, diagnostics, version).
//! - [`position`] — Char-offset ↔ LSP position conversion (UTF-16).

pub mod buffer_state;
pub mod client;
pub mod config;
pub mod events;
pub mod position;
pub mod protocol;
pub mod registry;

pub use buffer_state::LspBufferState;
pub use config::{LspConfig, LspServerConfig};
pub use events::{LspEvent, hover_contents_to_string};
pub use registry::LspRegistry;
