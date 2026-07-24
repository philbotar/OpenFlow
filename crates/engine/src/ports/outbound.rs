//! Outbound ports owned by the engine.

use crate::conversation::{
    filter_tool_turn_assistant_message, AgentReasoning, AgentTranscriptItem,
};
use crate::graph::{NodeId, WorkflowId};
use crate::tools::{
    FileChangeRecord, NodeToolConfig, ReadRecord, ToolCall, ToolDefinition, ToolResult,
};
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

/// Run-scoped capability policy selected by the engine.
///
/// This is distinct from node approval configuration. Planning is a hard
/// capability boundary, not a request to ask before a write.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAccessPolicy {
    #[default]
    Execution,
    Planning,
}

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
    /// Hard capability policy for this run phase.
    pub tool_access_policy: ToolAccessPolicy,
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
    pub reasoning: Vec<AgentReasoning>,
    pub usage: Option<UsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolCallBatch {
    pub raw_text: String,
    pub assistant_message: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Vec<AgentReasoning>,
    pub usage: Option<UsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNeedUserInput {
    pub raw_text: String,
    pub assistant_message: String,
    pub reasoning: Vec<AgentReasoning>,
}

/// A provider turn containing human-readable assistant text but no structured action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMessageTurn {
    pub raw_text: String,
    pub assistant_message: String,
    pub reasoning: Vec<AgentReasoning>,
    pub usage: Option<UsageReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTurnOutcome {
    Completed(AgentTurnSuccess),
    ToolCalls(AgentToolCallBatch),
    NeedsUserInput(AgentNeedUserInput),
    Message(AgentMessageTurn),
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
        /// Redacted repair payload for overseer recovery; omitted from [`std::fmt::Display`].
        candidate: Option<Box<OutputRepairCandidate>>,
    },
    #[error("{provider_label} response mixed incompatible tools in one batch: {tool_names}")]
    MixedToolTurn {
        provider_label: String,
        tool_names: String,
    },
    #[error("interrupted")]
    Interrupted,
}

/// Why a final-output submit call failed deterministic recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputRepairFailureKind {
    InvalidJson,
    WrongEnvelope,
    SchemaViolation,
    TruncatedResponse,
}

/// In-memory repair candidate for a malformed `openflow_submit_node_output` call.
///
/// [`Debug`] redacts raw arguments to a byte count. Never log [`Self::raw_arguments`].
#[derive(Clone, PartialEq, Eq)]
pub struct OutputRepairCandidate {
    pub tool_call_id: Option<String>,
    pub tool_name: String,
    pub(crate) raw_arguments: String,
    pub detail: String,
    pub output_schema: Value,
    pub failure_kind: OutputRepairFailureKind,
    pub usage: Option<UsageReport>,
    pub finish_reason: Option<String>,
}

impl OutputRepairCandidate {
    #[must_use]
    pub fn raw_arguments(&self) -> &str {
        &self.raw_arguments
    }

    /// Eligible for a bounded overseer repair pass (slice 3).
    #[must_use]
    pub const fn is_repairable(&self) -> bool {
        !matches!(
            self.failure_kind,
            OutputRepairFailureKind::TruncatedResponse
        ) && self.raw_arguments.len() <= 64 * 1024
    }
}

