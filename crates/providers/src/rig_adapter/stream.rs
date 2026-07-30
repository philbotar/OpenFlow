//! Drains a rig streaming response into an [`AiStreamSink`] and a final outcome.

use crate::mapping::NoToolCallsPolicy;
use crate::rig_adapter::{error, outcome};
use engine::{AgentError, AgentTurnOutcome, AiStreamEvent, AiStreamSink};
use futures::StreamExt;
use rig_core::completion::GetTokenUsage;
use rig_core::message::Reasoning;
use rig_core::streaming::{
    StreamedAssistantContent, StreamingCompletionResponse, ToolCallDeltaContent,
};
use std::collections::BTreeMap;

use rig_core::message::AssistantContent;

#[derive(Default)]
struct PartialToolCallBuilder {
    tool_call_id: String,
    tool_name: Option<String>,
    raw_arguments: String,
}

pub async fn drain<R>(
    mut stream: StreamingCompletionResponse<R>,
    sink: &dyn AiStreamSink,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
    no_tool_calls: NoToolCallsPolicy,
) -> Result<AgentTurnOutcome, AgentError>
where
    R: Clone + Unpin + GetTokenUsage + Send + 'static,
{
    let mut streamed_assistant_text = String::new();
    let mut streamed_reasoning_text = String::new();
    let mut partial_tool_calls = BTreeMap::<String, PartialToolCallBuilder>::new();
    while let Some(item) = stream.next().await {
        let item = match item {
            Ok(item) => item,
            Err(error) if error::is_output_limit(&error) => {
                let partial_tool_calls = partial_tool_calls
                    .into_iter()
                    .map(|(internal_call_id, partial)| {
                        engine::PartialToolCall::new(
                            partial.tool_call_id,
                            internal_call_id,
                            partial.tool_name,
                            partial.raw_arguments,
                        )
                    })
                    .collect();
                return Err(AgentError::output_truncated(
                    provider_label,
                    "output token limit",
                    partial_tool_calls,
                ));
            }
            Err(error) => return Err(error::to_agent_error(error, provider_label)),
        };
        match item {
            StreamedAssistantContent::Text(text) if !text.text.is_empty() => {
                streamed_assistant_text.push_str(&text.text);
                sink.on_stream_event(AiStreamEvent::AssistantDelta { content: text.text });
            }
            StreamedAssistantContent::Reasoning(reasoning) => {
                emit_reasoning(sink, &reasoning, &mut streamed_reasoning_text);
            }
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } if !reasoning.is_empty() => {
                streamed_reasoning_text.push_str(&reasoning);
                sink.on_stream_event(AiStreamEvent::ThinkingDelta { content: reasoning });
            }
            StreamedAssistantContent::ToolCallDelta {
                id,
                internal_call_id,
                content,
            } => {
                let partial = partial_tool_calls.entry(internal_call_id).or_default();
                if !id.is_empty() {
                    partial.tool_call_id = id;
                }
                match content {
                    ToolCallDeltaContent::Name(name) => partial.tool_name = Some(name),
                    ToolCallDeltaContent::Delta(arguments) => {
                        partial.raw_arguments.push_str(&arguments);
                    }
                }
            }
            StreamedAssistantContent::ToolCall {
                internal_call_id, ..
            } => {
                partial_tool_calls.remove(&internal_call_id);
            }
            StreamedAssistantContent::Text(_)
            | StreamedAssistantContent::ReasoningDelta { .. }
            | StreamedAssistantContent::Final(_) => {}
        }
    }

    let mut choice: Vec<_> = stream.choice.into_iter().collect();
    if !streamed_assistant_text.is_empty() {
        let (text_parts, _reasoning, tool_calls) = outcome::partition_choice(choice.clone());
        if tool_calls.is_empty() && text_parts.is_empty() {
            choice.push(AssistantContent::text(streamed_assistant_text));
        }
    }
    let usage = stream
        .response
        .as_ref()
        .map(GetTokenUsage::token_usage)
        .unwrap_or_default();

    outcome::resolve_outcome(choice, usage, provider_label, output_schema, no_tool_calls)
}

