//! Provider `prompt-cache` wire helpers (Anthropic `cache_control`, `OpenAI` `prompt_cache_key`).

use crate::spec::ProviderId;
use engine::AgentRequest;
use serde_json::{json, Value};

/// Anthropic explicit `cache_control` breakpoints.
#[must_use]
pub fn ephemeral_cache_control() -> Value {
    json!({ "type": "ephemeral" })
}

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

/// Index of the second-to-last message for Anthropic BP2, when applicable.
#[must_use]
pub const fn second_to_last_index(message_count: usize) -> Option<usize> {
    if message_count >= 2 {
        Some(message_count - 2)
    } else {
        None
    }
}

/// Merge `prompt_cache_key` into an OpenAI-compatible request body when enabled.
pub fn apply_openai_cache_key(body: &mut Value, request: &AgentRequest, enabled: bool) {
    if enabled {
        body["prompt_cache_key"] = Value::String(cache_session_key(request));
    }
}

/// Attach `cache_control` to the last content block in a message object.
pub fn apply_cache_control_to_message(message: &mut Value) {
    let Some(content) = message.get_mut("content") else {
        return;
    };
    match content {
        Value::String(text) => {
            *content = json!([{
                "type": "text",
                "text": text,
                "cache_control": ephemeral_cache_control(),
            }]);
        }
        Value::Array(blocks) => {
            if let Some(last) = blocks.last_mut() {
                if let Some(obj) = last.as_object_mut() {
                    obj.insert("cache_control".to_string(), ephemeral_cache_control());
                }
            }
        }
        _ => {}
    }
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
        }
    }

    #[test]
    fn cache_session_key_joins_workflow_and_node() {
        assert_eq!(cache_session_key(&request()), "wf-1:idea");
    }

    #[test]
    fn second_to_last_index_requires_at_least_two_messages() {
        assert_eq!(second_to_last_index(0), None);
        assert_eq!(second_to_last_index(1), None);
        assert_eq!(second_to_last_index(2), Some(0));
        assert_eq!(second_to_last_index(3), Some(1));
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

    #[test]
    fn apply_openai_cache_key_sets_field_when_enabled() {
        let mut body = json!({ "model": "gpt-4o" });
        apply_openai_cache_key(&mut body, &request(), true);
        assert_eq!(body["prompt_cache_key"], "wf-1:idea");

        let mut skipped = json!({ "model": "gpt-4o" });
        apply_openai_cache_key(&mut skipped, &request(), false);
        assert!(skipped.get("prompt_cache_key").is_none());
    }
}
