//! Drains a rig streaming response into an [`AiStreamSink`] and a final outcome.

use crate::mapping::NoToolCallsPolicy;
use crate::rig_adapter::{error, outcome};
use engine::{AgentError, AgentTurnOutcome, AiStreamEvent, AiStreamSink};
use futures::StreamExt;
use rig_core::completion::GetTokenUsage;
use rig_core::message::Reasoning;
use rig_core::streaming::{StreamedAssistantContent, StreamingCompletionResponse};

use rig_core::message::AssistantContent;

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
    while let Some(item) = stream.next().await {
        match item.map_err(|e| error::to_agent_error(e, provider_label))? {
            StreamedAssistantContent::Text(text) if !text.text.is_empty() => {
                streamed_assistant_text.push_str(&text.text);
                sink.on_stream_event(AiStreamEvent::AssistantDelta { content: text.text });
            }
            StreamedAssistantContent::Reasoning(reasoning) => {
                emit_reasoning(sink, &reasoning);
            }
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } if !reasoning.is_empty() => {
                sink.on_stream_event(AiStreamEvent::ThinkingDelta { content: reasoning });
            }
            StreamedAssistantContent::Text(_)
            | StreamedAssistantContent::ReasoningDelta { .. }
            | StreamedAssistantContent::ToolCall { .. }
            | StreamedAssistantContent::ToolCallDelta { .. }
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

fn emit_reasoning(sink: &dyn AiStreamSink, reasoning: &Reasoning) {
    let text = reasoning.display_text();
    if !text.is_empty() {
        sink.on_stream_event(AiStreamEvent::ThinkingDelta { content: text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use rig_core::message::ReasoningContent;
    use rig_core::streaming::{RawStreamingChoice, RawStreamingToolCall, StreamingResult};

    struct NoopSink;

    impl AiStreamSink for NoopSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
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
}
