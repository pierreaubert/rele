//! LSP server lifecycle manager.
//!
//! Manages running language server instances, keyed by server name.
//! Lazily spawns servers on first `didOpen` for a matching file extension.
//!
//! Server startup is fully asynchronous: `ensure_server_for_file` returns
//! immediately after kicking off a background init task, so the UI thread
//! never blocks waiting for a slow language server.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::mpsc;

use super::client::LspClient;
use super::config::{LspConfig, LspServerConfig, find_project_root};
use super::events::LspEvent;
use super::position::uri_from_path;

/// Shared map of server-name -> running client.
/// Populated by the background init task; read by the UI thread.
type ClientMap = Arc<RwLock<HashMap<String, Arc<LspClient>>>>;

/// Manages the lifecycle of language server processes.
pub struct LspRegistry {
    config: LspConfig,
    /// Running clients, shared with background init tasks that insert into it.
    clients: ClientMap,
    /// Server names whose init task has been spawned but not yet completed.
    /// Used to debounce repeated `ensure_server_for_file` calls.
    starting: Arc<RwLock<HashSet<String>>>,
    event_tx: mpsc::UnboundedSender<LspEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<LspEvent>>,
    runtime: tokio::runtime::Runtime,
}

impl std::fmt::Debug for LspRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspRegistry")
            .field(
                "servers",
                &self.clients.read().keys().cloned().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl LspRegistry {
    /// Create a new registry with the given configuration.
    /// The event receiver must be taken via `take_event_receiver()` by the UI layer.
    pub fn new(config: LspConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("lsp-io")
            .enable_all()
            .build()
            .expect("failed to create LSP tokio runtime");
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            starting: Arc::new(RwLock::new(HashSet::new())),
            event_tx,
            event_rx: Some(event_rx),
            runtime,
        }
    }

    /// Take the event receiver. Called once by the UI layer to poll for LSP events.
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<LspEvent>> {
        self.event_rx.take()
    }

    /// Get the event sender (for passing to additional components).
    pub fn event_sender(&self) -> mpsc::UnboundedSender<LspEvent> {
        self.event_tx.clone()
    }

    /// Find the server config for a file extension.
    pub fn config_for_extension(&self, ext: &str) -> Option<&LspServerConfig> {
        self.config.find_server_for_extension(ext)
    }

    /// Ensure a language server is running for the given file.
    ///
    /// Returns the server name if one is configured. If the server is not yet
    /// started, spawns a background init task; the client will appear in
    /// `client()` once init completes (signaled by `LspEvent::ServerStarted`).
    /// This function never blocks.
    pub fn ensure_server_for_file(&mut self, file_path: &std::path::Path) -> Option<String> {
        let ext = file_path.extension()?.to_str()?;
        let server_config = self.config.find_server_for_extension(ext)?.clone();
        let server_name = server_config.name.clone();

        // Already running or starting — nothing to do.
        if self.clients.read().contains_key(&server_name) {
            return Some(server_name);
        }
        {
            let mut starting = self.starting.write();
            if starting.contains(&server_name) {
                return Some(server_name);
            }
            starting.insert(server_name.clone());
        }

        // Find project root.
        let root = find_project_root(file_path, &server_config.root_markers)
            .unwrap_or_else(|| file_path.parent().unwrap_or(file_path).to_path_buf());
        let root_uri = uri_from_path(&root)?;

        // Kick off init on the runtime — do NOT block the UI thread.
        let event_tx = self.event_tx.clone();
        let clients = Arc::clone(&self.clients);
        let starting = Arc::clone(&self.starting);
        let config = server_config.clone();
        let server_name_for_task = server_name.clone();
        self.runtime.spawn(async move {
            let result: Result<LspClient, std::io::Error> = async {
                let mut client = LspClient::start(&config, event_tx.clone()).await?;
                // `initialize` returns Box<dyn Error>, which isn't Send.
                // Convert immediately to a Send-safe string error.
                client
                    .initialize(root_uri)
                    .await
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                Ok(client)
            }
            .await;

            match result {
                Ok(client) => {
                    clients
                        .write()
                        .insert(server_name_for_task.clone(), Arc::new(client));
                }
                Err(e) => {
                    log::error!("Failed to start LSP server {}: {}", server_name_for_task, e);
                    let _ = event_tx.send(LspEvent::Error {
                        message: format!("Failed to start {}: {}", server_name_for_task, e),
                    });
                }
            }
            // Mark as no-longer-starting regardless of outcome; on success
            // the client is in `clients`, on failure a retry is allowed.
            starting.write().remove(&server_name_for_task);
        });

        Some(server_name)
    }

    /// Get the client for a server name, if it has finished initializing.
    pub fn client(&self, server_name: &str) -> Option<Arc<LspClient>> {
        self.clients.read().get(server_name).cloned()
    }

    /// Get a handle to the tokio runtime for spawning LSP-related async work.
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// Shut down a single named language server.
    /// Returns true if a server was found and shutdown was attempted.
    pub fn shutdown_server(&mut self, server_name: &str) -> bool {
        let client = self.clients.write().remove(server_name);
        if let Some(client) = client {
            let client_for_task = Arc::clone(&client);
            self.runtime.spawn(async move {
                if let Err(e) = client_for_task.shutdown().await {
                    log::warn!("Error shutting down {}: {}", client_for_task.config.name, e);
                }
            });
            true
        } else {
            false
        }
    }

    /// Shut down all running language servers. Spawns shutdown tasks and
    /// returns immediately; the child processes will be killed via
    /// `kill_on_drop` if shutdown is still pending at registry drop.
    pub fn shutdown_all(&mut self) {
        let clients: Vec<Arc<LspClient>> = self.clients.write().drain().map(|(_, c)| c).collect();
        for client in clients {
            self.runtime.spawn(async move {
                if let Err(e) = client.shutdown().await {
                    log::warn!("Error shutting down {}: {}", client.config.name, e);
                }
            });
        }
    }
}

impl Drop for LspRegistry {
    fn drop(&mut self) {
        // Best-effort shutdown of all servers.
        let count = self.clients.read().len();
        if count > 0 {
            // We can't block in drop if the runtime is being dropped,
            // so just log. The kill_on_drop on each child process handles cleanup.
            log::debug!("LspRegistry dropped with {count} active servers");
        }
    }
}
