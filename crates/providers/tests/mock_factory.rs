//! Integration tests for provider `AiPort` mocks.

use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AgentTurnSuccess, AiPort};

// ── Mock AiPort ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MockAiPort {
    pub response: String,
}

impl Default for MockAiPort {
    fn default() -> Self {
        Self {
            response: "mock response".to_string(),
        }
    }
}

#[async_trait]
impl AiPort for MockAiPort {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: serde_json::json!({"result": self.response}),
            raw_text: self.response.clone(),
            assistant_message: Some(self.response.clone()),
        }))
    }
}

// ── Error Mock AiPort ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ErrorMockAiPort {
    pub error_message: String,
}

#[async_trait]
impl AiPort for ErrorMockAiPort {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        Err(AgentError::Failed(self.error_message.clone()))
    }
}

// ── Tests ──────────────────────────────────────────────────────

fn sample_request() -> AgentRequest {
    AgentRequest {
        workflow_id: "wf-1".into(),
        node_id: "node-1".into(),
        node_label: "Agent".to_string(),
        model: "mock".to_string(),
        system_prompt: String::new(),
        task_prompt: String::new(),
        input: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        tool_config: engine::NodeToolConfig::default(),
        available_tools: Vec::new(),
        transcript: Vec::new(),
    }
}

#[tokio::test]
async fn mock_ai_port_returns_completed_outcome() {
    let ai = MockAiPort::default();
    let outcome = ai.invoke(sample_request()).await;

    assert!(matches!(
        outcome,
        Ok(AgentTurnOutcome::Completed(ref success)) if success.raw_text == "mock response"
    ));
}

#[tokio::test]
async fn error_mock_ai_port_returns_failed_error() {
    let ai = ErrorMockAiPort {
        error_message: "provider down".to_string(),
    };
    let outcome = ai.invoke(sample_request()).await;

    assert!(matches!(
        outcome,
        Err(AgentError::Failed(ref message)) if message == "provider down"
    ));
}
