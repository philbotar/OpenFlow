use engine::AgentError;
use futures::StreamExt;
use reqwest::Response;
use serde_json::Value;
use std::collections::BTreeMap;

pub async fn stream_sse_data_lines<F>(
    response: Response,
    label: &str,
    mut on_data: F,
) -> Result<(), AgentError>
where
    F: FnMut(Value) -> Result<(), AgentError>,
{
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unavailable>".to_string());
        let message = format!("{label} returned HTTP {status}: {body}");
        return if status.as_u16() == 429 || status.is_server_error() {
            Err(AgentError::Transient(message))
        } else {
            Err(AgentError::Permanent(message))
        };
    }

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|error| AgentError::Transient(format!("{label} stream read failed: {error}")))?;
        buffer.extend_from_slice(&chunk);
        while let Some(pos) = buffer.iter().position(|&byte| byte == b'\n') {
            let mut line_bytes = buffer.drain(..=pos).collect::<Vec<_>>();
            while line_bytes.last().is_some_and(|byte| *byte == b'\n' || *byte == b'\r') {
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

#[derive(Debug, Default)]
pub struct ChatCompletionStreamAggregator {
    pub content: String,
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
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                let entry = self.tool_calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    entry.id = Some(id.to_string());
                }
                if let Some(name) = call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                {
                    entry.name = Some(name.to_string());
                }
                if let Some(args) = call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                {
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

    #[test]
    fn utf8_line_decoding_preserves_multibyte_characters() {
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"é\"}}]}\n";
        let line = payload
            .trim_end_matches('\n')
            .strip_prefix("data: ")
            .expect("data line");
        let event: Value = serde_json::from_str(line).expect("json");
        let mut aggregator = ChatCompletionStreamAggregator::default();
        aggregator.apply_chunk(&event);
        assert_eq!(aggregator.content, "é");
    }
}
