//! Authentication middleware and helpers for HTTP server.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::sync::Arc;

/// Middleware that checks for a valid Bearer token on protected routes.
///
/// The configured key is compared in constant time to avoid timing attacks.
pub async fn auth_middleware(
    axum::extract::State(api_key): axum::extract::State<Arc<str>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = format!("Bearer {}", api_key);
    let auth_header = request.headers().get("authorization");

    match auth_header {
        Some(value) => match value.to_str() {
            Ok(header) if constant_time_eq(header.as_bytes(), expected.as_bytes()) => {
                Ok(next.run(request).await)
            }
            Ok(_) => {
                tracing::warn!(
                    event = "authentication_failed",
                    reason = "credential_mismatch",
                    "protected request rejected"
                );
                Err(StatusCode::UNAUTHORIZED)
            }
            Err(error) => {
                tracing::warn!(
                    event = "authentication_failed",
                    reason = "invalid_header_encoding",
                    error = %error,
                    "protected request rejected"
                );
                Err(StatusCode::UNAUTHORIZED)
            }
        },
        None => {
            tracing::warn!(
                event = "authentication_failed",
                reason = "missing_authorization_header",
                "protected request rejected"
            );
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Constant-time comparison for API keys to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0_u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_only_identical_inputs() {
        assert!(constant_time_eq(b"Bearer s3cret", b"Bearer s3cret"));
        assert!(!constant_time_eq(b"Bearer s3cret", b"Bearer s3creX"));
        assert!(!constant_time_eq(b"Bearer s3cret", b"Bearer"));
        assert!(!constant_time_eq(b"", b"Bearer "));
    }
}
