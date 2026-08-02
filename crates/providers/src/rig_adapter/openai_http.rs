//! Final-wire compatibility for `OpenAI` request params, usage, and tool arguments.

use crate::mapping::{malformed_tool_args_marker_value, parse_or_recover_tool_arguments};
use crate::model_debug;
use crate::prompt_cache::{
    pack_openai_cache_usage, OPENAI_RESPONSES_CACHE_KEY_METADATA,
    OPENAI_RESPONSES_CACHE_MODE_METADATA,
};
use bytes::Bytes;
use futures::StreamExt;
use rig_core::http_client::{
    HttpClientExt, LazyBody, MultipartForm, Request, Response, Result, StreamingResponse,
};
use serde_json::Value;
use std::future::Future;

/// Wrap `reqwest` so `OpenAI` Chat/Responses requests and responses preserve
/// fields not yet modeled by Rig.
#[derive(Clone, Debug)]
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) struct OpenAiHttpClient {
    inner: reqwest::Client,
    debug_output: bool,
    provider_label: String,
}

impl Default for OpenAiHttpClient {
    fn default() -> Self {
        Self {
            inner: reqwest::Client::new(),
            debug_output: false,
            provider_label: String::new(),
        }
    }
}

impl OpenAiHttpClient {
    pub(crate) fn new(
        inner: reqwest::Client,
        debug_output: bool,
        provider_label: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            debug_output,
            provider_label: provider_label.into(),
        }
    }
}

impl HttpClientExt for OpenAiHttpClient {
    fn send<T, U>(
        &self,
        mut request: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        T: Into<Bytes> + Send,
        U: From<Bytes> + Send + 'static,
    {
        strip_empty_bearer_auth(&mut request);
        let request =
            relax_openai_responses_tools(promote_openai_prompt_cache(request.map(Into::into)));
        let response = self.inner.send::<Bytes, Bytes>(request);
        let debug_output = self.debug_output;
        let provider_label = self.provider_label.clone();
        async move {
            let response = response.await?;
            let (parts, body) = response.into_parts();
            let status = parts.status.as_u16();
            let normalized: LazyBody<U> = Box::pin(async move {
                let body = body.await?;
                model_debug::log_model_response(debug_output, &provider_label, status, &body);
                Ok(U::from(normalize_openai_response(body)))
            });
            Ok(Response::from_parts(parts, normalized))
        }
    }

    fn send_multipart<U>(
        &self,
        mut request: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        U: From<Bytes> + Send + 'static,
    {
        strip_empty_bearer_auth(&mut request);
        self.inner.send_multipart(request)
    }

    fn send_streaming<T>(
        &self,
        mut request: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + Send
    where
        T: Into<Bytes> + Send,
    {
        // Streaming recovery of SSE argument fragments is deferred (slice 2 scope).
        strip_empty_bearer_auth(&mut request);
        let response =
            self.inner
                .send_streaming(relax_openai_responses_tools(promote_openai_prompt_cache(
                    request.map(Into::into),
                )));
        async move { response.await.map(normalize_openai_streaming_response) }
    }
}

fn strip_empty_bearer_auth<T>(request: &mut Request<T>) {
    let empty_bearer = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "Bearer");
    if empty_bearer {
        request.headers_mut().remove("authorization");
    }
}

fn relax_openai_responses_tools(mut request: Request<Bytes>) -> Request<Bytes> {
    let original = request.body().clone();
    let Ok(mut value) = serde_json::from_slice::<Value>(&original) else {
        return request;
    };
    if !crate::rig_adapter::openai_tool_schema::relax_incompatible_responses_tools(&mut value) {
        return request;
    }
    *request.body_mut() = serde_json::to_vec(&value).map_or(original, Bytes::from);
    request
}

/// Promote `OpenFlow`'s namespaced metadata carrier into Responses API fields.
///
/// `Rig 0.39` deserializes `additional_params` into a closed struct, dropping
/// newer fields such as `prompt_cache_key` and `prompt_cache_options`.
#[must_use]
fn promote_openai_prompt_cache(mut request: Request<Bytes>) -> Request<Bytes> {
    let original = request.body().clone();
    let Ok(mut value) = serde_json::from_slice::<Value>(&original) else {
        return request;
    };
    let Some(root) = value.as_object_mut() else {
        return request;
    };

    let (cache_key, explicit, metadata_is_empty) = {
        let Some(metadata) = root.get_mut("metadata").and_then(Value::as_object_mut) else {
            return request;
        };
        let cache_key = metadata
            .remove(OPENAI_RESPONSES_CACHE_KEY_METADATA)
            .and_then(|value| value.as_str().map(ToOwned::to_owned));
        let explicit = metadata
            .remove(OPENAI_RESPONSES_CACHE_MODE_METADATA)
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .is_some_and(|mode| mode == "explicit");
        (cache_key, explicit, metadata.is_empty())
    };

    let Some(cache_key) = cache_key else {
        return request;
    };
    if metadata_is_empty {
        root.remove("metadata");
    }
    root.insert("prompt_cache_key".into(), Value::String(cache_key));

    if explicit && mark_responses_stable_system_prefix(root) {
        root.insert(
            "prompt_cache_options".into(),
            serde_json::json!({ "mode": "explicit" }),
        );
    }

    *request.body_mut() = serde_json::to_vec(&value).map_or(original, Bytes::from);
    request
}

fn mark_responses_stable_system_prefix(root: &mut serde_json::Map<String, Value>) -> bool {
    if let Some(input) = root.get_mut("input").and_then(Value::as_array_mut) {
        for item in input.iter_mut().rev() {
            if item.get("role").and_then(Value::as_str) != Some("system") {
                continue;
            }
            let Some(content) = item.get_mut("content").and_then(Value::as_array_mut) else {
                continue;
            };
            let Some(block) = content.iter_mut().rev().find(|block| {
                matches!(
                    block.get("type").and_then(Value::as_str),
                    Some("input_text" | "input_image" | "input_file")
                )
            }) else {
                continue;
            };
            let Some(block) = block.as_object_mut() else {
                continue;
            };
            block.insert(
                "prompt_cache_breakpoint".into(),
                serde_json::json!({ "mode": "explicit" }),
            );
            return true;
        }
    }

    let Some(instructions) = root
        .get("instructions")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return false;
    };
    let Some(input) = root.get_mut("input").and_then(Value::as_array_mut) else {
        return false;
    };
    input.insert(
        0,
        serde_json::json!({
            "type": "message",
            "role": "system",
            "content": [{
                "type": "input_text",
                "text": instructions,
                "prompt_cache_breakpoint": { "mode": "explicit" }
            }]
        }),
    );
    root.remove("instructions");
    true
}

fn normalize_openai_streaming_response(response: StreamingResponse) -> StreamingResponse {
    let (parts, body) = response.into_parts();
    let stream = futures::stream::unfold(
        (body, Vec::<u8>::new(), false),
        |(mut body, mut buffer, mut finished)| async move {
            loop {
                if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
                    let tail = buffer.split_off(newline + 1);
                    let line = std::mem::replace(&mut buffer, tail);
                    return Some((
                        Ok(normalize_openai_sse_line(line)),
                        (body, buffer, finished),
                    ));
                }
                if finished {
                    if buffer.is_empty() {
                        return None;
                    }
                    let line = std::mem::take(&mut buffer);
                    return Some((
                        Ok(normalize_openai_sse_line(line)),
                        (body, buffer, finished),
                    ));
                }
                match body.next().await {
                    Some(Ok(chunk)) => buffer.extend_from_slice(&chunk),
                    Some(Err(error)) => {
                        return Some((Err(error), (body, buffer, finished)));
                    }
                    None => finished = true,
                }
            }
        },
    );
    Response::from_parts(parts, Box::pin(stream))
}

