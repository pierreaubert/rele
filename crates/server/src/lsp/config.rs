//! Language server configuration: which server to launch for which file type.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for a single language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Human-readable name (e.g. "rust-analyzer").
    pub name: String,
    /// Executable command.
    pub command: String,
    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// File extensions this server handles (without the dot).
    pub file_extensions: Vec<String>,
    /// LSP language identifier (e.g. "rust", "python").
    pub language_id: String,
    /// Files/directories whose presence indicates the project root.
    #[serde(default)]
    pub root_markers: Vec<String>,
}

/// Top-level LSP configuration containing all server definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    pub servers: Vec<LspServerConfig>,
}

impl LspConfig {
    /// Find the first server configuration that handles the given file extension.
    pub fn find_server_for_extension(&self, ext: &str) -> Option<&LspServerConfig> {
        self.servers
            .iter()
            .find(|s| s.file_extensions.iter().any(|e| e == ext))
    }

    /// Load configuration from a JSON file.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        #[allow(clippy::disallowed_methods)]
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Built-in defaults for common language servers.
    pub fn default_config() -> Self {
        Self {
            servers: vec![
                LspServerConfig {
                    name: "rust-analyzer".to_string(),
                    command: "rust-analyzer".to_string(),
                    args: vec![],
                    file_extensions: vec!["rs".to_string()],
                    language_id: "rust".to_string(),
                    root_markers: vec!["Cargo.toml".to_string(), ".git".to_string()],
                },
                LspServerConfig {
                    name: "marksman".to_string(),
                    command: "marksman".to_string(),
                    args: vec!["server".to_string()],
                    file_extensions: vec!["md".to_string(), "markdown".to_string()],
                    language_id: "markdown".to_string(),
                    root_markers: vec![".marksman.toml".to_string(), ".git".to_string()],
                },
                LspServerConfig {
                    name: "pyright".to_string(),
                    command: "pyright-langserver".to_string(),
                    args: vec!["--stdio".to_string()],
                    file_extensions: vec!["py".to_string()],
                    language_id: "python".to_string(),
                    root_markers: vec![
                        "pyproject.toml".to_string(),
                        "setup.py".to_string(),
                        ".git".to_string(),
                    ],
                },
                LspServerConfig {
                    name: "typescript-language-server".to_string(),
                    command: "typescript-language-server".to_string(),
                    args: vec!["--stdio".to_string()],
                    file_extensions: vec![
                        "ts".to_string(),
                        "tsx".to_string(),
                        "js".to_string(),
                        "jsx".to_string(),
                    ],
                    language_id: "typescript".to_string(),
                    root_markers: vec![
                        "tsconfig.json".to_string(),
                        "package.json".to_string(),
                        ".git".to_string(),
                    ],
                },
            ],
        }
    }

    /// Load user config from `~/.config/rele/lsp.json`, falling back to defaults.
    pub fn load_or_default() -> Self {
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("rele").join("lsp.json");
            if path.exists() {
                match Self::load_from_file(&path) {
                    Ok(config) => return config,
                    Err(e) => {
                        log::warn!("Failed to load LSP config from {}: {}", path.display(), e);
                    }
                }
            }
        }
        Self::default_config()
    }
}

/// Walk up from `file_path` looking for a directory containing any of the
/// `root_markers`. Returns the first match, or the file's parent directory.
pub fn find_project_root(file_path: &Path, root_markers: &[String]) -> Option<std::path::PathBuf> {
    let mut dir = if file_path.is_dir() {
        file_path.to_path_buf()
    } else {
        file_path.parent()?.to_path_buf()
    };
    loop {
        for marker in root_markers {
            if dir.join(marker).exists() {
                return Some(dir);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    // Fallback: file's parent directory.
    file_path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_server_by_extension() {
        let config = LspConfig::default_config();
        let server = config.find_server_for_extension("rs").unwrap();
        assert_eq!(server.name, "rust-analyzer");
        assert_eq!(server.language_id, "rust");
    }

    #[test]
    fn find_server_unknown_extension() {
        let config = LspConfig::default_config();
        assert!(config.find_server_for_extension("xyz").is_none());
    }

    #[test]
    fn default_config_roundtrip() {
        let config = LspConfig::default_config();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: LspConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.servers.len(), config.servers.len());
    }
}
