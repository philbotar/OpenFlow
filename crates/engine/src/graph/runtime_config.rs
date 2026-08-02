use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::ports::outbound::AgentRequest;
use crate::tools::{ApprovalMode, NodeToolConfig};
use crate::AgentNodeConfig;
use crate::NodeId;

/// Mid-run overrides for per-node model, tool approval, and reasoning settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeRuntimeConfigPatch {
    pub model: Option<String>,
    pub approval_mode: Option<ApprovalMode>,
    pub reasoning_effort: Option<Option<String>>,
    pub reasoning_budget_tokens: Option<Option<u32>>,
    pub fast_mode: Option<bool>,
}

impl NodeRuntimeConfigPatch {
    pub fn merge_into(&self, target: &mut Self) {
        if self.model.is_some() {
            target.model.clone_from(&self.model);
        }
        if self.approval_mode.is_some() {
            target.approval_mode = self.approval_mode;
        }
        if self.reasoning_effort.is_some() {
            target.reasoning_effort.clone_from(&self.reasoning_effort);
        }
        if self.reasoning_budget_tokens.is_some() {
            target.reasoning_budget_tokens = self.reasoning_budget_tokens;
        }
        if self.fast_mode.is_some() {
            target.fast_mode = self.fast_mode;
        }
    }
}

pub type NodeRuntimeConfigStore = Arc<RwLock<BTreeMap<NodeId, NodeRuntimeConfigPatch>>>;

#[must_use]
pub fn new_runtime_config_store() -> NodeRuntimeConfigStore {
    Arc::new(RwLock::new(BTreeMap::new()))
}

pub fn upsert_runtime_patch(
    store: &NodeRuntimeConfigStore,
    node_id: NodeId,
    patch: &NodeRuntimeConfigPatch,
) {
    patch.merge_into(
        store
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(node_id)
            .or_default(),
    );
}

pub fn runtime_patch_for(
    store: &NodeRuntimeConfigStore,
    node_id: &NodeId,
) -> Option<NodeRuntimeConfigPatch> {
    store
        .read()
        .ok()
        .and_then(|guard| guard.get(node_id).cloned())
}

pub const fn apply_runtime_patch_to_tool_config(
    config: &mut NodeToolConfig,
    patch: &NodeRuntimeConfigPatch,
) {
    if let Some(mode) = patch.approval_mode {
        config.approval_mode = Some(mode);
    }
}

pub fn apply_runtime_patch_to_agent(agent: &mut AgentNodeConfig, patch: &NodeRuntimeConfigPatch) {
    apply_runtime_patch_to_tool_config(&mut agent.tools, patch);
    if let Some(model) = &patch.model {
        agent.model.clone_from(model);
    }
    if let Some(effort) = &patch.reasoning_effort {
        agent.reasoning_effort.clone_from(effort);
        if effort.is_none() {
            agent.reasoning_budget_tokens = None;
        }
    }
    if let Some(budget) = patch.reasoning_budget_tokens {
        agent.reasoning_budget_tokens = budget;
    }
}

pub fn apply_runtime_patch_to_request(request: &mut AgentRequest, patch: &NodeRuntimeConfigPatch) {
    apply_runtime_patch_to_tool_config(&mut request.tool_config, patch);
    if let Some(model) = &patch.model {
        request.model.clone_from(model);
    }
    if let Some(effort) = &patch.reasoning_effort {
        request.reasoning_effort.clone_from(effort);
        if effort.is_none() {
            request.reasoning_budget_tokens = None;
        }
    }
    if let Some(budget) = patch.reasoning_budget_tokens {
        request.reasoning_budget_tokens = budget;
    }
    if let Some(fast_mode) = patch.fast_mode {
        request.fast_mode = fast_mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::unwrap_used)]
    #[test]
    fn patch_merges_and_applies_to_request() {
        let store = new_runtime_config_store();
        let patch_value = NodeRuntimeConfigPatch {
            model: Some("gpt-next".to_string()),
            approval_mode: Some(ApprovalMode::ReadOnly),
            reasoning_effort: Some(Some("high".to_string())),
            reasoning_budget_tokens: None,
            fast_mode: Some(true),
        };
        upsert_runtime_patch(&store, NodeId("idea".to_string()), &patch_value);
        let patch = runtime_patch_for(&store, &NodeId("idea".to_string()));
        assert_eq!(
            patch.as_ref().and_then(|value| value.approval_mode),
            Some(ApprovalMode::ReadOnly)
        );
        let patch = patch.unwrap();
        let mut request = AgentRequest {
            workflow_id: "wf".into(),
            node_id: NodeId("idea".to_string()),
            node_label: "idea".into(),
            model: "gpt".into(),
            provider_id: None,
            max_output_tokens: None,
            system_messages: vec![],
            task_prompt: String::new(),
            input: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            tool_config: NodeToolConfig::default(),
            available_tools: vec![],
            transcript: vec![],
            entrypoint_attachments: Vec::new(),
            resolved_attachments: std::collections::BTreeMap::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            fast_mode: false,
            tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
            allow_user_input: true,
            conversation_mode: false,
        };
        apply_runtime_patch_to_request(&mut request, &patch);
        assert_eq!(
            request.tool_config.approval_mode,
            Some(ApprovalMode::ReadOnly)
        );
        assert_eq!(request.model, "gpt-next");
        assert_eq!(request.reasoning_effort, Some("high".to_string()));
        assert!(request.fast_mode);
    }
}
