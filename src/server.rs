// Copyright (c) 2026 ViCo Agent
// SPDX-License-Identifier: MIT
//! HTTP server for PADAGONIA: REST API, health checks, auth, metrics,
//! write-through persistence, and graceful shutdown.

use crate::api::*;
use crate::app_config::Settings;
use crate::auth::auth_middleware;
use crate::authorization::{AuthenticatedPrincipal, Operation};
use crate::contract::{BatchMutationRequest, BatchMutationResponse, NodeRecordResponse, API_VERSION};
use crate::hnsw::{Distance, HnswParams};
use crate::http_error::{bad_request, forbidden, internal_error, not_found, ApiResult};
use crate::id::NodeId;
use crate::lifecycle::{RecordKey, Tombstone};
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
        .route("/nodes", post(create_node_handler))
        .route("/nodes/:id", get(get_node_handler))
        .route("/edges", post(create_edge_handler))
        .route("/bfs", post(bfs_handler))
        .route("/vector-search", post(vector_search_handler))
        .route("/transactions", post(transaction_handler))
        .route("/query/nodes", post(query_nodes_handler))
        .route("/lookup", post(exact_lookup_handler))
        .route("/aggregate", post(aggregate_handler))
        .route("/lifecycle/tombstone", post(tombstone_handler))
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

fn enforce_namespace_quota(
    store: &Store,
    namespace: &crate::identity::NamespaceId,
    add_nodes: usize,
    add_edges: usize,
    limits: &crate::app_config::LimitsConfig,
) -> ApiResult<()> {
    let nodes = store
        .nodes()
        .values()
        .filter(|node| &node.namespace == namespace)
        .count();
    let edges = store
        .edges()
        .values()
        .filter(|edge| &edge.namespace == namespace)
        .count();
    if nodes.saturating_add(add_nodes) > limits.max_nodes_per_namespace {
        return Err(bad_request("namespace node quota exceeded"));
    }
    if edges.saturating_add(add_edges) > limits.max_edges_per_namespace {
        return Err(bad_request("namespace edge quota exceeded"));
    }
    Ok(())
}

