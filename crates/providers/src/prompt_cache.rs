//! Provider prompt-cache capability and stable cache routing.

#![allow(
    clippy::redundant_pub_crate,
    reason = "cache helpers form an intentional cross-sibling crate API"
)]

use crate::spec::ProviderId;
use engine::AgentRequest;

/// Reserved Responses API metadata carrier. Rig 0.39 drops unknown top-level
/// `additional_params`, so the final HTTP adapter promotes this value.
pub(super) const OPENAI_RESPONSES_CACHE_KEY_METADATA: &str = "__openflow_prompt_cache_key";
/// Reserved carrier requesting GPT-5.6 explicit cache-breakpoint mode.
pub(super) const OPENAI_RESPONSES_CACHE_MODE_METADATA: &str = "__openflow_prompt_cache_mode";
const PACKED_CACHE_USAGE_FLAG: u64 = 1 << 63;
const PACKED_CACHE_USAGE_MASK: u64 = (1 << 31) - 1;

/// Whether to emit `prompt_cache_key` for an OpenAI-compatible provider.
#[must_use]
pub fn openai_compat_cache_key_enabled(provider_id: &ProviderId) -> bool {
    !matches!(provider_id.as_str(), "ollama" | "lmstudio")
}

/// Whether `OpenAI` requires an explicit stable-prefix breakpoint to avoid
/// billable cache writes for a changing latest user/tool suffix.
#[must_use]
pub fn openai_explicit_prompt_cache_supported(model: &str) -> bool {
    model == "gpt-5.6" || model.starts_with("gpt-5.6-")
}

/// Preserve `OpenAI`'s cache read/write split through `Rig 0.39`'s single
/// `cached_input_tokens` field. Provider token counts cannot approach bit 63.
#[must_use]
pub(super) fn pack_openai_cache_usage(read_tokens: u64, write_tokens: u64) -> u64 {
    PACKED_CACHE_USAGE_FLAG
        | (write_tokens.min(PACKED_CACHE_USAGE_MASK) << 31)
        | read_tokens.min(PACKED_CACHE_USAGE_MASK)
}

/// Decode the HTTP-adapter sentinel created by [`pack_openai_cache_usage`].
#[must_use]
pub(super) const fn unpack_openai_cache_usage(packed: u64) -> Option<(u64, u64)> {
    if packed & PACKED_CACHE_USAGE_FLAG == 0 {
        return None;
    }
    Some((
        packed & PACKED_CACHE_USAGE_MASK,
        (packed >> 31) & PACKED_CACHE_USAGE_MASK,
    ))
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
            provider_id: None,
            max_output_tokens: None,
            system_messages: vec!["sys".to_string()],
            task_prompt: "task".to_string(),
            input: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            entrypoint_attachments: Vec::new(),
            resolved_attachments: std::collections::BTreeMap::default(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            fast_mode: false,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
            conversation_mode: false,
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

    #[test]
    fn explicit_prompt_cache_is_scoped_to_gpt_5_6_family() {
        assert!(openai_explicit_prompt_cache_supported("gpt-5.6"));
        assert!(openai_explicit_prompt_cache_supported("gpt-5.6-sol"));
        assert!(openai_explicit_prompt_cache_supported("gpt-5.6-terra"));
        assert!(openai_explicit_prompt_cache_supported("gpt-5.6-luna"));
        assert!(!openai_explicit_prompt_cache_supported("gpt-5.5"));
        assert!(!openai_explicit_prompt_cache_supported("gpt-4.1"));
    }

    #[test]
    fn cache_usage_pack_round_trips_reads_and_writes() {
        let packed = pack_openai_cache_usage(1_024, 768);
        assert_eq!(unpack_openai_cache_usage(packed), Some((1_024, 768)));
        assert_eq!(unpack_openai_cache_usage(1_024), None);
    }
}
