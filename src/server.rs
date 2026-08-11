//! HTTP server for PADAGONIA: REST API, health checks, auth, metrics,
//! write-through persistence, and graceful shutdown.

use crate::api::*;
use crate::app_config::Settings;
use crate::auth::auth_middleware;
use crate::authorization::{AuthenticatedPrincipal, Operation};
use crate::bench_support::generate_powerlaw;
use crate::contract::{BatchMutationRequest, BatchMutationResponse, API_VERSION};
use crate::hnsw::{Distance, HnswParams};
use crate::http_error::{bad_request, forbidden, internal_error, not_found, ApiResult};
use crate::id::NodeId;
use crate::metrics::get_metrics_handle;
use crate::ontology::StringTableExt;
use crate::projection::props_to_json;
use crate::query::QueryEngine;
use crate::server_middleware::{normalize_error_responses, rate_limit_middleware, request_context};
use crate::store::Store;
use axum::{
    extract::{DefaultBodyLimit, Extension, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::path::Path as FsPath;
use std::time::Duration;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

pub use crate::server_state::AppState;

/// Build the HTTP router. Kept separate from `serve()` so integration tests
/// can exercise the API without binding a socket.
pub fn router(state: AppState, metrics_path: &str) -> Router {
    let limits = state.limits.clone();
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
        .route("/transactions", post(transaction_handler))
        .route("/query/nodes", post(query_nodes_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .nest("/api/v1", api_routes)
        .route("/openapi.json", get(openapi_handler))
        .layer(DefaultBodyLimit::max(limits.request_body_bytes))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(limits.request_timeout_seconds),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(normalize_error_responses))
        .layer(axum::middleware::from_fn(request_context))
        .with_state(state)
}

/// Start the PADAGONIA HTTP server.
///
/// Refuses to start without a configured API key, binds to
/// `settings.listen_addr()`, loads an existing store from `settings.data_dir()`
/// if one exists, and saves the store on graceful shutdown (SIGTERM/Ctrl-C).
pub async fn serve(settings: Settings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    settings
        .validate()
        .map_err(|error| format!("invalid configuration: {error}"))?;
    let metrics_handle = get_metrics_handle()
        .ok_or("metrics recorder not installed; call install_metrics_recorder() first")?;

    let data_path = settings.data_dir();
    let store = load_store(&data_path).await?;
    let state = AppState::try_new_with_limits(
        store,
        metrics_handle,
        settings.api_key().to_string(),
        data_path.clone(),
        settings.hnsw_params(),
        settings.limits().clone(),
    )
    .map_err(|error| format!("failed to initialize durable server state: {error}"))?;
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
    let path = data_path.clone();
    match tokio::task::spawn_blocking(move || save_store_to(&snapshot, &path)).await {
        Ok(Ok(())) => tracing::info!(
            event = "shutdown_store_saved",
            data_path = %data_path.display(),
            "store saved on shutdown"
        ),
        Ok(Err(error)) => tracing::warn!(
            event = "shutdown_store_save_failed",
            data_path = %data_path.display(),
            error = %error,
            "failed to save store on shutdown"
        ),
        Err(error) => tracing::warn!(
            event = "shutdown_store_task_failed",
            data_path = %data_path.display(),
            error = %error,
            "shutdown save task failed"
        ),
    }

    tracing::info!(
        event = "server_shutdown_complete",
        data_path = %data_path.display(),
        "PADAGONIA HTTP server shutting down"
    );
    Ok(())
}

async fn openapi_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("../docs/openapi.json"),
    )
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
        Ok(Err(error)) => {
            tracing::error!(event = "persist_failed", error = %error, "store persistence failed");
            Err(internal_error(format!("failed to persist store: {error}")))
        }
        Err(error) => {
            tracing::error!(event = "persist_task_failed", error = %error, "store persistence task failed");
            Err(internal_error(format!("persist task failed: {error}")))
        }
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

async fn query_nodes_handler(
    State(state): State<AppState>,
    Json(request): Json<crate::contract::NodeQueryRequest>,
) -> Json<crate::contract::PageCursorResponse> {
    let store = state.store.read().await;
    let page = QueryEngine::new(&store).nodes_page(&request.clone().into_query());
    Json(crate::contract::PageCursorResponse {
        api_version: API_VERSION.to_string(),
        node_ids: page.entries.into_iter().map(|node| node.id.0).collect(),
        next_cursor: page.next_cursor,
    })
}

async fn transaction_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    headers: axum::http::HeaderMap,
    Json(request): Json<BatchMutationRequest>,
) -> ApiResult<Json<BatchMutationResponse>> {
    let _commit_guard = state.commit_gate.lock().await;
    let namespace_header = headers
        .get("x-padagonia-namespace")
        .ok_or_else(|| bad_request("x-padagonia-namespace header is required"))?
        .to_str()
        .map_err(|_| bad_request("x-padagonia-namespace header is invalid"))?;
    if namespace_header != request.namespace.as_str() {
        return Err(bad_request(
            "namespace header does not match transaction namespace",
        ));
    }
    let namespace = request.namespace.clone();
    if principal.namespace != namespace || !principal.role.permits(Operation::Write) {
        return Err(forbidden("credential is not authorized for this namespace or operation"));
    }
    let transaction = request.into_transaction();
    let result = {
        let mut store = state.store.write().await;
        let mut journal = state.journal.lock().await;
        let replay = journal
            .committed_result(&transaction.idempotency_key)
            .is_some();
        journal
            .commit(&mut store, transaction)
            .map(|result| (result, replay))
            .map_err(|error| internal_error(format!("transaction commit failed: {error}")))?
    };
    if !result.1 {
        state
            .outbox
            .lock()
            .await
            .append(
                namespace,
                "transaction.committed",
                rmp_serde::to_vec(&result.0)
                    .map_err(|error| internal_error(format!("outbox encode failed: {error}")))?,
            )
            .map_err(|error| internal_error(format!("outbox append failed: {error}")))?;
    }
    persist(&state).await?;
    Ok(Json(BatchMutationResponse {
        api_version: API_VERSION.to_string(),
        result: result.0,
    }))
}

async fn ingest_handler(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> ApiResult<Json<StatsResponse>> {
    let _commit_guard = state.commit_gate.lock().await;
    if req.nodes > state.limits.max_ingest_nodes {
        return Err(bad_request(format!(
            "nodes exceeds configured limit {}",
            state.limits.max_ingest_nodes
        )));
    }
    if req.edges > state.limits.max_ingest_edges {
        return Err(bad_request(format!(
            "edges exceeds configured limit {}",
            state.limits.max_ingest_edges
        )));
    }
    if req.nodes == 0 && req.edges != 0 {
        return Err(bad_request("edges require at least one node"));
    }
    let (nodes, edges, facts, labels, relations) = {
        let mut store = state.store.write().await;
        generate_powerlaw(&mut store, req.nodes, req.edges, req.seed);
        store.stats()
    };
    persist(&state).await?;
    tracing::info!(
        event = "graph_ingested",
        nodes = req.nodes,
        edges = req.edges,
        seed = req.seed,
        "synthetic graph persisted"
    );
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
    let _commit_guard = state.commit_gate.lock().await;
    validate_label(&req.label)?;
    validate_properties(&req.properties)?;
    validate_provenance(req.provenance.as_ref())?;
    validate_embedding(req.embedding.as_deref(), &state.limits)?;
    let provenance = req
        .provenance
        .map(ProvenanceInput::into_provenance)
        .unwrap_or_else(default_provenance);
    let external_id = req.external_id.unwrap_or_else(|| {
        format!(
            "http-node-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        )
    });
    let idempotency_key = req
        .idempotency_key
        .unwrap_or_else(|| format!("create-node:{}:{}", req.namespace, external_id));
    let props = json_props(&req.properties)
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    let id = {
        let store = state.store.read().await;
        if let (Some(expected), Some(actual)) = (
            expected_embedding_dimension(&store),
            req.embedding.as_ref().map(Vec::len),
        ) {
            if expected != actual {
                return Err(bad_request(format!(
                    "embedding dimension {actual} does not match graph dimension {expected}"
                )));
            }
        }
        drop(store);
        let transaction = crate::transaction::Transaction {
            idempotency_key,
            mutations: vec![crate::transaction::Mutation::AddNode {
                namespace: req.namespace,
                external_id,
                label: req.label,
                properties: props,
                embedding: req.embedding,
                provenance,
            }],
        };
        let mut store = state.store.write().await;
        let mut journal = state.journal.lock().await;
        journal
            .commit(&mut store, transaction)
            .map_err(|error| internal_error(format!("node transaction failed: {error}")))?
            .node_ids
            .into_iter()
            .next()
            .ok_or_else(|| internal_error("node transaction returned no node id"))?
    };
    metrics::counter!("padagonia_http_nodes_created_total").increment(1);
    persist(&state).await?;
    tracing::info!(event = "node_created", node_id = id.0, "node persisted");
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
    let _commit_guard = state.commit_gate.lock().await;
    validate_label(&req.label)?;
    validate_properties(&req.properties)?;
    validate_provenance(req.provenance.as_ref())?;
    validate_embedding(req.embedding.as_deref(), &state.limits)?;
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
    let external_id = req.external_id.unwrap_or_else(|| {
        format!(
            "http-edge-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        )
    });
    let idempotency_key = req
        .idempotency_key
        .unwrap_or_else(|| format!("create-edge:{}:{}", req.namespace, external_id));
    let props = json_props(&req.properties)
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    let id = {
        let mut store = state.store.write().await;
        let mut journal = state.journal.lock().await;
        journal
            .commit(
                &mut store,
                crate::transaction::Transaction {
                    idempotency_key,
                    mutations: vec![crate::transaction::Mutation::AddEdge {
                        namespace: req.namespace,
                        external_id,
                        src: NodeId(req.src),
                        dst: NodeId(req.dst),
                        label: req.label,
                        properties: props,
                        embedding: req.embedding,
                        provenance,
                    }],
                },
            )
            .map_err(|error| bad_request(format!("edge transaction failed: {error}")))?
            .edge_ids
            .into_iter()
            .next()
            .ok_or_else(|| internal_error("edge transaction returned no edge id"))?
    };
    metrics::counter!("padagonia_http_edges_created_total").increment(1);
    persist(&state).await?;
    tracing::info!(
        event = "edge_created",
        edge_id = id.0,
        src = req.src,
        dst = req.dst,
        "edge persisted"
    );
    Ok((StatusCode::CREATED, Json(IdResponse { id: id.0 })))
}

async fn bfs_handler(
    State(state): State<AppState>,
    Json(req): Json<BfsRequest>,
) -> ApiResult<Json<Vec<BfsEntry>>> {
    if req.depth > state.limits.max_bfs_depth {
        return Err(bad_request(format!(
            "depth exceeds configured limit {}",
            state.limits.max_bfs_depth
        )));
    }
    if req
        .min_confidence
        .is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
    {
        return Err(bad_request(
            "min_confidence must be finite and between 0 and 1",
        ));
    }
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
    if req.query.len() > state.limits.max_vector_dimensions {
        return Err(bad_request(format!(
            "query dimension exceeds configured limit {}",
            state.limits.max_vector_dimensions
        )));
    }
    if req.query.iter().any(|value| !value.is_finite()) {
        return Err(bad_request("query embedding values must be finite"));
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
    if let Some(expected) = expected_embedding_dimension(&store) {
        if req.query.len() != expected {
            return Err(bad_request(format!(
                "query dimension {} does not match graph dimension {expected}",
                req.query.len()
            )));
        }
    }
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
    if k == 0 || k > state.limits.max_vector_results {
        return Err(bad_request(format!(
            "k must be between 1 and {}",
            state.limits.max_vector_results
        )));
    }
    let ef_search = req.ef.unwrap_or(ef_default);
    if ef_search == 0 || ef_search > state.limits.max_vector_ef {
        return Err(bad_request(format!(
            "ef must be between 1 and {}",
            state.limits.max_vector_ef
        )));
    }
    let params = HnswParams {
        m,
        ef_construction,
        ef_search,
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
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(event = "ctrl_c_handler_failed", error = %error, "Ctrl-C handler failed");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(event = "sigterm_handler_failed", error = %error, "SIGTERM handler failed");
                std::future::pending::<()>().await;
            }
        }
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
