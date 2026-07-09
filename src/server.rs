//! HTTP server for PADAGONIA: REST API, health checks, auth, metrics, and graceful shutdown.

use crate::app_config::Settings;
use crate::bench_support::generate_powerlaw;
use crate::store::Store;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing;

/// Global handle to the Prometheus recorder so the HTTP server can render `/metrics` without
/// needing the handle passed through the CLI dispatch layer.
static METRICS_HANDLE: std::sync::OnceLock<PrometheusHandle> = std::sync::OnceLock::new();

/// Installs a Prometheus metrics recorder and returns a handle for rendering scrape output.
///
/// The handle is also stashed in a global so `serve()` can retrieve it later.
pub fn install_metrics_recorder(
) -> Result<PrometheusHandle, metrics_exporter_prometheus::BuildError> {
    let handle = PrometheusBuilder::new()
        .add_global_label("service", "padagonia")
        .install_recorder()?;
    let _ = METRICS_HANDLE.set(handle.clone());
    Ok(handle)
}

fn get_metrics_handle() -> Option<PrometheusHandle> {
    METRICS_HANDLE.get().cloned()
}

#[derive(Clone)]
struct AppState {
    store: Arc<RwLock<Store>>,
    metrics_handle: PrometheusHandle,
    api_key: String,
}

/// JSON response body for `/api/v1/stats` and `/api/v1/ingest`.
#[derive(Serialize)]
struct StatsResponse {
    nodes: usize,
    edges: usize,
    facts: usize,
    labels: usize,
    relations: usize,
}

/// Request body for `/api/v1/ingest`.
#[derive(Deserialize)]
struct IngestRequest {
    nodes: usize,
    edges: usize,
    seed: u64,
}

/// Start the PADAGONIA HTTP server.
///
/// Binds to `settings.listen_addr()`, loads an existing store from `settings.data_dir()` if a
/// file exists, and runs until SIGTERM (or Ctrl-C on non-Unix platforms).
pub async fn serve(settings: Settings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let metrics_handle = get_metrics_handle()
        .ok_or("metrics recorder not installed; call install_metrics_recorder() first")?;

    let data_path = settings.data_dir();
    let store = load_store(&data_path).await?;
    let state = AppState {
        store: Arc::new(RwLock::new(store)),
        metrics_handle,
        api_key: settings.api_key().to_string(),
    };

    let public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/metrics", get(metrics_handler));

    let api_routes = Router::new()
        .route("/stats", get(stats_handler))
        .route("/ingest", post(ingest_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));

    let app = Router::new()
        .merge(public_routes)
        .nest("/api/v1", api_routes)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(settings.listen_addr()).await?;
    tracing::info!(
        listen_addr = %settings.listen_addr(),
        data_path = %data_path.display(),
        "PADAGONIA HTTP server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("PADAGONIA HTTP server shutting down");
    Ok(())
}

async fn load_store<P: AsRef<Path>>(
    path: P,
) -> Result<Store, Box<dyn std::error::Error + Send + Sync>> {
    let path = path.as_ref();
    if path.is_file() {
        tracing::info!(path = %path.display(), "loading store from disk");
        Ok(Store::load(path)?)
    } else {
        tracing::info!(path = %path.display(), "no store file found, starting empty");
        Ok(Store::new())
    }
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn ready_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ready"}))
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics_handle.render();
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

async fn stats_handler(State(state): State<AppState>) -> Result<Json<StatsResponse>, StatusCode> {
    let store = state.store.read().await;
    let (nodes, edges, facts, labels, relations) = store.stats();
    Ok(Json(StatsResponse {
        nodes,
        edges,
        facts,
        labels,
        relations,
    }))
}

async fn ingest_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<StatsResponse>, StatusCode> {
    let mut store = state.store.write().await;
    generate_powerlaw(&mut store, req.nodes, req.edges, req.seed);
    let (nodes, edges, facts, labels, relations) = store.stats();
    Ok(Json(StatsResponse {
        nodes,
        edges,
        facts,
        labels,
        relations,
    }))
}

/// Bearer-token API-key middleware.
///
/// Reads the `Authorization: Bearer <token>` header and compares it to the configured key.
/// Returns 401 if the header is missing, malformed, or invalid.
async fn require_api_key(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = format!("Bearer {}", state.api_key);
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header == expected => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Wait for SIGTERM (Unix) or Ctrl-C, whichever comes first.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sig = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        sig.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
