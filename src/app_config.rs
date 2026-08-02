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
    pub limits: LimitsConfig,
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
            .add_source(
                Environment::with_prefix("PADAGONIA")
                    .prefix_separator("__")
                    .separator("__"),
            )
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

    /// Returns the path of the store file the HTTP server loads and persists.
    ///
    /// If `server.data_dir` is set it is used verbatim as the store file path;
    /// otherwise the store lives at `storage.default_graph` inside
    /// `storage.data_dir`.
    pub fn data_dir(&self) -> PathBuf {
        if self.server.data_dir.is_empty() {
            PathBuf::from(&self.storage.data_dir).join(&self.storage.default_graph)
        } else {
            PathBuf::from(&self.server.data_dir)
        }
    }

    /// Returns the configured API key for protected endpoints.
    pub fn api_key(&self) -> &str {
        &self.server.api_key
    }

    /// Returns the metrics scrape path, defaulting to `/metrics`.
    pub fn metrics_path(&self) -> &str {
        if self.server.metrics_path.is_empty() {
            "/metrics"
        } else {
            &self.server.metrics_path
        }
    }

    /// Returns the default tracing/log level.
    pub fn log_level(&self) -> &str {
        &self.logging.level
    }

    /// Returns HNSW index parameters as `(m, ef_construction, ef)`.
    pub fn hnsw_params(&self) -> (usize, usize, usize) {
        (self.hnsw.m, self.hnsw.ef_construction, self.hnsw.ef)
    }

    /// Returns the operational limits applied by the HTTP server.
    pub fn limits(&self) -> &LimitsConfig {
        &self.limits
    }

    /// Validate security- and resource-sensitive settings before binding a socket.
    pub fn validate(&self) -> Result<(), String> {
        let api_key = self.api_key();
        if api_key.len() < 16 || api_key.trim() != api_key {
            return Err(
                "server.api_key must contain at least 16 bytes with no surrounding whitespace"
                    .to_string(),
            );
        }
        let limits = &self.limits;
        if limits.request_body_bytes == 0
            || limits.request_timeout_seconds == 0
            || limits.requests_per_second == 0
            || limits.request_burst == 0
            || limits.max_ingest_nodes == 0
            || limits.max_ingest_edges == 0
            || limits.max_bfs_depth == 0
            || limits.max_vector_dimensions == 0
            || limits.max_vector_results == 0
            || limits.max_vector_ef == 0
        {
            return Err("all limits must be greater than zero".to_string());
        }
        if self.hnsw.m == 0 || self.hnsw.ef_construction == 0 || self.hnsw.ef == 0 {
            return Err("all HNSW parameters must be greater than zero".to_string());
        }
        Ok(())
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

/// Resource-governance settings for untrusted HTTP requests.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub request_body_bytes: usize,
    pub request_timeout_seconds: u64,
    pub requests_per_second: u32,
    pub request_burst: u32,
    pub max_ingest_nodes: usize,
    pub max_ingest_edges: usize,
    pub max_bfs_depth: usize,
    pub max_vector_dimensions: usize,
    pub max_vector_results: usize,
    pub max_vector_ef: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            request_body_bytes: 1_048_576,
            request_timeout_seconds: 30,
            requests_per_second: 100,
            request_burst: 200,
            max_ingest_nodes: 100_000,
            max_ingest_edges: 500_000,
            max_bfs_depth: 64,
            max_vector_dimensions: 4_096,
            max_vector_results: 1_000,
            max_vector_ef: 10_000,
        }
    }
}

/// Initialize the `tracing` subscriber using the configured log level, falling back to `RUST_LOG`.
pub fn init_tracing(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_defaults_require_an_explicit_api_key() {
        let settings = Settings::default();
        assert!(settings.validate().unwrap_err().contains("api_key"));
    }

    #[test]
    fn zero_resource_limit_is_rejected() {
        let mut settings = Settings::default();
        settings.server.api_key = "a-secure-test-key".to_string();
        settings.limits.max_bfs_depth = 0;
        assert!(settings.validate().unwrap_err().contains("limits"));
    }
}
