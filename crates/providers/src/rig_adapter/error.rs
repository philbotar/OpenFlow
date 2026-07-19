//! rig `CompletionError` → `AgentError` with retryability classification.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "rig migration: wired when AiClient switches to rig_adapter"
    )
)]

use crate::http_errors::classify_http_status;
use crate::rig_adapter::outcome::EMPTY_TURN_ERROR;
use engine::AgentError;
use rig_core::completion::CompletionError;
use rig_core::http_client::Error as HttpClientError;

/// Map HTTP status + body to [`AgentError`], reusing the existing provider classification table.
#[must_use]
pub fn classify_status(status: u16, body: &str, label: &str) -> AgentError {
    classify_http_status(status, body, label)
}

/// True only for the structured 401 shape produced by this adapter's HTTP
/// status mapper. Matching the prefix avoids treating arbitrary response text
/// that happens to mention 401 as an authentication failure.
#[must_use]
pub(crate) fn is_unauthorized(error: &AgentError, label: &str) -> bool {
    let AgentError::Permanent(message) = error else {
        return false;
    };
    message.starts_with(&format!("{label} returned HTTP 401:"))
}

/// Rig rejects empty assistant choices with this phrase before OpenFlow outcome mapping runs.
fn is_rig_empty_response(message: &str) -> bool {
    message.contains("no message or tool call")
}

#[must_use]
pub fn to_agent_error(error: CompletionError, label: &str) -> AgentError {
    match error {
        CompletionError::HttpError(http) => http_client_error(http, label),
        CompletionError::JsonError(error) => {
            AgentError::Failed(format!("{label} response JSON error: {error}"))
        }
        CompletionError::UrlError(error) => {
            AgentError::Failed(format!("{label} request URL error: {error}"))
        }
        CompletionError::RequestError(error) => {
            AgentError::Failed(format!("{label} request error: {error}"))
        }
        CompletionError::ResponseError(message) => {
            // Normalize so enrich_empty_turn_error + engine empty-turn retries both fire.
            if is_rig_empty_response(&message) {
                AgentError::Failed(EMPTY_TURN_ERROR.to_string())
            } else {
                AgentError::Failed(format!("{label} response error: {message}"))
            }
        }
        CompletionError::ProviderError(message) => {
            if is_rig_empty_response(&message) {
                return AgentError::Failed(EMPTY_TURN_ERROR.to_string());
            }
            let rendered = format!("{label} provider error: {message}");
            if crate::http_errors::is_retryable_proxy_body(&message) {
                AgentError::Transient(rendered)
            } else {
                AgentError::Failed(rendered)
            }
        }
    }
}

fn http_client_error(error: HttpClientError, label: &str) -> AgentError {
    match error {
        HttpClientError::InvalidStatusCodeWithMessage(status, body) => {
            classify_status(status.as_u16(), &body, label)
        }
        HttpClientError::InvalidStatusCode(status) => classify_status(status.as_u16(), "", label),
        HttpClientError::Instance(source) => {
            let cause = source
                .downcast_ref::<reqwest::Error>()
                .map_or("failed", reqwest_transport_cause);
            AgentError::Transient(format!("{label} HTTP transport {cause}: {source}"))
        }
        HttpClientError::StreamEnded | HttpClientError::Protocol(_) => {
            AgentError::Transient(format!("{label} HTTP transport error: {error}"))
        }
        HttpClientError::NoHeaders
        | HttpClientError::InvalidContentType(_)
        | HttpClientError::InvalidHeaderValue(_) => {
            AgentError::Failed(format!("{label} HTTP client error: {error}"))
        }
    }
}

