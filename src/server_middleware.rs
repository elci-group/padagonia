//! HTTP rate, correlation, and structured-error middleware.

use crate::http_error::ErrorResponse;
use crate::server_state::AppState;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::Instrument;

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> axum::response::Response {
    if !state.allow_request().await {
        metrics::counter!("padagonia_http_rate_limited_total").increment(1);
        tracing::warn!(event = "request_rate_limited", "HTTP request rejected");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "request rate limit exceeded".to_string(),
            }),
        )
            .into_response();
    }
    next.run(request).await
}

pub(crate) async fn request_context(request: Request, next: Next) -> axum::response::Response {
    let supplied = match request.headers().get("x-request-id") {
        Some(value) => match value.to_str() {
            Ok(value) if !value.is_empty() && value.len() <= 128 => Some(value),
            Ok(_) => None,
            Err(error) => {
                tracing::debug!(event = "invalid_request_id", error = %error, "request id ignored");
                None
            }
        },
        None => None,
    };
    let request_id = supplied.map(str::to_owned).unwrap_or_else(|| {
        format!(
            "pad-{}-{}",
            std::process::id(),
            REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    });
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %request.method(),
        path = %request.uri().path()
    );
    let mut response = next.run(request).instrument(span).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

pub(crate) async fn normalize_error_responses(
    request: Request,
    next: Next,
) -> axum::response::Response {
    let response = next.run(request).await;
    if !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }
    let is_json = match response.headers().get(header::CONTENT_TYPE) {
        Some(value) => match value.to_str() {
            Ok(value) => value.starts_with("application/json"),
            Err(error) => {
                tracing::debug!(event = "invalid_response_content_type", error = %error, "non-text content type treated as unstructured");
                false
            }
        },
        None => false,
    };
    if is_json {
        return response;
    }
    let status = response.status();
    (
        status,
        Json(ErrorResponse {
            error: status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_string(),
        }),
    )
        .into_response()
}
