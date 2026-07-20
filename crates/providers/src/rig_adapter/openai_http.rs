//! Compatibility HTTP client that preserves malformed `OpenAI` tool arguments for repair.

use crate::mapping::{malformed_tool_args_marker_value, parse_or_recover_tool_arguments};
use bytes::Bytes;
use rig_core::http_client::{
    HttpClientExt, LazyBody, MultipartForm, Request, Response, Result, StreamingResponse,
};
use serde_json::Value;
use std::future::Future;

/// Wrap `reqwest` so non-streaming `OpenAI` Chat/Responses bodies are normalized
/// before Rig deserializes stringified tool arguments into `serde_json::Value`.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenAiHttpClient {
    inner: reqwest::Client,
}

impl OpenAiHttpClient {
    pub(crate) const fn new(inner: reqwest::Client) -> Self {
        Self { inner }
    }
}

impl HttpClientExt for OpenAiHttpClient {
    fn send<T, U>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        T: Into<Bytes> + Send,
        U: From<Bytes> + Send + 'static,
    {
        let response = self.inner.send::<T, Bytes>(request);
        async move {
            let response = response.await?;
            let (parts, body) = response.into_parts();
            let normalized: LazyBody<U> = Box::pin(async move {
                let body = body.await?;
                Ok(U::from(normalize_openai_response(body)))
            });
            Ok(Response::from_parts(parts, normalized))
        }
    }

    fn send_multipart<U>(
        &self,
        request: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        U: From<Bytes> + Send + 'static,
    {
        self.inner.send_multipart(request)
    }

    fn send_streaming<T>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + Send
    where
        T: Into<Bytes> + Send,
    {
        // Streaming recovery of SSE argument fragments is deferred (slice 2 scope).
        self.inner.send_streaming(request)
    }
}

/// Normalize Chat Completions and Responses API bodies in place.
#[must_use]
pub(super) fn normalize_openai_response(body: Bytes) -> Bytes {
    let Ok(mut value) = serde_json::from_slice::<Value>(&body) else {
        // Unrelated response JSON failures stay generic provider errors.
        return body;
    };

    // Both normalizers must run; `|` avoids short-circuit so Responses still mutates after Chat.
    let changed = normalize_chat_completions_arguments(&mut value)
        | normalize_responses_arguments(&mut value);

    if !changed {
        return body;
    }
    serde_json::to_vec(&value).map_or(body, Bytes::from)
}

fn normalize_chat_completions_arguments(value: &mut Value) -> bool {
    let Some(choices) = value.get_mut("choices").and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    for choice in choices {
        let Some(tool_calls) = choice
            .pointer_mut("/message/tool_calls")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for call in tool_calls {
            if let Some(arguments) = call.pointer_mut("/function/arguments") {
                if normalize_arguments_field(arguments) {
                    changed = true;
                }
            }
        }
    }
    changed
}

fn normalize_responses_arguments(value: &mut Value) -> bool {
    let Some(output) = value.get_mut("output").and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            continue;
        }
        if let Some(arguments) = item.get_mut("arguments") {
            if normalize_arguments_field(arguments) {
                changed = true;
            }
        }
    }
    changed
}

/// Rewrite a Chat/Responses `arguments` field that arrives as a JSON string.
fn normalize_arguments_field(field: &mut Value) -> bool {
    let Value::String(raw) = field else {
        return false;
    };
    match parse_or_recover_tool_arguments(raw) {
        Ok(parsed) => {
            let repaired = parsed.to_string();
            if repaired == *raw {
                return false;
            }
            *raw = repaired;
            true
        }
        Err(detail) => {
            let marker = malformed_tool_args_marker_value(raw, &detail);
            *field = Value::String(marker.to_string());
            true
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "unit tests assert JSON shapes with expect/unwrap"
)]
mod tests {
    use super::*;
    use crate::mapping::{extract_malformed_tool_args_marker, MALFORMED_TOOL_ARGS_MARKER_KEY};
    use serde_json::json;

    #[test]
    fn chat_trailing_comma_repairs_without_marker() {
        let body = Bytes::from(
            json!({
                "choices": [{
                    "message": {
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "openflow_submit_node_output",
                                "arguments": r#"{"output":{"summary":"done"},"assistant_message":null,}"#
                            }
                        }]
                    }
                }]
            })
            .to_string(),
        );
        let normalized = normalize_openai_response(body);
        let value: Value = serde_json::from_slice(&normalized).unwrap();
        let args = value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap();
        let parsed: Value = serde_json::from_str(args).unwrap();
        assert!(extract_malformed_tool_args_marker(&parsed).is_none());
        assert_eq!(parsed["output"]["summary"], "done");
        assert!(!args.contains(MALFORMED_TOOL_ARGS_MARKER_KEY));
    }

    #[test]
    fn chat_unrecoverable_arguments_become_marker() {
        let secret = "SECRET_CHAT_RAW_ARGS";
        let body = Bytes::from(
            json!({
                "choices": [{
                    "message": {
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "openflow_submit_node_output",
                                "arguments": format!("not-json-{secret}")
                            }
                        }]
                    }
                }]
            })
            .to_string(),
        );
        let normalized = normalize_openai_response(body);
        let value: Value = serde_json::from_slice(&normalized).unwrap();
        let args = value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap();
        let parsed: Value = serde_json::from_str(args).unwrap();
        let marker = extract_malformed_tool_args_marker(&parsed).expect("marker");
        assert!(marker.raw.contains(secret));
        assert!(!args.is_empty());
    }

    #[test]
    fn responses_unrecoverable_arguments_become_marker() {
        let secret = "SECRET_RESPONSES_RAW";
        let body = Bytes::from(
            json!({
                "output": [{
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "openflow_submit_node_output",
                    "arguments": format!("{{{{broken {secret}")
                }]
            })
            .to_string(),
        );
        let normalized = normalize_openai_response(body);
        let value: Value = serde_json::from_slice(&normalized).unwrap();
        let args = value["output"][0]["arguments"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(args).unwrap();
        let marker = extract_malformed_tool_args_marker(&parsed).expect("marker");
        assert!(marker.raw.contains(secret));
    }

    #[test]
    fn malformed_outer_json_is_left_unchanged() {
        let body = Bytes::from("{not-valid-response");
        let normalized = normalize_openai_response(body.clone());
        assert_eq!(normalized, body);
    }
}
