use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::ports::outbound::AgentRequest;
use crate::tools::{ApprovalMode, NodeToolConfig};
use crate::AgentNodeConfig;
use crate::NodeId;

/// Mid-run overrides for per-node tool approval and reasoning settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeRuntimeConfigPatch {
    pub approval_mode: Option<ApprovalMode>,
    pub reasoning_effort: Option<Option<String>>,
    pub reasoning_budget_tokens: Option<Option<u32>>,
}

impl NodeRuntimeConfigPatch {
    pub fn merge_into(&self, target: &mut Self) {
        if self.approval_mode.is_some() {
            target.approval_mode = self.approval_mode;
        }
        if self.reasoning_effort.is_some() {
            target.reasoning_effort = self.reasoning_effort.clone();
        }
        if self.reasoning_budget_tokens.is_some() {
            target.reasoning_budget_tokens = self.reasoning_budget_tokens;
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
    patch: NodeRuntimeConfigPatch,
) {
    let mut guard = store
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = guard.entry(node_id).or_default();
    patch.merge_into(entry);
}

pub fn runtime_patch_for(store: &NodeRuntimeConfigStore, node_id: &NodeId) -> Option<NodeRuntimeConfigPatch> {
    store
        .read()
        .ok()
        .and_then(|guard| guard.get(node_id).cloned())
}

pub fn apply_runtime_patch_to_tool_config(config: &mut NodeToolConfig, patch: &NodeRuntimeConfigPatch) {
    if let Some(mode) = patch.approval_mode {
        config.approval_mode = Some(mode);
    }
}

pub fn apply_runtime_patch_to_agent(agent: &mut AgentNodeConfig, patch: &NodeRuntimeConfigPatch) {
    apply_runtime_patch_to_tool_config(&mut agent.tools, patch);
    if let Some(effort) = &patch.reasoning_effort {
        agent.reasoning_effort = effort.clone();
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
    if let Some(effort) = &patch.reasoning_effort {
        request.reasoning_effort = effort.clone();
        if effort.is_none() {
            request.reasoning_budget_tokens = None;
        }
    }
    if let Some(budget) = patch.reasoning_budget_tokens {
        request.reasoning_budget_tokens = budget;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_merges_and_applies_to_request() {
        let store = new_runtime_config_store();
        upsert_runtime_patch(
            &store,
            NodeId("idea".to_string()),
            NodeRuntimeConfigPatch {
                approval_mode: Some(ApprovalMode::ReadOnly),
                reasoning_effort: Some(Some("high".to_string())),
                reasoning_budget_tokens: None,
            },
        );
        let patch = runtime_patch_for(&store, &NodeId("idea".to_string())).expect("patch");
        let mut request = AgentRequest {
            workflow_id: "wf".into(),
            node_id: NodeId("idea".to_string()),
            node_label: "idea".into(),
            model: "gpt".into(),
            system_messages: vec![],
            task_prompt: String::new(),
            input: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            tool_config: NodeToolConfig::default(),
            available_tools: vec![],
            transcript: vec![],
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            allow_user_input: true,
        };
        apply_runtime_patch_to_request(&mut request, &patch);
        assert_eq!(
            request.tool_config.approval_mode,
            Some(ApprovalMode::ReadOnly)
        );
        assert_eq!(request.reasoning_effort, Some("high".to_string()));
    }
}
