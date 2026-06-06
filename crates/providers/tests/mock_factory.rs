//! Integration tests for providers port traits using mock implementations.
//!
//! These tests verify the ProviderFactoryPort contract in isolation by
//! providing a mock factory that returns canned AiPort implementations.

use async_trait::async_trait;
use domain::{AgentError, AgentRequest, AgentTurnOutcome, AgentTurnSuccess, AiPort};

use providers::ports::inbound::{BoxedAiPort, ProviderFactoryPort};
use providers::AiClientConfig;

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

// ── Mock ProviderFactory ───────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MockProviderFactory {
    pub response: String,
}

impl MockProviderFactory {
    pub fn with_response(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

impl ProviderFactoryPort for MockProviderFactory {
    fn create(&self, _config: AiClientConfig) -> BoxedAiPort {
        Box::new(MockAiPort {
            response: self.response.clone(),
        })
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

// ── Factory Tests ──────────────────────────────────────────────

#[test]
fn mock_factory_creates_ai_port() {
    let factory = MockProviderFactory::default();
    let config = AiClientConfig::openai("test-key");
    let _ai = factory.create(config);
    // Factory successfully creates a boxed AiPort
}
