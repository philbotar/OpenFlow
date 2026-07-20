//! `OpenAI` `prompt_cache_key` routing via rig `additional_params`.

use crate::spec::ProviderId;
use engine::AgentRequest;

/// Whether to emit `prompt_cache_key` for an OpenAI-compatible provider.
#[must_use]
pub fn openai_compat_cache_key_enabled(provider_id: &ProviderId) -> bool {
    !matches!(provider_id.as_str(), "ollama" | "lmstudio")
}

/// Steers `OpenAI` cache routing for all turns of one workflow node.
#[must_use]
pub fn cache_session_key(request: &AgentRequest) -> String {
    format!("{}:{}", request.workflow_id.0, request.node_id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{NodeId, WorkflowId};

    fn request() -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf-1".to_string()),
            node_id: NodeId("idea".to_string()),
            node_label: "Idea".to_string(),
            model: "test".to_string(),
            system_messages: vec!["sys".to_string()],
            task_prompt: "task".to_string(),
            input: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: engine::AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
        }
    }

    #[test]
    fn cache_session_key_joins_workflow_and_node() {
        assert_eq!(cache_session_key(&request()), "wf-1:idea");
    }

    #[test]
    fn openai_compat_cache_key_enabled_skips_local_hosts() {
        assert!(openai_compat_cache_key_enabled(&ProviderId::from("openai")));
        assert!(!openai_compat_cache_key_enabled(&ProviderId::from(
            "ollama"
        )));
        assert!(!openai_compat_cache_key_enabled(&ProviderId::from(
            "lmstudio"
        )));
    }
}