fn reqwest_transport_cause(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_body() {
        "response body failed"
    } else if error.is_request() {
        "request failed"
    } else {
        "failed"
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "construct invalid JSON for error-path test"
)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn rate_limit_is_transient() {
        let err = classify_status(429, "rate limited", "Anthropic");
        assert!(err.is_retryable());
    }

    #[test]
    fn server_errors_are_transient() {
        assert!(classify_status(500, "boom", "Anthropic").is_retryable());
        assert!(classify_status(529, "overloaded", "Anthropic").is_retryable());
    }

    #[test]
    fn auth_errors_are_permanent() {
        let err = classify_status(401, "bad key", "Anthropic");
        assert!(!err.is_retryable());
        assert!(matches!(err, AgentError::Permanent(_)));
        assert!(is_unauthorized(&err, "Anthropic"));
        assert!(!is_unauthorized(&err, "OpenAI Codex"));
    }

    #[test]
    fn unauthorized_detection_does_not_match_body_text() {
        let error = AgentError::Failed("OpenAI Codex response mentioned HTTP 401".into());
        assert!(!is_unauthorized(&error, "OpenAI Codex"));
    }

    #[test]
    fn bad_request_is_failed_not_retryable() {
        let err = classify_status(400, "invalid model", "Anthropic");
        assert!(matches!(err, AgentError::Failed(_)));
    }

    #[test]
    fn error_message_includes_provider_label_and_body() {
        let err = classify_status(429, "rate limited", "OpenRouter");
        let msg = err.to_string();
        assert!(msg.contains("OpenRouter"));
        assert!(msg.contains("rate limited"));
    }

    #[test]
    fn json_error_is_failed() {
        let err = to_agent_error(
            CompletionError::JsonError(serde_json::from_str::<serde_json::Value>("x").unwrap_err()),
            "Anthropic",
        );
        assert!(matches!(err, AgentError::Failed(_)));
    }

    #[test]
    fn provider_error_with_retryable_proxy_body_is_transient() {
        let err = to_agent_error(
            CompletionError::ProviderError(
                "Invalid status code 400 Bad Request with message: \
                 {\"error\":{\"message\":\"Error from provider (Console Go): Upstream request failed\"}}"
                    .to_string(),
            ),
            "Custom OpenAI-compatible API",
        );
        assert!(err.is_retryable(), "expected transient, got {err}");
    }

    #[test]
    fn provider_error_without_retryable_body_stays_failed() {
        let err = to_agent_error(
            CompletionError::ProviderError("invalid 'parameters' schema".to_string()),
            "Custom OpenAI-compatible API",
        );
        assert!(matches!(err, AgentError::Failed(_)));
    }

    #[test]
    fn rig_empty_response_error_maps_to_empty_provider_turn() {
        let err = to_agent_error(
            CompletionError::ResponseError(
                "Response contained no message or tool call (empty)".to_string(),
            ),
            "Custom OpenAI-compatible API",
        );
        assert!(
            err.is_empty_provider_turn(),
            "expected empty-turn classification, got {err}"
        );
        // Raw Rig phrase must not leak; enrich path expects the canonical marker.
        assert!(!err.to_string().contains("no message or tool call"));
        assert!(
            err.to_string()
                .contains("neither tool calls nor recoverable output")
        );
    }

    #[test]
    fn rig_empty_provider_error_maps_to_empty_provider_turn() {
        let err = to_agent_error(
            CompletionError::ProviderError(
                "Response contained no message or tool call (empty)".to_string(),
            ),
            "Custom OpenAI-compatible API",
        );
        assert!(err.is_empty_provider_turn(), "got {err}");
    }

    #[tokio::test]
    async fn reqwest_timeout_names_the_transport_cause() {
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(100)))
            .mount(&server)
            .await;
        let error = reqwest::Client::builder()
            .timeout(Duration::from_millis(10))
            .build()
            .unwrap()
            .get(server.uri())
            .send()
            .await
            .unwrap_err();

        let mapped = http_client_error(HttpClientError::Instance(Box::new(error)), "Test");

        assert!(mapped.is_retryable());
        assert!(mapped.to_string().contains("HTTP transport timed out"));
    }
}