fn normalize_openai_sse_line(line: Vec<u8>) -> Bytes {
    let payload_end = line
        .iter()
        .rposition(|byte| !matches!(*byte, b'\r' | b'\n'))
        .map_or(0, |at| at + 1);
    let Some(data_prefix) = line[..payload_end].strip_prefix(b"data:") else {
        return Bytes::from(line);
    };
    let leading_space = usize::from(data_prefix.first() == Some(&b' '));
    let payload_start = "data:".len() + leading_space;
    let Ok(mut value) = serde_json::from_slice::<Value>(&line[payload_start..payload_end]) else {
        return Bytes::from(line);
    };
    if !pack_openai_cache_usage_in_response(&mut value) {
        return Bytes::from(line);
    }

    let Ok(json) = serde_json::to_vec(&value) else {
        return Bytes::from(line);
    };
    let mut normalized = Vec::with_capacity(line.len());
    normalized.extend_from_slice(&line[..payload_start]);
    normalized.extend_from_slice(&json);
    normalized.extend_from_slice(&line[payload_end..]);
    Bytes::from(normalized)
}

fn pack_openai_cache_usage_in_response(value: &mut Value) -> bool {
    let usage = if value.get("usage").is_some() {
        value.get_mut("usage")
    } else {
        value.pointer_mut("/response/usage")
    };
    let Some(usage) = usage.and_then(Value::as_object_mut) else {
        return false;
    };
    let details = if usage.contains_key("input_tokens_details") {
        usage.get_mut("input_tokens_details")
    } else {
        usage.get_mut("prompt_tokens_details")
    };
    let Some(details) = details.and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(write_tokens) = details.get("cache_write_tokens").and_then(Value::as_u64) else {
        return false;
    };
    let read_tokens = details
        .get("cached_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    details.insert(
        "cached_tokens".into(),
        Value::from(pack_openai_cache_usage(read_tokens, write_tokens)),
    );
    true
}

/// Normalize Chat Completions and Responses API bodies in place.
#[must_use]
pub(super) fn normalize_openai_response(body: Bytes) -> Bytes {
    let Ok(mut value) = serde_json::from_slice::<Value>(&body) else {
        // Unrelated response JSON failures stay generic provider errors.
        return body;
    };

    // All normalizers must run; `|` avoids short-circuit.
    let changed = pack_openai_cache_usage_in_response(&mut value)
        | normalize_chat_completions_arguments(&mut value)
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
