//! Shared HTTP state and process-wide request-rate governance.

use crate::app_config::LimitsConfig;
use crate::store::Store;
use metrics_exporter_prometheus::PrometheusHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub(crate) store: Arc<RwLock<Store>>,
    pub(crate) metrics_handle: PrometheusHandle,
    pub(crate) api_key: Arc<str>,
    pub(crate) data_path: Arc<PathBuf>,
    pub(crate) hnsw: (usize, usize, usize),
    pub(crate) limits: LimitsConfig,
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
        let rate_limiter = RateLimiter::new(limits.requests_per_second, limits.request_burst);
        Self {
            store: Arc::new(RwLock::new(store)),
            metrics_handle,
            api_key: Arc::from(api_key.into().into_boxed_str()),
            data_path: Arc::new(data_path),
            hnsw,
            limits,
            rate_limiter: Arc::new(Mutex::new(rate_limiter)),
        }
    }

    pub(crate) async fn allow_request(&self) -> bool {
        self.rate_limiter.lock().await.allow()
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
