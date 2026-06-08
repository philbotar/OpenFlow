//! Outbound ports owned by the domain.

use crate::{AgentTranscriptItem, NodeId, NodeToolConfig, ToolCall, ToolDefinition, WorkflowId};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_error_retryable_classification() {
        assert!(AgentError::Transient("timeout".to_string()).is_retryable());
        assert!(!AgentError::Permanent("auth".to_string()).is_retryable());
        assert!(!AgentError::Failed("unknown".to_string()).is_retryable());
    }
}
