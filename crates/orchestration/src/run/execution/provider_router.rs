use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink};
use std::collections::BTreeMap;

/// Run-scoped provider dispatch. Missing request overrides inherit the workflow provider.
pub(crate) struct ProviderRouter {
    default_provider_id: String,
    providers: BTreeMap<String, Box<dyn AiPort>>,
}

impl ProviderRouter {
    #[must_use]
    pub(crate) fn new(
        default_provider_id: impl Into<String>,
        providers: BTreeMap<String, Box<dyn AiPort>>,
    ) -> Self {
        Self {
            default_provider_id: default_provider_id.into(),
            providers,
        }
    }

    fn provider_for(&self, request: &AgentRequest) -> Result<&dyn AiPort, AgentError> {
        let provider_id = request
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|provider_id| !provider_id.is_empty())
            .unwrap_or(&self.default_provider_id);
        self.providers
            .get(provider_id)
            .map(Box::as_ref)
            .ok_or_else(|| {
                AgentError::Permanent(format!(
                    "provider {provider_id} is not configured for this workflow run"
                ))
            })
    }
}

#[async_trait]
impl AiPort for ProviderRouter {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.provider_for(&request)?.invoke(request).await
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        self.provider_for(&request)?
            .invoke_stream(request, sink)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderRouter;
    use async_trait::async_trait;
    use engine::{
        AgentError, AgentRequest, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamEvent,
        AiStreamSink, NodeToolConfig, ToolAccessPolicy,
    };
    use parking_lot::Mutex;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct RecordingAi {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl AiPort for RecordingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.calls.lock().push(request.node_id.to_string());
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: json!({"ok": true}),
                raw_text: "{}".to_string(),
                assistant_message: None,
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    struct FailingAi;

    #[async_trait]
    impl AiPort for FailingAi {
        async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Permanent("provider unavailable".to_string()))
        }
    }

    struct NoopSink;

    impl AiStreamSink for NoopSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    fn request(node_id: &str, provider_id: Option<&str>) -> AgentRequest {
        AgentRequest {
            workflow_id: "wf".into(),
            node_id: node_id.into(),
            node_label: node_id.to_string(),
            model: "model".to_string(),
            provider_id: provider_id.map(ToString::to_string),
            max_output_tokens: None,
            system_messages: Vec::new(),
            task_prompt: String::new(),
            input: json!(null),
            output_schema: json!({}),
            tool_config: NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            entrypoint_attachments: Vec::new(),
            resolved_attachments: BTreeMap::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            fast_mode: false,
            tool_access_policy: ToolAccessPolicy::Execution,
            allow_user_input: false,
            conversation_mode: false,
        }
    }

    #[tokio::test]
    async fn routes_node_override_and_inherits_shared_provider() {
        let openai_calls = Arc::new(Mutex::new(Vec::new()));
        let anthropic_calls = Arc::new(Mutex::new(Vec::new()));
        let mut providers: BTreeMap<String, Box<dyn AiPort>> = BTreeMap::new();
        providers.insert(
            "openai".to_string(),
            Box::new(RecordingAi {
                calls: Arc::clone(&openai_calls),
            }),
        );
        providers.insert(
            "anthropic".to_string(),
            Box::new(RecordingAi {
                calls: Arc::clone(&anthropic_calls),
            }),
        );
        let router = ProviderRouter::new("openai", providers);

        router
            .invoke_stream(request("implementation", None), &NoopSink)
            .await
            .expect("shared provider request");
        router
            .invoke_stream(request("planning", Some("anthropic")), &NoopSink)
            .await
            .expect("node provider request");

        assert_eq!(*openai_calls.lock(), vec!["implementation"]);
        assert_eq!(*anthropic_calls.lock(), vec!["planning"]);
    }

    #[tokio::test]
    async fn provider_failure_does_not_fall_back_to_shared_provider() {
        let openai_calls = Arc::new(Mutex::new(Vec::new()));
        let mut providers: BTreeMap<String, Box<dyn AiPort>> = BTreeMap::new();
        providers.insert(
            "openai".to_string(),
            Box::new(RecordingAi {
                calls: Arc::clone(&openai_calls),
            }),
        );
        providers.insert("anthropic".to_string(), Box::new(FailingAi));
        let router = ProviderRouter::new("openai", providers);

        let error = router
            .invoke(request("planning", Some("anthropic")))
            .await
            .expect_err("node provider error must surface");

        assert!(matches!(error, AgentError::Permanent(_)));
        assert!(openai_calls.lock().is_empty());
    }
}