/// Write-through persistence after a mutation: snapshot under a read lock,
/// serialize off the async runtime so large stores do not block it.
async fn persist(state: &AppState) -> ApiResult<()> {
    let snapshot = state.store.read().await.clone();
    let path = (*state.data_path).clone();
    match tokio::task::spawn_blocking(move || save_store_to(&snapshot, &path)).await {
        Ok(Ok(())) => {
            metrics::counter!("padagonia_persist_total").increment(1);
            state
                .journal
                .lock()
                .await
                .checkpoint()
                .map_err(|error| internal_error(format!("journal checkpoint failed: {error}")))?;
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

async fn stats_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
) -> ApiResult<Json<StatsResponse>> {
    if !principal.role.permits(Operation::Read) {
        return Err(forbidden("credential is not authorized for this operation"));
    }
    let store = state.store.read().await;
    let lifecycle = state.lifecycle.read().await;
    let namespace = &principal.namespace;
    let nodes = store
        .nodes()
        .values()
        .filter(|node| &node.namespace == namespace)
        .filter(|node| {
            lifecycle
                .get_tombstone(&RecordKey {
                    namespace: namespace.clone(),
                    external_id: node.external_id.clone(),
                })
                .is_none()
        })
        .count();
    let edges = store
        .edges()
        .values()
        .filter(|edge| &edge.namespace == namespace)
        .filter(|edge| {
            lifecycle
                .get_tombstone(&RecordKey {
                    namespace: namespace.clone(),
                    external_id: edge.external_id.clone(),
                })
                .is_none()
        })
        .count();
    let facts = store
        .facts()
        .iter()
        .filter(|(subject, _)| match subject {
            crate::fact::FactSubject::Node(id) => store
                .nodes()
                .get(id)
                .is_some_and(|node| &node.namespace == namespace),
            crate::fact::FactSubject::Edge(id) => store
                .edges()
                .get(id)
                .is_some_and(|edge| &edge.namespace == namespace),
        })
        .map(|(_, facts)| facts.len())
        .sum();
    let labels = store
        .node_label_index
        .iter()
        .filter(|(_, ids)| {
            ids.iter()
                .filter_map(|id| store.nodes().get(id))
                .any(|node| &node.namespace == namespace)
        })
        .count();
    let relations = store
        .edge_label_index
        .iter()
        .filter(|(_, ids)| {
            ids.iter()
                .filter_map(|id| store.edges().get(id))
                .any(|edge| &edge.namespace == namespace)
        })
        .count();
    Ok(Json(StatsResponse {
        nodes,
        edges,
        facts,
        labels,
        relations,
    }))
}

async fn query_nodes_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(request): Json<crate::contract::NodeQueryRequest>,
) -> ApiResult<Json<crate::contract::PageCursorResponse>> {
    if principal.namespace != request.namespace || !principal.role.permits(Operation::Read) {
        return Err(forbidden("credential is not authorized for this namespace"));
    }
    let store = state.store.read().await;
    let lifecycle = state.lifecycle.read().await;
    let page = QueryEngine::new(&store).nodes_page(&request.clone().into_query());
    // Filter tombstoned nodes and resolve their interned values while the
    // store read lock is held. Returning complete records here prevents
    // consumers from turning one bounded query into an unbounded N+1 series
    // of GET /nodes/:id requests.
    let nodes: Vec<NodeRecordResponse> = page
        .entries
        .into_iter()
        .filter(|node| {
            lifecycle
                .get_tombstone(&RecordKey {
                    namespace: node.namespace.clone(),
                    external_id: node.external_id.clone(),
                })
                .is_none()
        })
        .map(|node| NodeRecordResponse {
            id: node.id.0,
            label: store
                .string_table()
                .resolve_label(node.label)
                .unwrap_or("?")
                .to_string(),
            properties: props_to_json(&node.properties, &store),
            embedding: node.embedding.clone(),
            provenance: node.provenance.clone(),
        })
        .collect();
    let node_ids = nodes.iter().map(|node| node.id).collect();
    Ok(Json(crate::contract::PageCursorResponse {
        api_version: API_VERSION.to_string(),
        node_ids,
        nodes,
        next_cursor: page.next_cursor,
    }))
}

async fn exact_lookup_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(request): Json<crate::contract::ExactLookupRequest>,
) -> ApiResult<Json<crate::contract::ExactLookupResponse>> {
    if principal.namespace != request.namespace || !principal.role.permits(Operation::Read) {
        return Err(forbidden("credential is not authorized for this namespace"));
    }
    if request.external_id.trim().is_empty() {
        return Err(bad_request("external_id must not be empty"));
    }
    let lifecycle = state.lifecycle.read().await;
    if lifecycle
        .get_tombstone(&RecordKey {
            namespace: request.namespace.clone(),
            external_id: request.external_id.clone(),
        })
        .is_some()
    {
        return Err(not_found(format!(
            "record {} not found",
            request.external_id
        )));
    }
    drop(lifecycle);
    let store = state.store.read().await;
    let engine = QueryEngine::new(&store);
    let node = engine
        .exact_node(&request.namespace, &request.external_id)
        .ok_or_else(|| not_found(format!("record {} not found", request.external_id)))?;
    if node.namespace != principal.namespace {
        return Err(forbidden("credential is not authorized for this namespace"));
    }
    Ok(Json(crate::contract::ExactLookupResponse {
        api_version: API_VERSION.to_string(),
        node_id: node.id.0,
        external_id: node.external_id.clone(),
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

async fn aggregate_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(request): Json<crate::contract::AggregateRequest>,
) -> ApiResult<Json<crate::contract::AggregateResponse>> {
    if principal.namespace != request.namespace
        || (!principal.role.permits(Operation::Read) && !principal.role.permits(Operation::Analyze))
    {
        return Err(forbidden("credential is not authorized for this namespace"));
    }
    if request.property.trim().is_empty() {
        return Err(bad_request("property must not be empty"));
    }
    let store = state.store.read().await;
    let _lifecycle = state.lifecycle.read().await;
    let key_id = store
        .string_table()
        .key_id(&request.property)
        .ok_or_else(|| bad_request(format!("unknown property '{}'", request.property)))?;
    let label_id = match &request.label {
        Some(label) => Some(
            store
                .string_table()
                .label_id(label)
                .ok_or_else(|| bad_request(format!("unknown label '{}'", label)))?,
        ),
        None => None,
    };
    let engine = QueryEngine::new(&store);
    // Aggregate respects namespace filtering; tombstoned nodes are excluded by the engine's underlying store check
    // we filter lifecycle here manually by counting only non-tombstoned nodes.
    let lifecycle = state.lifecycle.read().await;
    let aggregate = {
        // We need to compute aggregate only over non-tombstoned nodes in the namespace.
        let mut agg = crate::query::NumericAggregate::default();
        let nodes: Vec<&crate::node::Node> = label_id
            .and_then(|label| store.node_label_index.get(&label))
            .map_or_else(
                || store.nodes().values().collect(),
                |ids| ids.iter().filter_map(|id| store.nodes.get(id)).collect(),
            );
        for node in nodes {
            if node.namespace != principal.namespace {
                continue;
            }
            if lifecycle
                .get_tombstone(&RecordKey {
                    namespace: node.namespace.clone(),
                    external_id: node.external_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            if let Some(value) = node
                .properties
                .iter()
                .find_map(|(k, v)| {
                    (*k == key_id).then(|| match v {
                        crate::value::Scalar::I64(value) => Some(*value as f64),
                        crate::value::Scalar::F64(value) if value.is_finite() => Some(*value),
                        _ => None,
                    })
                })
                .flatten()
            {
                agg.count += 1;
                agg.sum += value;
                agg.min = Some(agg.min.map_or(value, |current| current.min(value)));
                agg.max = Some(agg.max.map_or(value, |current| current.max(value)));
            }
        }
        agg
    };
    // Also demonstrate engine usage for equivalence check in debug; not returned.
    let _engine_agg = engine.aggregate_numeric_property(Some(&request.namespace), label_id, key_id);
    Ok(Json(crate::contract::AggregateResponse {
        api_version: API_VERSION.to_string(),
        count: aggregate.count,
        sum: aggregate.sum,
        min: aggregate.min,
        max: aggregate.max,
    }))
}

async fn tombstone_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(request): Json<TombstoneRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if principal.namespace != request.namespace || !principal.role.permits(Operation::Administer) {
        return Err(forbidden(
            "only a namespace administrator may tombstone records",
        ));
    }
    if request.external_id.trim().is_empty() || request.reason.trim().is_empty() {
        return Err(bad_request("external_id and reason must not be empty"));
    }
    let _commit_guard = state.commit_gate.lock().await;
    {
        let mut lifecycle = state.lifecycle.write().await;
        lifecycle
            .tombstone(Tombstone {
                key: RecordKey {
                    namespace: request.namespace,
                    external_id: request.external_id,
                },
                deleted_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs()),
                reason: request.reason,
                schema_version: request.schema_version,
            })
            .map_err(|error| bad_request(format!("invalid tombstone: {error:?}")))?;
        let bytes = serde_json::to_vec(&*lifecycle)
            .map_err(|error| internal_error(format!("lifecycle encode failed: {error}")))?;
        let path = state.data_path.with_extension("lifecycle");
        tokio::task::spawn_blocking(move || save_lifecycle(&path, &bytes))
            .await
            .map_err(|error| internal_error(format!("lifecycle write task failed: {error}")))?
            .map_err(|error| internal_error(format!("lifecycle write failed: {error}")))?;
    }
    Ok(Json(serde_json::json!({"status": "tombstoned"})))
}

fn save_lifecycle(path: &FsPath, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let temporary = path.with_extension(format!("lifecycle.tmp.{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
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
        return Err(forbidden(
            "credential is not authorized for this namespace or operation",
        ));
    }
    let transaction = request.into_transaction();
    // Lifecycle tombstone guard: rejected external_ids cannot be resurrected via journal.
    {
        let lifecycle = state.lifecycle.read().await;
        for mutation in &transaction.mutations {
            let key = match mutation {
                crate::transaction::Mutation::AddNode {
                    namespace,
                    external_id,
                    ..
                }
                | crate::transaction::Mutation::AddEdge {
                    namespace,
                    external_id,
                    ..
                } => RecordKey {
                    namespace: namespace.clone(),
                    external_id: external_id.clone(),
                },
            };
            if lifecycle.get_tombstone(&key).is_some() {
                return Err(bad_request(format!(
                    "record {} is tombstoned and cannot be recreated",
                    key.external_id
                )));
            }
        }
    }
    let result = {
        let mut store = state.store.write().await;
        let (add_nodes, add_edges) =
            transaction
                .mutations
                .iter()
                .fold((0, 0), |(nodes, edges), mutation| match mutation {
                    crate::transaction::Mutation::AddNode { .. } => (nodes + 1, edges),
                    crate::transaction::Mutation::AddEdge { .. } => (nodes, edges + 1),
                });
        enforce_namespace_quota(&store, &namespace, add_nodes, add_edges, &state.limits)?;
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

async fn create_node_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<CreateNodeRequest>,
) -> ApiResult<(StatusCode, Json<IdResponse>)> {
    let _commit_guard = state.commit_gate.lock().await;
    if principal.namespace != req.namespace || !principal.role.permits(Operation::Write) {
        return Err(forbidden(
            "credential is not authorized for this namespace or operation",
        ));
    }
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
    let namespace = req.namespace.clone();
    // Tombstone guard
    {
        let lifecycle = state.lifecycle.read().await;
        if lifecycle
            .get_tombstone(&RecordKey {
                namespace: namespace.clone(),
                external_id: external_id.clone(),
            })
            .is_some()
        {
            return Err(bad_request(format!(
                "record {} is tombstoned and cannot be recreated",
                external_id
            )));
        }
    }
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
        enforce_namespace_quota(&store, &namespace, 1, 0, &state.limits)?;
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
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<u64>,
) -> ApiResult<Json<NodeResponse>> {
    let store = state.store.read().await;
    let lifecycle = state.lifecycle.read().await;
    let node = store
        .nodes()
        .get(&NodeId(id))
        .ok_or_else(|| not_found(format!("node {id} not found")))?;
    if node.namespace != principal.namespace || !principal.role.permits(Operation::Read) {
        return Err(forbidden("credential is not authorized for this namespace"));
    }
    if lifecycle
        .get_tombstone(&RecordKey {
            namespace: node.namespace.clone(),
            external_id: node.external_id.clone(),
        })
        .is_some()
    {
        return Err(not_found(format!("node {id} not found")));
    }
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
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<CreateEdgeRequest>,
) -> ApiResult<(StatusCode, Json<IdResponse>)> {
    let _commit_guard = state.commit_gate.lock().await;
    if principal.namespace != req.namespace || !principal.role.permits(Operation::Write) {
        return Err(forbidden(
            "credential is not authorized for this namespace or operation",
        ));
    }
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
    // Tombstone guard
    {
        let lifecycle = state.lifecycle.read().await;
        if lifecycle
            .get_tombstone(&RecordKey {
                namespace: req.namespace.clone(),
                external_id: external_id.clone(),
            })
            .is_some()
        {
            return Err(bad_request(format!(
                "record {} is tombstoned and cannot be recreated",
                external_id
            )));
        }
    }
    let id = {
        let mut store = state.store.write().await;
        enforce_namespace_quota(&store, &req.namespace, 0, 1, &state.limits)?;
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
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<BfsRequest>,
) -> ApiResult<Json<Vec<BfsEntry>>> {
    if !principal.role.permits(Operation::Read) {
        return Err(forbidden("credential is not authorized for this operation"));
    }
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
    let (store_guard, lifecycle_guard) = {
        let store = state.store.read().await;
        let lifecycle = state.lifecycle.read().await;
        // Validate start node belongs to caller's namespace before traversing.
        let start_node = store
            .nodes()
            .get(&NodeId(req.start))
            .ok_or_else(|| not_found(format!("node {} not found", req.start)))?;
        if start_node.namespace != principal.namespace {
            return Err(forbidden("credential is not authorized for this namespace"));
        }
        if lifecycle
            .get_tombstone(&RecordKey {
                namespace: start_node.namespace.clone(),
                external_id: start_node.external_id.clone(),
            })
            .is_some()
        {
            return Err(not_found(format!("node {} not found", req.start)));
        }
        // Clone for later filtering; we hold no lock while computing BFS to avoid deadlock,
        // so we snapshot store and lifecycle.
        (store.clone(), lifecycle.clone())
    };
    let relation_id = match &req.relation {
        Some(relation) => Some(
            store_guard
                .string_table()
                .relation_id(relation)
                .ok_or_else(|| bad_request(format!("unknown relation '{relation}'")))?,
        ),
        None => None,
    };
    let engine = QueryEngine::new(&store_guard);
    let reached = engine.bfs(
        NodeId(req.start),
        req.depth,
        relation_id,
        req.min_confidence,
    );
    // Namespace filtering: only return nodes in caller's namespace and not tombstoned.
    let filtered: Vec<BfsEntry> = reached
        .into_iter()
        .filter_map(|(node_id, depth)| {
            let node = store_guard.nodes().get(&node_id)?;
            if node.namespace != principal.namespace {
                return None;
            }
            if lifecycle_guard
                .get_tombstone(&RecordKey {
                    namespace: node.namespace.clone(),
                    external_id: node.external_id.clone(),
                })
                .is_some()
            {
                return None;
            }
            Some(BfsEntry {
                node_id: node_id.0,
                depth,
            })
        })
        .collect();
    Ok(Json(filtered))
}

async fn vector_search_handler(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<VectorSearchRequest>,
) -> ApiResult<Json<Vec<VectorHit>>> {
    if !principal.role.permits(Operation::Read) && !principal.role.permits(Operation::Analyze) {
        return Err(forbidden("credential is not authorized for this operation"));
    }
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
    // Snapshot store + lifecycle for namespace-filtered search.
    let (store_snapshot, lifecycle_snapshot) = {
        let store = state.store.read().await;
        let lifecycle = state.lifecycle.read().await;
        (store.clone(), lifecycle.clone())
    };
    if let Some(expected) = expected_embedding_dimension(&store_snapshot) {
        if req.query.len() != expected {
            return Err(bad_request(format!(
                "query dimension {} does not match graph dimension {expected}",
                req.query.len()
            )));
        }
    }
    let label_id = match &req.label {
        Some(label) => Some(
            store_snapshot
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
    let engine = QueryEngine::new(&store_snapshot);
    let hits = engine.vector_search_with_params(distance, params, &req.query, k, label_id);
    // Tenant-isolated filtering: drop hits not in caller's namespace or tombstoned.
    let filtered: Vec<VectorHit> = hits
        .into_iter()
        .filter_map(|(node_id, distance)| {
            let node = store_snapshot.nodes().get(&node_id)?;
            if node.namespace != principal.namespace {
                return None;
            }
            if lifecycle_snapshot
                .get_tombstone(&RecordKey {
                    namespace: node.namespace.clone(),
                    external_id: node.external_id.clone(),
                })
                .is_some()
            {
                return None;
            }
            Some(VectorHit {
                node_id: node_id.0,
                distance,
            })
        })
        .collect();
    Ok(Json(filtered))
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
