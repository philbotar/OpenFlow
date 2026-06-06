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

#[test]
fn mock_factory_creates_with_custom_response() {
    let factory = MockProviderFactory::with_response("custom response");
    let config = AiClientConfig::openai("test-key");
    let ai = factory.create(config);

    // Verify the created AiPort works with the custom response
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let request = AgentRequest {
            workflow_id: "wf-1".into(),
            node_id: "node-1".into(),
            node_label: "Test Node".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: "You are a test".to_string(),
            task_prompt: "Do something".to_string(),
            input: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            tool_config: domain::NodeToolConfig::default(),
            available_tools: vec![],
            transcript: vec![],
        };

        let result = ai.invoke(request).await.unwrap();
        match result {
            AgentTurnOutcome::Completed(success) => {
                assert_eq!(success.raw_text, "custom response");
            }
            _ => panic!("Expected Completed outcome"),
        }
    });
}
#[tokio::test]
async fn mock_ai_port_invoke_returns_success() {
    let ai = MockAiPort::default();
    let request = AgentRequest {
        workflow_id: "wf-1".into(),
        node_id: "node-1".into(),
        node_label: "Test Node".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: "You are a test".to_string(),
        task_prompt: "Do something".to_string(),
        input: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        tool_config: domain::NodeToolConfig::default(),
        available_tools: vec![],
        transcript: vec![],
    };

    let result = ai.invoke(request).await.unwrap();

    match result {
        AgentTurnOutcome::Completed(success) => {
            assert_eq!(success.raw_text, "mock response");
            assert_eq!(success.assistant_message, Some("mock response".to_string()));
        }
        _ => panic!("Expected Completed outcome"),
    }
}

#[tokio::test]
async fn mock_ai_port_invoke_returns_custom_response() {
    let ai = MockAiPort {
        response: "custom response".to_string(),
    };
    let request = AgentRequest {
        workflow_id: "wf-1".into(),
        node_id: "node-1".into(),
        node_label: "Test Node".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: "You are a test".to_string(),
        task_prompt: "Do something".to_string(),
        input: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        tool_config: domain::NodeToolConfig::default(),
        available_tools: vec![],
        transcript: vec![],
    };

    let result = ai.invoke(request).await.unwrap();

    match result {
        AgentTurnOutcome::Completed(success) => {
            assert_eq!(success.raw_text, "custom response");
        }
        _ => panic!("Expected Completed outcome"),
    }
}

#[tokio::test]
async fn error_mock_ai_port_invoke_returns_error() {
    let ai = ErrorMockAiPort {
        error_message: "test error".to_string(),
    };
    let request = AgentRequest {
        workflow_id: "wf-1".into(),
        node_id: "node-1".into(),
        node_label: "Test Node".to_string(),
        model: "gpt-4".to_string(),
        system_prompt: "You are a test".to_string(),
        task_prompt: "Do something".to_string(),
        input: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        tool_config: domain::NodeToolConfig::default(),
        available_tools: vec![],
        transcript: vec![],
    };

    let result = ai.invoke(request).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentError::Failed(msg) => assert_eq!(msg, "test error"),
    }
}

#[test]
fn mock_factory_default_has_empty_response() {
    let factory = MockProviderFactory::default();
    assert!(factory.response.is_empty());
}

#[test]
fn mock_factory_with_response_overrides() {
    let factory = MockProviderFactory::with_response("override");
    assert_eq!(factory.response, "override");
}
