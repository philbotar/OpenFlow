use engine::AgentError;

/// Map an HTTP status and response body to [`AgentError`] with retry semantics.
///
/// Proxy gateways (e.g. `OpenCode Zen`) often surface upstream blips as `HTTP 400` with an
/// opaque body; those are treated as [`AgentError::Transient`] so the engine retry policy
/// can absorb them. Auth failures stay [`AgentError::Permanent`].
#[must_use]
pub fn classify_http_status(status: u16, body: &str, label: &str) -> AgentError {
    let message = format!("{label} returned HTTP {status}: {body}");
    match status {
        408 | 409 | 429 | 500..=599 => AgentError::Transient(message),
        400 if is_retryable_proxy_body(body) => AgentError::Transient(message),
        400 => AgentError::Failed(message),
        _ => AgentError::Permanent(message),
    }
}

fn is_retryable_proxy_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    [
        "upstream request failed",
        "provider returned error",
        "temporarily unavailable",
        "service unavailable",
        "overloaded",
        "bad gateway",
        "gateway timeout",
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
    fn schema_400_is_failed_not_retryable() {
        let body = r"invalid 'parameters' schema";
        let err = classify_http_status(400, body, "OpenAI-compatible");
        assert!(!err.is_retryable());
        assert!(matches!(err, AgentError::Failed(_)));
    }
}