fn emit_reasoning(
    sink: &dyn AiStreamSink,
    reasoning: &Reasoning,
    streamed_reasoning_text: &mut String,
) {
    let text = reasoning.display_text();
    // Rig can replay an accumulated reasoning delta as the final typed block.
    if text.is_empty() || streamed_reasoning_text.ends_with(&text) {
        return;
    }
    let content = text
        .strip_prefix(streamed_reasoning_text.as_str())
        .unwrap_or(&text)
        .to_string();
    streamed_reasoning_text.push_str(&content);
    sink.on_stream_event(AiStreamEvent::ThinkingDelta { content });
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use rig_core::completion::CompletionError;
    use rig_core::message::ReasoningContent;
    use rig_core::streaming::{
        RawStreamingChoice, RawStreamingToolCall, StreamingResult, ToolCallDeltaContent,
    };

    struct NoopSink;

    impl AiStreamSink for NoopSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    #[derive(Default)]
    struct RecordingSink(std::sync::Mutex<Vec<AiStreamEvent>>);

    impl AiStreamSink for RecordingSink {
        fn on_stream_event(&self, event: AiStreamEvent) {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(event);
        }
    }

    #[tokio::test]
    async fn drain_emits_streamed_reasoning_once_when_final_summary_matches() {
        let chunks = vec![
            Ok(RawStreamingChoice::ReasoningDelta {
                id: None,
                reasoning: "checking the request".to_string(),
            }),
            Ok(RawStreamingChoice::Reasoning {
                id: Some("reasoning-1".to_string()),
                content: ReasoningContent::Summary("checking the request".to_string()),
            }),
            Ok(RawStreamingChoice::ToolCall(RawStreamingToolCall::new(
                "call-1".to_string(),
                "search".to_string(),
                serde_json::json!({"query": "OpenFlow"}),
            ))),
        ];
        let raw: StreamingResult<()> = Box::pin(stream::iter(chunks));
        let sink = RecordingSink::default();

        let _ = drain(
            StreamingCompletionResponse::stream(raw),
            &sink,
            "OpenAI Codex",
            None,
            NoToolCallsPolicy::Recover {
                error: "expected tool call",
            },
        )
        .await;

        assert_eq!(
            *sink
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            vec![AiStreamEvent::ThinkingDelta {
                content: "checking the request".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn drain_replaces_unsigned_reasoning_delta_with_signed_final_block() {
        let chunks = vec![
            Ok(RawStreamingChoice::ReasoningDelta {
                id: None,
                reasoning: "checking the request".to_string(),
            }),
            Ok(RawStreamingChoice::Reasoning {
                id: None,
                content: ReasoningContent::Text {
                    text: "checking the request".to_string(),
                    signature: Some("bedrock-signature".to_string()),
                },
            }),
            Ok(RawStreamingChoice::ToolCall(RawStreamingToolCall::new(
                "call-1".to_string(),
                "search".to_string(),
                serde_json::json!({"query": "OpenFlow"}),
            ))),
        ];
        let raw: StreamingResult<()> = Box::pin(stream::iter(chunks));
        let outcome = drain(
            StreamingCompletionResponse::stream(raw),
            &NoopSink,
            "Amazon Bedrock",
            None,
            NoToolCallsPolicy::Recover {
                error: "expected tool call",
            },
        )
        .await;

        let reasoning = match outcome {
            Ok(AgentTurnOutcome::ToolCalls(batch)) => batch.reasoning,
            Ok(_) | Err(_) => Vec::new(),
        };
        assert_eq!(reasoning.len(), 1);
        assert!(matches!(
            reasoning[0].content.as_slice(),
            [engine::AgentReasoningContent::Text {
                text,
                signature: Some(signature),
            }] if text == "checking the request" && signature == "bedrock-signature"
        ));
    }

    #[tokio::test]
    #[allow(
        clippy::expect_used,
        reason = "test asserts the exact recoverable error payload"
    )]
    async fn drain_preserves_partial_tool_call_when_output_limit_ends_stream() {
        let chunks = vec![
            Ok(RawStreamingChoice::ToolCallDelta {
                id: "call-1".to_string(),
                internal_call_id: "internal-1".to_string(),
                content: ToolCallDeltaContent::Name("write".to_string()),
            }),
            Ok(RawStreamingChoice::ToolCallDelta {
                id: "call-1".to_string(),
                internal_call_id: "internal-1".to_string(),
                content: ToolCallDeltaContent::Delta(
                    r#"{"path":"docs/large.md","content":"partial"#.to_string(),
                ),
            }),
            Err(CompletionError::ProviderError(
                "OpenAI response stream was incomplete: max_output_tokens".to_string(),
            )),
        ];
        let raw: StreamingResult<()> = Box::pin(stream::iter(chunks));

        let error = drain(
            StreamingCompletionResponse::stream(raw),
            &NoopSink,
            "OpenAI Codex",
            None,
            NoToolCallsPolicy::Recover {
                error: "expected tool call",
            },
        )
        .await
        .expect_err("output cutoff must remain recoverable");

        assert!(error.is_output_truncated());
        let partial_calls = error
            .partial_tool_calls()
            .expect("truncation must retain partial tool calls");
        assert_eq!(partial_calls.len(), 1);
        assert_eq!(partial_calls[0].tool_name(), Some("write"));
        assert_eq!(partial_calls[0].arguments_len(), 42);
        assert!(!format!("{error:?}").contains("docs/large.md"));
    }
}
