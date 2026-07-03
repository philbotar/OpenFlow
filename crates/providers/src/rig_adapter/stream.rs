//! Drains a rig streaming response into an [`AiStreamSink`] and a final outcome.

use crate::rig_adapter::{error, outcome};
use engine::{AgentError, AgentTurnOutcome, AiStreamEvent, AiStreamSink};
use futures::StreamExt;
use rig_core::completion::GetTokenUsage;
use rig_core::message::Reasoning;
use rig_core::streaming::{StreamedAssistantContent, StreamingCompletionResponse};

pub async fn drain<R>(
    mut stream: StreamingCompletionResponse<R>,
    sink: &dyn AiStreamSink,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError>
where
    R: Clone + Unpin + GetTokenUsage + Send + 'static,
{
    while let Some(item) = stream.next().await {
        match item.map_err(|e| error::to_agent_error(e, provider_label))? {
            StreamedAssistantContent::Text(text) if !text.text.is_empty() => {
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

    let choice: Vec<_> = stream.choice.into_iter().collect();
    let usage = stream
        .response
        .as_ref()
        .map(GetTokenUsage::token_usage)
        .unwrap_or_default();

    outcome::resolve_outcome(choice, usage, provider_label, output_schema)
}

fn emit_reasoning(sink: &dyn AiStreamSink, reasoning: &Reasoning) {
    let text = reasoning.display_text();
    if !text.is_empty() {
        sink.on_stream_event(AiStreamEvent::ThinkingDelta { content: text });
    }
}
