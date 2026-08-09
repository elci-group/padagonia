use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use metrics_exporter_prometheus::PrometheusBuilder;
use padagonia::app_config::LimitsConfig;
use padagonia::ontology::StringTableExt;
use padagonia::server::{router, AppState};
use padagonia::store::Store;
use serde_json::{json, Value};
use tower::ServiceExt;

const KEY: &str = "test-secret";

fn test_app() -> (Router, tempfile::TempDir) {
    test_app_with_limits(LimitsConfig::default())
}

fn test_app_with_limits(limits: LimitsConfig) -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let handle = PrometheusBuilder::new().build_recorder().handle();
    let state = AppState::new_with_limits(
        Store::new(),
        handle,
        KEY,
        dir.path().join("store.pad"),
        (16, 64, 50),
        limits,
    );
    (router(state, "/metrics"), dir)
}

async fn request(
    app: &Router,
    method: &str,
    uri: &str,
    key: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(key) = key {
        builder = builder.header("authorization", format!("Bearer {key}"));
    }
    let body = match body {
        Some(value) => Body::from(serde_json::to_vec(&value).unwrap()),
        None => Body::empty(),
    };
    let request = builder
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn public_endpoints_do_not_require_auth() {
    let (app, _dir) = test_app();
    for uri in ["/health", "/ready", "/metrics"] {
        let (status, _) = request(&app, "GET", uri, None, None).await;
        assert_eq!(status, StatusCode::OK, "{uri} should be public");
    }
}

#[tokio::test]
async fn protected_routes_require_valid_bearer_token() {
    let (app, _dir) = test_app();

    let (status, _) = request(&app, "GET", "/api/v1/stats", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = request(&app, "GET", "/api/v1/stats", Some("wrong"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = request(&app, "GET", "/api/v1/stats", Some(KEY), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["nodes"], 0);
    assert_eq!(body["edges"], 0);
}

#[tokio::test]
async fn versioned_transaction_route_is_namespace_bound_and_idempotent() {
    let (app, _dir) = test_app();
    let body = json!({
        "namespace": "workspace",
        "idempotency_key": "txn-1",
        "mutations": [{"AddNode": {
            "namespace": "workspace",
            "external_id": "run-1",
            "label": "Run",
            "properties": [],
            "embedding": null,
            "provenance": {"agent": "test", "model": "fixture", "confidence": 1.0, "cost": 0.0, "timestamp": 1, "evidence": []}
        }}]
    });
    let tx_request = |namespace: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/v1/transactions")
            .header("authorization", format!("Bearer {KEY}"))
            .header("x-padagonia-namespace", namespace)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    };
    let response = app.clone().oneshot(tx_request("workspace")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app.clone().oneshot(tx_request("workspace")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (_, stats) = request(&app, "GET", "/api/v1/stats", Some(KEY), None).await;
    assert_eq!(stats["nodes"], 1);
}

#[tokio::test]
async fn create_node_and_edge_then_read_back() {
    let (app, _dir) = test_app();

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/nodes",
        Some(KEY),
        Some(json!({
            "label": "Person",
            "properties": {"name": "alice", "age": 34},
            "provenance": {"agent": "tester", "model": "m1", "confidence": 0.9}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["id"], 0);

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/nodes",
        Some(KEY),
        Some(json!({"label": "Company"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["id"], 1);

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/edges",
        Some(KEY),
        Some(json!({"src": 0, "dst": 1, "label": "works_for"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["id"], 0);

    let (status, body) = request(&app, "GET", "/api/v1/nodes/0", Some(KEY), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["label"], "Person");
    assert_eq!(body["properties"]["name"], "alice");
    assert_eq!(body["properties"]["age"], 34);
    assert_eq!(body["provenance"]["agent"], "tester");
    assert_eq!(body["provenance"]["confidence"], 0.9);

    let (_, stats) = request(&app, "GET", "/api/v1/stats", Some(KEY), None).await;
    assert_eq!(stats["nodes"], 2);
    assert_eq!(stats["edges"], 1);
    assert_eq!(stats["facts"], 3);
}

#[tokio::test]
async fn create_edge_rejects_unknown_endpoints() {
    let (app, _dir) = test_app();

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/edges",
        Some(KEY),
        Some(json!({"src": 0, "dst": 7, "label": "knows"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("src"));
}

#[tokio::test]
async fn get_unknown_node_is_404() {
    let (app, _dir) = test_app();
    let (status, _) = request(&app, "GET", "/api/v1/nodes/42", Some(KEY), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mutations_are_persisted_to_disk() {
    let (app, dir) = test_app();

    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/nodes",
        Some(KEY),
        Some(json!({"label": "Person", "properties": {"name": "persisted"}})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let store = Store::load(dir.path().join("store.pad")).unwrap();
    let (nodes, _, _, _, _) = store.stats();
    assert_eq!(nodes, 1);
    assert!(store.string_table().label_id("Person").is_some());
}

#[tokio::test]
async fn bfs_endpoint_traverses_created_edges() {
    let (app, _dir) = test_app();

    for label in ["A", "B"] {
        request(
            &app,
            "POST",
            "/api/v1/nodes",
            Some(KEY),
            Some(json!({"label": label})),
        )
        .await;
    }
    request(
        &app,
        "POST",
        "/api/v1/edges",
        Some(KEY),
        Some(json!({"src": 0, "dst": 1, "label": "knows"})),
    )
    .await;

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/bfs",
        Some(KEY),
        Some(json!({"start": 0, "depth": 2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().unwrap();
    assert!(entries.iter().any(|e| e["node_id"] == 1 && e["depth"] == 1));

    // A confidence threshold above the default edge confidence prunes the edge.
    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/bfs",
        Some(KEY),
        Some(json!({"start": 0, "depth": 2, "min_confidence": 1.5})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("min_confidence"));

    // Unknown relation names are rejected, not panicked on.
    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/bfs",
        Some(KEY),
        Some(json!({"start": 0, "depth": 2, "relation": "nope"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn openapi_and_request_ids_are_exposed() {
    let (app, _dir) = test_app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let document: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(document["openapi"], "3.1.0");
}

#[tokio::test]
async fn operation_limits_return_structured_errors() {
    let limits = LimitsConfig {
        max_ingest_nodes: 2,
        max_ingest_edges: 3,
        max_bfs_depth: 1,
        max_vector_dimensions: 2,
        max_vector_results: 1,
        max_vector_ef: 2,
        ..LimitsConfig::default()
    };
    let (app, _dir) = test_app_with_limits(limits);

    let cases = [
        ("/api/v1/ingest", json!({"nodes": 3, "edges": 0, "seed": 1})),
        ("/api/v1/bfs", json!({"start": 0, "depth": 2})),
        ("/api/v1/vector-search", json!({"query": [0.0, 1.0, 2.0]})),
    ];
    for (uri, body) in cases {
        let (status, body) = request(&app, "POST", uri, Some(KEY), Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert!(body["error"].is_string(), "{uri}: {body}");
    }
}

#[tokio::test]
async fn request_body_and_rate_limits_fail_closed() {
    let body_limits = LimitsConfig {
        request_body_bytes: 32,
        ..LimitsConfig::default()
    };
    let (app, _dir) = test_app_with_limits(body_limits);
    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/nodes",
        Some(KEY),
        Some(json!({"label": "a label deliberately larger than the configured body"})),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(body["error"].is_string());

    let rate_limits = LimitsConfig {
        requests_per_second: 0,
        request_burst: 2,
        ..LimitsConfig::default()
    };
    let (app, _dir) = test_app_with_limits(rate_limits);
    for expected in [
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let (status, body) = request(&app, "GET", "/api/v1/stats", Some(KEY), None).await;
        assert_eq!(status, expected);
        if expected == StatusCode::TOO_MANY_REQUESTS {
            assert!(body["error"].is_string());
        }
    }
}

#[tokio::test]
async fn vector_search_endpoint_ranks_by_distance() {
    let (app, _dir) = test_app();

    for (label, embedding) in [("A", vec![0.0, 0.0]), ("A", vec![1.0, 1.0])] {
        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/nodes",
            Some(KEY),
            Some(json!({"label": label, "embedding": embedding})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/vector-search",
        Some(KEY),
        Some(json!({"query": [0.0, 0.0], "k": 2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hits = body.as_array().unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["node_id"], 0);
    assert!(hits[0]["distance"].as_f64().unwrap() < hits[1]["distance"].as_f64().unwrap());

    let (status, _) = request(
        &app,
        "POST",
        "/api/v1/vector-search",
        Some(KEY),
        Some(json!({"query": [], "k": 2})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ingest_endpoint_generates_and_persists() {
    let (app, dir) = test_app();

    let (status, body) = request(
        &app,
        "POST",
        "/api/v1/ingest",
        Some(KEY),
        Some(json!({"nodes": 10, "edges": 20, "seed": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["nodes"], 10);
    assert_eq!(body["edges"], 20);

    let store = Store::load(dir.path().join("store.pad")).unwrap();
    let (nodes, edges, _, _, _) = store.stats();
    assert_eq!(nodes, 10);
    assert_eq!(edges, 20);
}
