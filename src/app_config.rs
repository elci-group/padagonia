//! Configuration loaded from `padagonia.toml`, environment variables, and defaults.

use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

/// Top-level application settings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
    pub hnsw: HnswConfig,
}

impl Settings {
    /// Load settings from `padagonia.toml` (optional) and `PADAGONIA_*` environment variables.
    ///
    /// Nested fields can be overridden via double-underscore separators, e.g.:
    /// `PADAGONIA_SERVER__PORT=8080`.
    pub fn load() -> Result<Self, config::ConfigError> {
        Self::load_from("padagonia.toml")
    }

    /// Load settings from a specific config file.
    pub fn load_from<P: AsRef<std::path::Path>>(path: P) -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(
                File::from(path.as_ref())
                    .format(config::FileFormat::Toml)
                    .required(false),
            )
            .add_source(Environment::with_prefix("PADAGONIA").separator("__"))
            .build()?
            .try_deserialize()
    }

    /// Returns the socket address the HTTP server should bind to.
    ///
    /// If `server.listen_addr` is set, it is used verbatim; otherwise the address is built from
    /// `server.host` and `server.port`.
    pub fn listen_addr(&self) -> String {
        if self.server.listen_addr.is_empty() {
            format!("{}:{}", self.server.host, self.server.port)
        } else {
            self.server.listen_addr.clone()
        }
    }

    /// Returns the configured data directory / store file path.
    ///
    /// If `server.data_dir` is set it takes precedence, otherwise `storage.data_dir` is used.
    pub fn data_dir(&self) -> PathBuf {
        if self.server.data_dir.is_empty() {
            PathBuf::from(&self.storage.data_dir)
        } else {
            PathBuf::from(&self.server.data_dir)
        }
    }

    /// Returns the configured API key for protected endpoints.
    pub fn api_key(&self) -> &str {
        &self.server.api_key
    }

    /// Returns the path to the default graph file.
    pub fn default_graph_path(&self) -> PathBuf {
        PathBuf::from(&self.storage.default_graph)
    }

    /// Returns the default tracing/log level.
    pub fn log_level(&self) -> &str {
        &self.logging.level
    }

    /// Returns HNSW index parameters as `(m, ef_construction, ef)`.
    pub fn hnsw_params(&self) -> (usize, usize, usize) {
        (self.hnsw.m, self.hnsw.ef_construction, self.hnsw.ef)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub metrics_path: String,
    pub listen_addr: String,
    pub api_key: String,
    pub data_dir: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 7373,
            metrics_path: "/metrics".to_string(),
            listen_addr: String::new(),
            api_key: String::new(),
            data_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub data_dir: String,
    pub default_graph: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "data".to_string(),
            default_graph: "graph.pad".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HnswConfig {
    pub m: usize,
    pub ef_construction: usize,
    pub ef: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef: 50,
        }
    }
}

/// Initialize the `tracing` subscriber using the configured log level, falling back to `RUST_LOG`.
pub fn init_tracing(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