impl std::fmt::Debug for OutputRepairCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputRepairCandidate")
            .field("tool_call_id", &self.tool_call_id)
            .field("tool_name", &self.tool_name)
            .field("raw_arguments_len", &self.raw_arguments.len())
            .field("detail", &self.detail)
            .field("output_schema", &self.output_schema)
            .field("failure_kind", &self.failure_kind)
            .field("usage", &self.usage)
            .field("finish_reason", &self.finish_reason)
            .finish()
    }
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
            candidate: None,
        }
    }

    #[must_use]
    pub fn malformed_submit_with_candidate(
        provider_label: impl Into<String>,
        detail: impl Into<String>,
        candidate: OutputRepairCandidate,
    ) -> Self {
        Self::MalformedSubmitOutput {
            provider_label: provider_label.into(),
            detail: detail.into(),
            candidate: Some(Box::new(candidate)),
        }
    }

    #[must_use]
    pub fn mixed_tool_turn(
        provider_label: impl Into<String>,
        tool_names: impl Into<String>,
    ) -> Self {
        Self::MixedToolTurn {
            provider_label: provider_label.into(),
            tool_names: tool_names.into(),
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

    #[must_use]
    pub fn output_repair_candidate(&self) -> Option<&OutputRepairCandidate> {
        match self {
            Self::MalformedSubmitOutput { candidate, .. } => candidate.as_deref(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_repairable_submit_output(&self) -> bool {
        self.output_repair_candidate()
            .is_some_and(OutputRepairCandidate::is_repairable)
    }

    #[must_use]
    pub const fn is_mixed_tool_turn(&self) -> bool {
        matches!(self, Self::MixedToolTurn { .. })
    }

    #[must_use]
    pub fn mixed_tool_names(&self) -> Option<&str> {
        match self {
            Self::MixedToolTurn { tool_names, .. } => Some(tool_names),
            _ => None,
        }
    }

    /// Provider turn had no tool calls and no recoverable assistant text.
    #[must_use]
    pub fn is_empty_provider_turn(&self) -> bool {
        match self {
            Self::Failed(message) => {
                message.contains("neither tool calls nor recoverable output")
                    || message.contains("no tool calls and no usable text")
                    // Rig rejects empty choices before OpenFlow outcome mapping.
                    || message.contains("no message or tool call")
            }
            _ => false,
        }
    }
}

/// Streaming event emitted during [`AiPort::invoke_stream`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiStreamEvent {
    AssistantDelta {
        content: String,
    },
    ThinkingDelta {
        content: String,
    },
    /// Overseer repair started for a malformed final-output submit (no raw content).
    OutputRepairStarted {
        node_id: NodeId,
        model: String,
    },
    /// Overseer repair produced a completion-protocol-accepted candidate.
    OutputRepairSucceeded {
        node_id: NodeId,
        model: String,
    },
    /// Overseer repair failed; sanitized reason only.
    OutputRepairFailed {
        node_id: NodeId,
        reason: String,
    },
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

/// Emit displayable reasoning followed by assistant text when streaming is unavailable.
pub fn emit_assistant_deltas_from_outcome(sink: &dyn AiStreamSink, outcome: &AgentTurnOutcome) {
    let reasoning = match outcome {
        AgentTurnOutcome::Completed(success) => &success.reasoning,
        AgentTurnOutcome::ToolCalls(batch) => &batch.reasoning,
        AgentTurnOutcome::NeedsUserInput(need) => &need.reasoning,
        AgentTurnOutcome::Message(message) => &message.reasoning,
    };
    for block in reasoning {
        for content in &block.content {
            let display = match content {
                crate::conversation::AgentReasoningContent::Text { text, .. }
                | crate::conversation::AgentReasoningContent::Summary(text) => Some(text),
                crate::conversation::AgentReasoningContent::Encrypted(_)
                | crate::conversation::AgentReasoningContent::Redacted { .. } => None,
            };
            if let Some(content) = display.filter(|value| !value.is_empty()) {
                sink.on_stream_event(AiStreamEvent::ThinkingDelta {
                    content: content.clone(),
                });
            }
        }
    }

    let message = match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(batch) => batch.assistant_message.clone(),
        AgentTurnOutcome::NeedsUserInput(need) => Some(need.assistant_message.clone()),
        AgentTurnOutcome::Message(message) => Some(message.assistant_message.clone()),
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
        policy: ToolAccessPolicy,
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
        policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        (**self).execute_batch(node_id, label, calls, policy).await
    }

    fn augment_request(&self, node_id: &NodeId, request: &mut AgentRequest) {
        (**self).augment_request(node_id, request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::AgentReasoningContent;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingSink(Mutex<Vec<AiStreamEvent>>);

    impl AiStreamSink for RecordingSink {
        fn on_stream_event(&self, event: AiStreamEvent) {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(event);
        }
    }

    #[test]
    fn fallback_stream_emits_reasoning_before_assistant_text() {
        let outcome = AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "Done.".to_string(),
            assistant_message: Some("Done.".to_string()),
            tool_calls: Vec::new(),
            reasoning: vec![AgentReasoning {
                id: None,
                content: vec![
                    AgentReasoningContent::Text {
                        text: "Inspecting inputs. ".to_string(),
                        signature: None,
                    },
                    AgentReasoningContent::Summary("Choosing the next action.".to_string()),
                    AgentReasoningContent::Encrypted("opaque".to_string()),
                ],
            }],
            usage: None,
        });
        let sink = RecordingSink::default();

        emit_assistant_deltas_from_outcome(&sink, &outcome);

        assert_eq!(
            *sink
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            vec![
                AiStreamEvent::ThinkingDelta {
                    content: "Inspecting inputs. ".to_string(),
                },
                AiStreamEvent::ThinkingDelta {
                    content: "Choosing the next action.".to_string(),
                },
                AiStreamEvent::AssistantDelta {
                    content: "Done.".to_string(),
                },
            ]
        );
    }

    #[test]
    fn agent_error_retryable_classification() {
        assert!(AgentError::Transient("timeout".to_string()).is_retryable());
        assert!(!AgentError::Permanent("auth".to_string()).is_retryable());
        assert!(!AgentError::Failed("unknown".to_string()).is_retryable());
        assert!(!AgentError::Interrupted.is_retryable());
        assert!(AgentError::Failed(
            "provider returned neither tool calls nor recoverable output".to_string()
        )
        .is_empty_provider_turn());
        assert!(AgentError::Failed(
            "Custom OpenAI-compatible API model `mimo` returned no tool calls and no usable text."
                .to_string()
        )
        .is_empty_provider_turn());
        assert!(AgentError::Failed(
            "Custom OpenAI-compatible API response error: Response contained no message or tool call (empty)"
                .to_string()
        )
        .is_empty_provider_turn());
        assert!(AgentError::Interrupted.is_interrupted());
        assert!(
            AgentError::malformed_submit_output("AI provider", "missing field `output`")
                .is_malformed_submit_output()
        );
        let mixed = AgentError::mixed_tool_turn(
            "Custom OpenAI-compatible API",
            "openflow_submit_node_output, write",
        );
        assert!(mixed.is_mixed_tool_turn());
        assert_eq!(
            mixed.mixed_tool_names(),
            Some("openflow_submit_node_output, write")
        );
        assert!(!AgentError::Failed("unrelated".to_string()).is_malformed_submit_output());
    }
}
