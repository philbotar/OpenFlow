#![allow(clippy::must_use_candidate)]

use domain::{
    ApprovalMode, NodeId, NodeToolConfig, PendingToolApproval, ToolCall, ToolPolicy, ToolTier,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Prompt,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalRequest {
    pub approval_id: String,
    pub node_id: NodeId,
    pub node_label: String,
    pub tool_call: ToolCall,
    pub tier: ToolTier,
}

impl ToolApprovalRequest {
    #[must_use]
    pub fn to_pending(&self) -> PendingToolApproval {
        PendingToolApproval {
            approval_id: self.approval_id.clone(),
            node_id: self.node_id.to_string(),
            node_label: self.node_label.clone(),
            tool_call: self.tool_call.clone(),
            tier: self.tier,
        }
    }
}

pub fn resolve_tool_policy(
    config: &NodeToolConfig,
    tool_name: &str,
    tier: ToolTier,
    exec_granted: bool,
) -> ApprovalDecision {
    if let Some(override_policy) = config
        .overrides
        .iter()
        .find(|entry| entry.tool_name == tool_name)
        .map(|entry| entry.policy)
    {
        return match override_policy {
            ToolPolicy::Allow => ApprovalDecision::Allow,
            ToolPolicy::Prompt => ApprovalDecision::Prompt,
            ToolPolicy::Deny => ApprovalDecision::Deny,
        };
    }

    match tier {
        ToolTier::Read => match config.approval_mode.unwrap_or(ApprovalMode::Write) {
            ApprovalMode::AlwaysAsk => ApprovalDecision::Prompt,
            ApprovalMode::Write | ApprovalMode::Yolo => ApprovalDecision::Allow,
        },
        ToolTier::Write => match config.approval_mode.unwrap_or(ApprovalMode::Write) {
            ApprovalMode::AlwaysAsk | ApprovalMode::Write => ApprovalDecision::Prompt,
            ApprovalMode::Yolo => ApprovalDecision::Allow,
        },
        ToolTier::Exec => {
            if exec_granted {
                ApprovalDecision::Allow
            } else {
                ApprovalDecision::Prompt
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_write_mode_auto_allows_reads() {
        let config = NodeToolConfig::default();
        assert_eq!(
            resolve_tool_policy(&config, "read", ToolTier::Read, false),
            ApprovalDecision::Allow
        );
    }

    #[test]
    fn always_ask_prompts_reads() {
        let config = NodeToolConfig {
            approval_mode: Some(ApprovalMode::AlwaysAsk),
            ..NodeToolConfig::default()
        };
        assert_eq!(
            resolve_tool_policy(&config, "read", ToolTier::Read, false),
            ApprovalDecision::Prompt
        );
    }

    #[test]
    fn override_can_deny_tool() {
        let mut config = NodeToolConfig::default();
        config.overrides.push(domain::ToolPolicyOverride {
            tool_name: "read".to_string(),
            policy: ToolPolicy::Deny,
            timeout_secs: None,
        });
        assert_eq!(
            resolve_tool_policy(&config, "read", ToolTier::Read, false),
            ApprovalDecision::Deny
        );
    }
}
