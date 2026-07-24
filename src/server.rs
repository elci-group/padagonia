//! HTTP server for PADAGONIA: REST API, health checks, auth, metrics,
//! write-through persistence, and graceful shutdown.

use crate::app_config::Settings;
use crate::auth::auth_middleware;
use crate::bench_support::generate_powerlaw;
use crate::hnsw::{Distance, HnswParams};
use crate::http_error::{bad_request, internal_error, not_found, ApiResult};
use crate::id::NodeId;
use crate::metrics::get_metrics_handle;
use crate::ontology::StringTableExt;
use crate::projection::props_to_json;
use crate::provenance::Provenance;
use crate::query::QueryEngine;
use crate::store::Store;
use crate::value::Scalar;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

/// Shared state behind the HTTP API.
#[derive(Clone)]
pub struct AppState {
    store: Arc<RwLock<Store>>,
    metrics_handle: PrometheusHandle,
    api_key: Arc<str>,
    data_path: Arc<PathBuf>,
    /// HNSW parameters `(m, ef_construction, ef)` from configuration.
    hnsw: (usize, usize, usize),
}

impl AppState {
    pub fn new(
        store: Store,
        metrics_handle: PrometheusHandle,
        api_key: impl Into<String>,
        data_path: PathBuf,
        hnsw: (usize, usize, usize),
    ) -> Self {
        Self {
            store: Arc::new(RwLock::new(store)),
            metrics_handle,
            api_key: Arc::from(api_key.into().into_boxed_str()),
            data_path: Arc::new(data_path),
            hnsw,
        }
    }
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

/// JSON response body for the node/edge creation endpoints.
#[derive(Serialize)]
struct IdResponse {
    id: u64,
}

/// Request body for `/api/v1/ingest` (synthetic workload generator).
#[derive(Deserialize)]
struct IngestRequest {
    nodes: usize,
    edges: usize,
    seed: u64,
}

/// Provenance supplied by the caller; server-side defaults fill the gaps.
#[derive(Deserialize)]
struct ProvenanceInput {
    agent: String,
    model: String,
    confidence: Option<f32>,
    cost: Option<f32>,
    evidence: Option<Vec<String>>,
}

impl ProvenanceInput {
    fn into_provenance(self) -> Provenance {
        Provenance {
            agent: self.agent,
            model: self.model,
            confidence: self.confidence.unwrap_or(1.0),
            cost: self.cost.unwrap_or(0.0),
            timestamp: now_unix(),
            evidence: self.evidence.unwrap_or_default(),
        }
    }
}

fn default_provenance() -> Provenance {
    Provenance {
        agent: "http-api".to_string(),
        model: "unknown".to_string(),
        confidence: 1.0,
        cost: 0.0,
        timestamp: now_unix(),
        evidence: Vec::new(),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convert a JSON property value to a `Scalar`. Arrays and objects have no
/// scalar counterpart, so they are stored as their JSON text representation.
fn json_to_scalar(value: serde_json::Value) -> Scalar {
    match value {
        serde_json::Value::Null => Scalar::Null,
        serde_json::Value::Bool(b) => Scalar::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Scalar::I64(i)
            } else {
                Scalar::F64(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Scalar::String(s),
        other => Scalar::String(other.to_string()),
    }
}

fn json_props(properties: &HashMap<String, serde_json::Value>) -> Vec<(&str, Scalar)> {
    properties
        .iter()
        .map(|(k, v)| (k.as_str(), json_to_scalar(v.clone())))
        .collect()
}

/// Request body for `POST /api/v1/nodes`.
#[derive(Deserialize)]
struct CreateNodeRequest {
    label: String,
    #[serde(default)]
    properties: HashMap<String, serde_json::Value>,
    embedding: Option<Vec<f32>>,
    provenance: Option<ProvenanceInput>,
}

/// Request body for `POST /api/v1/edges`.
#[derive(Deserialize)]
struct CreateEdgeRequest {
    src: u64,
    dst: u64,
    label: String,
    #[serde(default)]
    properties: HashMap<String, serde_json::Value>,
    embedding: Option<Vec<f32>>,
    provenance: Option<ProvenanceInput>,
}

/// Request body for `POST /api/v1/bfs`.
#[derive(Deserialize)]
struct BfsRequest {
    start: u64,
    depth: usize,
    relation: Option<String>,
    min_confidence: Option<f32>,
}

/// One entry of the `/api/v1/bfs` response.
#[derive(Serialize)]
struct BfsEntry {
    node_id: u64,
    depth: usize,
}

/// Request body for `POST /api/v1/vector-search`.
#[derive(Deserialize)]
struct VectorSearchRequest {
    query: Vec<f32>,
    k: Option<usize>,
    ef: Option<usize>,
    label: Option<String>,
    metric: Option<String>,
}

/// One entry of the `/api/v1/vector-search` response.
#[derive(Serialize)]
struct VectorHit {
    node_id: u64,
    distance: f32,
}

/// JSON response body for `GET /api/v1/nodes/:id`.
#[derive(Serialize)]
struct NodeResponse {
    id: u64,
    label: String,
    properties: serde_json::Value,
    embedding: Option<Vec<f32>>,
    provenance: Provenance,
}

/// Build the HTTP router. Kept separate from `serve()` so integration tests
/// can exercise the API without binding a socket.
pub fn router(state: AppState, metrics_path: &str) -> Router {
    let public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route(metrics_path, get(metrics_handler));

    let api_routes = Router::new()
        .route("/stats", get(stats_handler))
        .route("/ingest", post(ingest_handler))
        .route("/nodes", post(create_node_handler))
        .route("/nodes/:id", get(get_node_handler))
        .route("/edges", post(create_edge_handler))
        .route("/bfs", post(bfs_handler))
        .route("/vector-search", post(vector_search_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            state.api_key.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .nest("/api/v1", api_routes)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Start the PADAGONIA HTTP server.
///
/// Refuses to start without a configured API key, binds to
/// `settings.listen_addr()`, loads an existing store from `settings.data_dir()`
/// if one exists, and saves the store on graceful shutdown (SIGTERM/Ctrl-C).
pub async fn serve(settings: Settings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if settings.api_key().is_empty() {
        return Err(
            "server.api_key must not be empty: protected routes would accept a blank bearer token"
                .into(),
        );
    }
    let metrics_handle = get_metrics_handle()
        .ok_or("metrics recorder not installed; call install_metrics_recorder() first")?;

    let data_path = settings.data_dir();
    let store = load_store(&data_path).await?;
    let state = AppState::new(
        store,
        metrics_handle,
        settings.api_key().to_string(),
        data_path.clone(),
        settings.hnsw_params(),
    );
    let app = router(state.clone(), settings.metrics_path());

    let listener = tokio::net::TcpListener::bind(settings.listen_addr()).await?;
    tracing::info!(
        listen_addr = %settings.listen_addr(),
        data_path = %data_path.display(),
        "PADAGONIA HTTP server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Best-effort final save so writes made while the server ran survive restarts.
    let snapshot = state.store.read().await.clone();
    let path = data_path;
    match tokio::task::spawn_blocking(move || save_store_to(&snapshot, &path)).await {
        Ok(Ok(())) => tracing::info!("store saved on shutdown"),
        Ok(Err(e)) => tracing::warn!("failed to save store on shutdown: {e}"),
        Err(e) => tracing::warn!("shutdown save task failed: {e}"),
    }

    tracing::info!("PADAGONIA HTTP server shutting down");
    Ok(())
}

async fn load_store<P: AsRef<FsPath>>(
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

/// Persist a store to disk, creating the parent directory when needed.
fn save_store_to(store: &Store, path: &FsPath) -> crate::storage::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    store.save(path)
}

/// Write-through persistence after a mutation: snapshot under a read lock,
/// serialize off the async runtime so large stores do not block it.
async fn persist(state: &AppState) -> ApiResult<()> {
    let snapshot = state.store.read().await.clone();
    let path = (*state.data_path).clone();
    match tokio::task::spawn_blocking(move || save_store_to(&snapshot, &path)).await {
        Ok(Ok(())) => {
            metrics::counter!("padagonia_persist_total").increment(1);
            Ok(())
        }
        Ok(Err(e)) => Err(internal_error(format!("failed to persist store: {e}"))),
        Err(e) => Err(internal_error(format!("persist task failed: {e}"))),
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

async fn stats_handler(State(state): State<AppState>) -> Json<StatsResponse> {
    let store = state.store.read().await;
    let (nodes, edges, facts, labels, relations) = store.stats();
    Json(StatsResponse {
        nodes,
        edges,
        facts,
        labels,
        relations,
    })
}

async fn ingest_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> ApiResult<Json<StatsResponse>> {
    let (nodes, edges, facts, labels, relations) = {
        let mut store = state.store.write().await;
        generate_powerlaw(&mut store, req.nodes, req.edges, req.seed);
        store.stats()
    };
    persist(&state).await?;
    Ok(Json(StatsResponse {
        nodes,
        edges,
        facts,
        labels,
        relations,
    }))
}

async fn create_node_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateNodeRequest>,
) -> ApiResult<(StatusCode, Json<IdResponse>)> {
    let provenance = req
        .provenance
        .map(ProvenanceInput::into_provenance)
        .unwrap_or_else(default_provenance);
    let props = json_props(&req.properties);
    let id = {
        let mut store = state.store.write().await;
        store.add_node(&req.label, props, req.embedding, provenance)
    };
    metrics::counter!("padagonia_http_nodes_created_total").increment(1);
    persist(&state).await?;
    Ok((StatusCode::CREATED, Json(IdResponse { id: id.0 })))
}

async fn get_node_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> ApiResult<Json<NodeResponse>> {
    let store = state.store.read().await;
    let node = store
        .nodes()
        .get(&NodeId(id))
        .ok_or_else(|| not_found(format!("node {id} not found")))?;
    Ok(Json(NodeResponse {
        id: node.id.0,
        label: store
            .string_table()
            .resolve_label(node.label)
            .unwrap_or("?")
            .to_string(),
        properties: props_to_json(&node.properties, &store),
        embedding: node.embedding.clone(),
        provenance: node.provenance.clone(),
    }))
}

async fn create_edge_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateEdgeRequest>,
) -> ApiResult<(StatusCode, Json<IdResponse>)> {
    {
        let store = state.store.read().await;
        if !store.nodes().contains_key(&NodeId(req.src)) {
            return Err(bad_request(format!("src node {} not found", req.src)));
        }
        if !store.nodes().contains_key(&NodeId(req.dst)) {
            return Err(bad_request(format!("dst node {} not found", req.dst)));
        }
    }
    let provenance = req
        .provenance
        .map(ProvenanceInput::into_provenance)
        .unwrap_or_else(default_provenance);
    let props = json_props(&req.properties);
    let id = {
        let mut store = state.store.write().await;
        store.add_edge(
            NodeId(req.src),
            NodeId(req.dst),
            &req.label,
            props,
            req.embedding,
            provenance,
        )
    };
    metrics::counter!("padagonia_http_edges_created_total").increment(1);
    persist(&state).await?;
    Ok((StatusCode::CREATED, Json(IdResponse { id: id.0 })))
}

async fn bfs_handler(
    State(state): State<AppState>,
    Json(req): Json<BfsRequest>,
) -> ApiResult<Json<Vec<BfsEntry>>> {
    let store = state.store.read().await;
    let relation_id = match &req.relation {
        Some(relation) => Some(
            store
                .string_table()
                .relation_id(relation)
                .ok_or_else(|| bad_request(format!("unknown relation '{relation}'")))?,
        ),
        None => None,
    };
    let engine = QueryEngine::new(&store);
    let reached = engine.bfs(
        NodeId(req.start),
        req.depth,
        relation_id,
        req.min_confidence,
    );
    Ok(Json(
        reached
            .into_iter()
            .map(|(node, depth)| BfsEntry {
                node_id: node.0,
                depth,
            })
            .collect(),
    ))
}

async fn vector_search_handler(
    State(state): State<AppState>,
    Json(req): Json<VectorSearchRequest>,
) -> ApiResult<Json<Vec<VectorHit>>> {
    if req.query.is_empty() {
        return Err(bad_request("query embedding must not be empty"));
    }
    let distance = match req.metric.as_deref() {
        None | Some("euclidean") => Distance::Euclidean,
        Some("cosine") => Distance::Cosine,
        Some(other) => {
            return Err(bad_request(format!(
                "unknown metric '{other}' (expected euclidean|cosine)"
            )))
        }
    };
    let store = state.store.read().await;
    let label_id = match &req.label {
        Some(label) => Some(
            store
                .string_table()
                .label_id(label)
                .ok_or_else(|| bad_request(format!("unknown label '{label}'")))?,
        ),
        None => None,
    };
    let (m, ef_construction, ef_default) = state.hnsw;
    let k = req.k.unwrap_or(10);
    let params = HnswParams {
        m,
        ef_construction,
        ef_search: req.ef.unwrap_or(ef_default),
    };
    let engine = QueryEngine::new(&store);
    let hits = engine.vector_search_with_params(distance, params, &req.query, k, label_id);
    Ok(Json(
        hits.into_iter()
            .map(|(node, distance)| VectorHit {
                node_id: node.0,
                distance,
            })
            .collect(),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serve_rejects_empty_api_key_before_binding() {
        let result = serve(Settings::default()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("api_key"));
    }
}
