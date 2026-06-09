//! Outbound ports owned by the engine.

use crate::conversation::AgentTranscriptItem;
use crate::graph::{NodeId, WorkflowId};
use crate::tools::{NodeToolConfig, ToolCall, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    pub workflow_id: WorkflowId,
    pub node_id: NodeId,
    pub node_label: String,
    pub model: String,
    pub system_prompt: String,
    pub task_prompt: String,
    pub input: Value,
    pub output_schema: Value,
    pub tool_config: NodeToolConfig,
    pub available_tools: Vec<ToolDefinition>,
    pub transcript: Vec<AgentTranscriptItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnSuccess {
    pub output: Value,
    pub raw_text: String,
    pub assistant_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolCallBatch {
    pub raw_text: String,
    pub assistant_message: Option<String>,
    pub tool_calls: Vec<ToolCall>,
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
}

impl AgentError {
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient(_))
    }

    #[must_use]
    pub fn is_malformed_submit_output(&self) -> bool {
        matches!(
            self,
            Self::Failed(message)
                if message.contains("final output tool arguments were not valid JSON")
        )
    }
}

#[async_trait]
pub trait AiPort: Send + Sync {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError>;
}

#[async_trait]
impl<T> AiPort for Box<T>
where
    T: AiPort + ?Sized,
{
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        (**self).invoke(request).await
    }
}

#[async_trait]
pub trait ToolPort: Send + Sync {
    /// Execute a batch of tool calls (including subagent calls) for a given node.
    /// Returns one [`ToolResult`] per input call, in order.
    async fn execute_batch(
        &self,
        engine: &mut crate::execution::InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> Vec<ToolResult>;

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
        engine: &mut crate::execution::InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> Vec<ToolResult> {
        (**self).execute_batch(engine, node_id, label, calls).await
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
        assert!(AgentError::Failed(
            "OpenAI-compatible final output tool arguments were not valid JSON: missing field `output`"
                .to_string()
        )
        .is_malformed_submit_output());
    }
}
