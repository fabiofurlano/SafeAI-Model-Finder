use axum::{
    Json,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use rand::Rng;

/// Generate a random per-launch session token for protecting mutating requests.
pub fn generate_session_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    hex::encode(bytes)
}

/// AXUM middleware that rejects requests with unexpected Host headers.
/// Only allows 127.0.0.1:PORT and localhost:PORT and [::1]:PORT.
pub async fn validate_host(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Always allow loopback — the actual binding restriction handles non-loopback
    if let Some(host) = headers.get("host") {
        if let Ok(host_str) = host.to_str() {
            let host_lower = host_str.to_lowercase();
            let host_part = host_lower.split(':').next().unwrap_or(&host_lower);

            // Allow only loopback hostnames
            if host_part != "127.0.0.1" && host_part != "localhost" && host_part != "[::1]" {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    Ok(next.run(request).await)
}
/// Centralised per-handler gate: returns `Err(UNAUTHORIZED)` unless the
/// request carries the live session token via `Authorization: Bearer`,
/// `x-smf-token:`, or `?token=` in the URL query string.
pub fn require_session(
    headers: &HeaderMap,
    query_string: Option<&str>,
    state_token: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let provided = if let Some(auth) = headers.get("authorization") {
        auth.to_str()
            .ok()
            .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.to_string()))
    } else {
        None
    }
    .or_else(|| {
        // Token may come in via the `?token=` query parameter; the handler
        // hands us the raw query string so we can split it ourselves.
        query_string.and_then(|q| {
            q.split('&')
                .find_map(|p| p.strip_prefix("token=").map(|t| t.to_string()))
        })
    })
    .or_else(|| {
        headers
            .get("x-smf-token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });
    match provided {
        Some(t) if t == state_token => Ok(()),
        _ => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Missing or invalid session token. Refresh the UI and retry."
            })),
        )),
    }
}

/// Extract session token from `Authorization: Bearer` header or `?token=`
/// in the query string. Kept for unit-test parity; the live gate is
/// `require_session`.
#[allow(dead_code)]
pub fn extract_session_token(headers: &HeaderMap, query: &str) -> Option<String> {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}
