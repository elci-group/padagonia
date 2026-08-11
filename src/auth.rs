//! Authentication middleware and helpers for HTTP server.

use crate::authorization::AuthenticatedPrincipal;
use crate::identity::NamespaceId;
use crate::server_state::AppState;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};

/// Middleware that checks for a valid Bearer token on protected routes.
///
/// The configured key is compared in constant time to avoid timing attacks.
pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request.headers().get("authorization");
    let namespace_header = request.headers().get("x-padagonia-namespace");

    match auth_header {
        Some(value) => match (
            value.to_str(),
            Some(namespace_header.and_then(|v| v.to_str().ok()).unwrap_or("default")),
        ) {
            (Ok(header), Some(namespace_value)) if header.starts_with("Bearer ") => {
                let token = &header["Bearer ".len()..];
                let namespace = NamespaceId::new(namespace_value).map_err(|_| StatusCode::BAD_REQUEST)?;
                let registry = state.credentials.read().await;
                let principal = registry
                    .authenticate(token, &namespace)
                    .ok_or(StatusCode::UNAUTHORIZED)?;
                request.extensions_mut().insert(principal);
                Ok(next.run(request).await)
            }
            (Ok(_), _) => {
                tracing::warn!(
                    event = "authentication_failed",
                    reason = "credential_mismatch",
                    "protected request rejected"
                );
                Err(StatusCode::UNAUTHORIZED)
            }
            (Err(error), _) => {
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

/// Retrieve the authenticated tenant context from a protected request.
pub fn principal(request: &Request) -> Result<&AuthenticatedPrincipal, StatusCode> {
    request
        .extensions()
        .get::<AuthenticatedPrincipal>()
        .ok_or(StatusCode::UNAUTHORIZED)
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
