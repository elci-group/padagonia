// Copyright (c) 2026 ViCo Agent
// SPDX-License-Identifier: MIT
//! Shared HTTP state and process-wide request-rate governance.

use crate::app_config::LimitsConfig;
use crate::authorization::{CredentialRegistry, Role};
use crate::identity::NamespaceId;
use crate::lifecycle::LifecycleRegistry;
use crate::outbox::PersistentOutbox;
use crate::store::Store;
use crate::transaction::TransactionJournal;
use metrics_exporter_prometheus::PrometheusHandle;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<RwLock<Store>>,
    pub metrics_handle: PrometheusHandle,
    pub credentials: Arc<tokio::sync::RwLock<CredentialRegistry>>,
    pub lifecycle: Arc<RwLock<LifecycleRegistry>>,
    pub data_path: Arc<PathBuf>,
    pub hnsw: (usize, usize, usize),
    pub limits: LimitsConfig,
    pub journal: Arc<Mutex<TransactionJournal>>,
    pub outbox: Arc<Mutex<PersistentOutbox>>,
    /// Serializes journal commit, snapshot replacement and checkpointing.
    /// This is the single-writer boundary for durable state.
    pub commit_gate: Arc<Mutex<()>>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl AppState {
    pub fn new(
        store: Store,
        metrics_handle: PrometheusHandle,
        api_key: impl Into<String>,
        data_path: PathBuf,
        hnsw: (usize, usize, usize),
    ) -> Self {
        Self::new_with_limits(
            store,
            metrics_handle,
            api_key,
            data_path,
            hnsw,
            LimitsConfig::default(),
        )
    }

    pub fn new_with_limits(
        store: Store,
        metrics_handle: PrometheusHandle,
        api_key: impl Into<String>,
        data_path: PathBuf,
        hnsw: (usize, usize, usize),
        limits: LimitsConfig,
    ) -> Self {
        Self::try_new_with_limits(store, metrics_handle, api_key, data_path, hnsw, limits)
            .expect("transaction journal must be openable")
    }

    pub fn try_new_with_limits(
        mut store: Store,
        metrics_handle: PrometheusHandle,
        api_key: impl Into<String>,
        data_path: PathBuf,
        hnsw: (usize, usize, usize),
        limits: LimitsConfig,
    ) -> Result<Self, String> {
        let api_key = api_key.into();
        let rate_limiter = RateLimiter::new(limits.requests_per_second, limits.request_burst);
        let journal_path = data_path.with_extension("journal");
        let journal = TransactionJournal::open(journal_path).map_err(|error| error.to_string())?;
        let outbox_path = data_path.with_extension("outbox");
        let outbox = PersistentOutbox::open(outbox_path).map_err(|error| error.to_string())?;
        let lifecycle_path = data_path.with_extension("lifecycle");
        let lifecycle = if lifecycle_path.is_file() {
            let bytes = fs::read(&lifecycle_path).map_err(|error| error.to_string())?;
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?
        } else {
            LifecycleRegistry::default()
        };
        // Replay only mutations absent from the snapshot and not tombstoned.
        journal
            .replay_missing_with_lifecycle(&mut store, &lifecycle)
            .map_err(|error| error.to_string())?;
        let credentials_path = data_path.with_extension("credentials");
        let mut credentials = if credentials_path.is_file() {
            CredentialRegistry::load_from_path(&credentials_path)?
        } else {
            let mut registry = CredentialRegistry::default();
            registry.insert_token(&api_key, NamespaceId::default(), Role::Administrator);
            registry.save_to_path(&credentials_path)?;
            registry
        };
        // Ensure bootstrap admin is present even if credentials file predates it (forward compat).
        let bootstrap_hash = {
            use sha2::{Digest, Sha256};
            let h: [u8; 32] = Sha256::digest(api_key.as_bytes()).into();
            h
        };
        if !credentials.credentials.contains_key(&bootstrap_hash) {
            credentials.insert_token(&api_key, NamespaceId::default(), Role::Administrator);
            credentials.save_to_path(&credentials_path)?;
        }
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            metrics_handle,
            credentials: Arc::new(RwLock::new(credentials)),
            lifecycle: Arc::new(RwLock::new(lifecycle)),
            data_path: Arc::new(data_path),
            hnsw,
            limits,
            journal: Arc::new(Mutex::new(journal)),
            outbox: Arc::new(Mutex::new(outbox)),
            commit_gate: Arc::new(Mutex::new(())),
            rate_limiter: Arc::new(Mutex::new(rate_limiter)),
        })
    }

    pub(crate) async fn allow_request(&self) -> bool {
        self.rate_limiter.lock().await.allow()
    }

    /// Durably insert a credential and persist the registry atomically.
    pub async fn insert_credential_token(
        &self,
        token: &str,
        namespace: NamespaceId,
        role: Role,
    ) -> Result<(), String> {
        let _guard = self.commit_gate.lock().await;
        let mut registry = self.credentials.write().await;
        registry.insert_token(token, namespace, role);
        let path = self.data_path.with_extension("credentials");
        registry.save_to_path(&path)?;
        Ok(())
    }

    /// Durably revoke a credential and persist the registry atomically.
    pub async fn revoke_credential(&self, token: &str) -> Result<bool, String> {
        let _guard = self.commit_gate.lock().await;
        let mut registry = self.credentials.write().await;
        let revoked = registry.revoke(token);
        if revoked {
            let path = self.data_path.with_extension("credentials");
            registry.save_to_path(&path)?;
        }
        Ok(revoked)
    }

    /// Expose the durable credential path for backup tooling.
    pub fn credentials_path(&self) -> PathBuf {
        self.data_path.with_extension("credentials")
    }

    pub fn lifecycle_path(&self) -> PathBuf {
        self.data_path.with_extension("lifecycle")
    }

    pub fn journal_path(&self) -> PathBuf {
        self.data_path.with_extension("journal")
    }
}

#[derive(Debug)]
struct RateLimiter {
    tokens: f64,
    last_refill: Instant,
    requests_per_second: f64,
    burst: f64,
}

impl RateLimiter {
    fn new(requests_per_second: u32, burst: u32) -> Self {
        let burst = f64::from(burst.max(1));
        Self {
            tokens: burst,
            last_refill: Instant::now(),
            requests_per_second: f64::from(requests_per_second),
            burst,
        }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.requests_per_second).min(self.burst);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}
