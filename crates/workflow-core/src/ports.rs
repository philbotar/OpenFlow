use crate::{ChatMessage, NodeId, WorkflowId};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResponse {
    pub output: Value,
    pub raw_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationAgentRequest {
    pub workflow_id: WorkflowId,
    pub node_id: NodeId,
    pub node_label: String,
    pub model: String,
    pub system_prompt: String,
    pub task_prompt: String,
    pub input: Value,
    pub output_schema: Value,
    pub conversation: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationAgentResponse {
    pub ready_to_advance: bool,
    pub assistant_message: Option<String>,
    pub output: Option<Value>,
    pub raw_text: String,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("{0}")]
    Failed(String),
}

#[async_trait]
pub trait AiPort: Send + Sync {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;

    async fn invoke_conversation(
        &self,
        request: ConversationAgentRequest,
    ) -> Result<ConversationAgentResponse, AgentError>;
}
