use engine::AgentError;
use futures::Stream;
use futures::StreamExt;
use reqwest::Response;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::time::timeout;

#[cfg(test)]
const IDLE_CHUNK_TIMEOUT: Duration = Duration::from_millis(50);
#[cfg(not(test))]
const IDLE_CHUNK_TIMEOUT: Duration = Duration::from_secs(90);

pub async fn stream_sse_data_lines<F>(
    response: Response,
    label: &str,
    on_data: F,
) -> Result<(), AgentError>
where
    F: FnMut(Value) -> Result<(), AgentError>,
{
    let status = response.status();
    if !status.is_success() {
        return stream_sse_http_error(response, label).await;
    }
    stream_sse_data_lines_from(response.bytes_stream(), label, on_data).await
}

async fn stream_sse_data_lines_from<S, F>(
    mut stream: S,
    label: &str,
    mut on_data: F,
) -> Result<(), AgentError>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    F: FnMut(Value) -> Result<(), AgentError>,
{
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let chunk = match timeout(IDLE_CHUNK_TIMEOUT, stream.next()).await {
            Ok(Some(chunk)) => chunk.map_err(|error| {
                AgentError::Transient(format!("{label} stream read failed: {error}"))
            })?,
            Ok(None) => break,
            Err(_) => {
                return Err(AgentError::Transient(format!(
                    "{label} stream stalled: no data for {}s",
                    IDLE_CHUNK_TIMEOUT.as_secs()
                )));
            }
        };
        buffer.extend_from_slice(&chunk);
        while let Some(pos) = buffer.iter().position(|&byte| byte == b'\n') {
            let mut line_bytes = buffer.drain(..=pos).collect::<Vec<_>>();
            while line_bytes
                .last()
                .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
            {
                line_bytes.pop();
            }
            let line = std::str::from_utf8(&line_bytes).map_err(|error| {
                AgentError::Failed(format!("{label} stream UTF-8 failed: {error}"))
            })?;
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                let payload: Value = serde_json::from_str(data).map_err(|error| {
                    AgentError::Failed(format!("{label} stream JSON failed: {error}"))
                })?;
                on_data(payload)?;
            }
        }
    }
    Ok(())
}

async fn stream_sse_http_error(response: Response, label: &str) -> Result<(), AgentError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<unavailable>".to_string());
    let message = format!("{label} returned HTTP {status}: {body}");
    if status.as_u16() == 429 || status.is_server_error() {
        Err(AgentError::Transient(message))
    } else {
        Err(AgentError::Permanent(message))
    }
}

#[derive(Debug, Default)]
pub struct ChatCompletionStreamAggregator {
    pub content: String,
    pub reasoning: String,
    tool_calls: BTreeMap<usize, StreamingToolCall>,
}

#[derive(Debug, Default)]
struct StreamingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl ChatCompletionStreamAggregator {
    pub fn apply_chunk(&mut self, chunk: &Value) {
        let Some(delta) = chunk
            .pointer("/choices/0/delta")
            .or_else(|| chunk.pointer("/choices/0/message"))
        else {
            return;
        };
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                self.content.push_str(text);
            }
        }
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta.get(key).and_then(Value::as_str) {
                if !text.is_empty() {
                    self.reasoning.push_str(text);
                }
            }
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0);
                let entry = self.tool_calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    entry.id = Some(id.to_string());
                }
                if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                    entry.name = Some(name.to_string());
                }
                if let Some(args) = call.pointer("/function/arguments").and_then(Value::as_str) {
                    entry.arguments.push_str(args);
                }
            }
        }
    }

    pub fn into_completion_payload(self) -> Value {
        let mut message = serde_json::json!({ "role": "assistant" });
        if !self.content.is_empty() {
            message["content"] = Value::String(self.content);
        }
        if !self.tool_calls.is_empty() {
            let tool_calls: Vec<Value> = self
                .tool_calls
                .into_iter()
                .map(|(index, call)| {
                    serde_json::json!({
                        "id": call.id.unwrap_or_else(|| format!("call-{index}")),
                        "type": "function",
                        "function": {
                            "name": call.name.unwrap_or_default(),
                            "arguments": call.arguments,
                        }
                    })
                })
                .collect();
            message["tool_calls"] = Value::Array(tool_calls);
        }
        serde_json::json!({
            "choices": [{
                "message": message,
                "finish_reason": if message.get("tool_calls").is_some() { "tool_calls" } else { "stop" }
            }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    struct YieldOnceThenStall {
        yielded: bool,
    }

    impl Stream for YieldOnceThenStall {
        type Item = Result<Bytes, reqwest::Error>;

        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if self.yielded {
                Poll::Pending
            } else {
                self.yielded = true;
                Poll::Ready(Some(Ok(Bytes::from(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
                ))))
            }
        }
    }

    #[test]
    fn utf8_line_decoding_preserves_multibyte_characters() {
        let event = serde_json::json!({
            "choices": [{ "delta": { "content": "é" } }]
        });
        let mut aggregator = ChatCompletionStreamAggregator::default();
        aggregator.apply_chunk(&event);
        assert_eq!(aggregator.content, "é");
    }

    #[test]
    fn aggregator_accumulates_reasoning_content_deltas() {
        let mut aggregator = ChatCompletionStreamAggregator::default();
        aggregator.apply_chunk(&serde_json::json!({
            "choices": [{ "delta": { "reasoning_content": "Step 1. " } }]
        }));
        aggregator.apply_chunk(&serde_json::json!({
            "choices": [{ "delta": { "reasoning": "Step 2." } }]
        }));
        assert_eq!(aggregator.reasoning, "Step 1. Step 2.");
    }

    #[tokio::test]
    async fn idle_timeout_returns_transient_error_when_stream_stalls() {
        let stream = YieldOnceThenStall { yielded: false };
        let result = stream_sse_data_lines_from(stream, "test", |_| Ok(())).await;
        assert!(matches!(
            result,
            Err(AgentError::Transient(ref message)) if message.contains("stream stalled")
        ));
    }

    #[tokio::test]
    async fn completes_when_stream_ends_after_chunk() {
        let stream = stream::iter(vec![Ok(Bytes::from(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n",
        ))]);
        assert!(stream_sse_data_lines_from(stream, "test", |_| Ok(()))
            .await
            .is_ok());
    }
}
