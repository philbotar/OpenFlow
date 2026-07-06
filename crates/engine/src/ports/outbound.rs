//! Outbound ports owned by the engine.

use crate::conversation::{filter_tool_turn_assistant_message, AgentTranscriptItem};
use crate::graph::{NodeId, WorkflowId};
use crate::tools::{
    FileChangeRecord, NodeToolConfig, ReadRecord, ToolCall, ToolDefinition, ToolResult,
};
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    pub workflow_id: WorkflowId,
    pub node_id: NodeId,
    pub node_label: String,
    pub model: String,
    /// Ordered system instruction bodies assembled by the engine; providers map to wire format as-is.
    pub system_messages: Vec<String>,
    pub task_prompt: String,
    pub input: Value,
    pub output_schema: Value,
    pub tool_config: NodeToolConfig,
    pub available_tools: Vec<ToolDefinition>,
    pub transcript: Vec<AgentTranscriptItem>,
    /// 1-based model invocation attempt for this node (retries increment).
    pub model_attempt: u8,
    /// Opaque reasoning effort level forwarded to the provider.
    pub reasoning_effort: Option<String>,
    /// Optional reasoning budget token count forwarded to the provider.
    pub reasoning_budget_tokens: Option<u32>,
    /// Whether this node may pause for human input. When false, providers must
    /// not offer the request-input tool nor convert plain-text turns into
    /// input requests.
    pub allow_user_input: bool,
}

impl AgentRequest {
    /// Join [`Self::system_messages`] for providers that accept a single system string.
    #[must_use]
    pub fn system_content(&self) -> String {
        self.system_messages.join("\n\n")
    }
}

/// Token usage report extracted from LLM provider responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageReport {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnSuccess {
    pub output: Value,
    pub raw_text: String,
    pub assistant_message: Option<String>,
    pub usage: Option<UsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolCallBatch {
    pub raw_text: String,
    pub assistant_message: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<UsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNeedUserInput {
    pub raw_text: String,
    pub assistant_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTurnOutcome {
    Completed(AgentTurnSuccess),
    ToolCalls(AgentToolCallBatch),
    NeedsUserInput(AgentNeedUserInput),
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("transient: {0}")]
    Transient(String),
    #[error("permanent: {0}")]
    Permanent(String),
    #[error("{0}")]
    Failed(String),
    #[error("{provider_label} final output tool arguments were not valid JSON: {detail}")]
    MalformedSubmitOutput {
        provider_label: String,
        detail: String,
    },
    #[error("interrupted")]
    Interrupted,
}

impl AgentError {
    #[must_use]
    pub fn malformed_submit_output(
        provider_label: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::MalformedSubmitOutput {
            provider_label: provider_label.into(),
            detail: detail.into(),
        }
    }

    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient(_))
    }

    #[must_use]
    pub const fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted)
    }

    #[must_use]
    pub const fn is_malformed_submit_output(&self) -> bool {
        matches!(self, Self::MalformedSubmitOutput { .. })
    }
}

/// Streaming event emitted during [`AiPort::invoke_stream`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiStreamEvent {
    AssistantDelta { content: String },
    ThinkingDelta { content: String },
}

/// Receives streaming events from provider adapters during an AI invocation.
pub trait AiStreamSink: Send + Sync {
    fn on_stream_event(&self, event: AiStreamEvent);
}

#[async_trait]
pub trait AiPort: Send + Sync {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError>;

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let outcome = self.invoke(request).await?;
        emit_assistant_deltas_from_outcome(sink, &outcome);
        Ok(outcome)
    }
}

/// Emit a single assistant delta from a completed turn (fallback when streaming is unavailable).
pub fn emit_assistant_deltas_from_outcome(sink: &dyn AiStreamSink, outcome: &AgentTurnOutcome) {
    let message = match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(batch) => batch.assistant_message.clone(),
        AgentTurnOutcome::NeedsUserInput(need) => Some(need.assistant_message.clone()),
    };
    let message = filter_tool_turn_assistant_message(message);
    if let Some(content) = message.filter(|value| !value.trim().is_empty()) {
        sink.on_stream_event(AiStreamEvent::AssistantDelta { content });
    }
}

#[async_trait]
impl<T> AiPort for Box<T>
where
    T: AiPort + ?Sized,
{
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        (**self).invoke(request).await
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        (**self).invoke_stream(request, sink).await
    }
}

/// Side effects a tool batch produced; the engine applies these when the batch returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolBatchEffects {
    pub file_changes: Vec<FileChangeRecord>,
    pub reads: Vec<ReadRecord>,
    /// Local paths passed to `read` calls, for redundant-read accounting.
    pub read_call_paths: Vec<String>,
    /// The node was interrupted mid-batch; remaining calls did not run.
    pub interrupted: bool,
}

/// Result of executing one tool batch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolBatchOutput {
    /// One result per completed call, in order. May be shorter than the
    /// input when the batch was interrupted or the run was cancelled.
    pub results: Vec<ToolResult>,
    pub effects: ToolBatchEffects,
}

#[async_trait]
pub trait ToolPort: Send + Sync {
    /// Execute a batch of tool calls (including subagent calls) for a given node.
    async fn execute_batch(
        &self,
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> ToolBatchOutput;

    /// Augment an AI request's available tool descriptions before each AI invocation.
    fn augment_request(&self, node_id: &NodeId, request: &mut AgentRequest);
}

#[async_trait]
impl<T> ToolPort for Box<T>
where
    T: ToolPort + ?Sized,
{
    async fn execute_batch(
        &self,
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> ToolBatchOutput {
        (**self).execute_batch(node_id, label, calls).await
    }

    fn augment_request(&self, node_id: &NodeId, request: &mut AgentRequest) {
        (**self).augment_request(node_id, request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_error_retryable_classification() {
        assert!(AgentError::Transient("timeout".to_string()).is_retryable());
        assert!(!AgentError::Permanent("auth".to_string()).is_retryable());
        assert!(!AgentError::Failed("unknown".to_string()).is_retryable());
        assert!(!AgentError::Interrupted.is_retryable());
        assert!(AgentError::Interrupted.is_interrupted());
        assert!(
            AgentError::malformed_submit_output("AI provider", "missing field `output`")
                .is_malformed_submit_output()
        );
        assert!(!AgentError::Failed("unrelated".to_string()).is_malformed_submit_output());
    }
}
