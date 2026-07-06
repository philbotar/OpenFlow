use engine::AgentError;

/// Map an HTTP status and response body to [`AgentError`] with retry semantics.
///
/// Proxy gateways (e.g. `OpenCode Zen`) often surface upstream blips as `HTTP 400` with an
/// opaque body; those are treated as [`AgentError::Transient`] so the engine retry policy
/// can absorb them. Auth failures stay [`AgentError::Permanent`].
#[must_use]
pub fn classify_http_status(status: u16, body: &str, label: &str) -> AgentError {
    let message = format!("{label} returned HTTP {status}: {body}");
    if status == 401 {
        return AgentError::Permanent(message);
    }
    // Proxy gateways may surface upstream blips on any 4xx/5xx with a recognizable body.
    if is_retryable_proxy_body(body) {
        return AgentError::Transient(message);
    }
    match status {
        408 | 409 | 429 | 500..=599 => AgentError::Transient(message),
        400 => AgentError::Failed(message),
        _ => AgentError::Permanent(message),
    }
}

pub fn is_retryable_proxy_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    [
        "upstream request failed",
        "provider returned error",
        "temporarily unavailable",
        "service unavailable",
        "overloaded",
        "bad gateway",
        "gateway timeout",
        "error decoding response body",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_errors_are_permanent() {
        let err = classify_http_status(401, "bad key", "OpenAI-compatible");
        assert!(matches!(err, AgentError::Permanent(_)));
        assert!(!err.is_retryable());
    }

    #[test]
    fn rate_limit_and_server_errors_are_transient() {
        assert!(classify_http_status(429, "rate limited", "OpenAI-compatible").is_retryable());
        assert!(classify_http_status(500, "boom", "OpenAI-compatible").is_retryable());
    }

    #[test]
    fn opaque_proxy_upstream_400_is_transient() {
        let body =
            r#"{"error":{"message":"Error from provider (Console Go): Upstream request failed"}}"#;
        let err = classify_http_status(400, body, "OpenAI-compatible");
        assert!(err.is_retryable(), "expected transient, got {err}");
    }

    #[test]
    fn opaque_proxy_upstream_403_is_transient() {
        let body =
            r#"{"error":{"message":"Error from provider (Console Go): Upstream request failed"}}"#;
        let err = classify_http_status(403, body, "OpenAI-compatible");
        assert!(err.is_retryable(), "expected transient, got {err}");
        assert!(!matches!(err, AgentError::Permanent(_)));
    }

    #[test]
    fn proxy_upstream_decode_failure_is_transient() {
        let body = "Http client error: error decoding response body";
        assert!(is_retryable_proxy_body(body));
    }

    #[test]
    fn schema_400_is_failed_not_retryable() {
        let body = r"invalid 'parameters' schema";
        let err = classify_http_status(400, body, "OpenAI-compatible");
        assert!(!err.is_retryable());
        assert!(matches!(err, AgentError::Failed(_)));
    }
}
