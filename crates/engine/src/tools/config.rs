//! Tool catalog, approval policy, and transcript types for agent nodes.

use crate::graph::NodeId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Capability class used to decide default approval behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTier {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolConcurrency {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    ReadOnly,
    AlwaysAsk,
    #[default]
    Write,
    Yolo,
}

/// Tool approval settings attached to an agent node or saved agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<ApprovalMode>,
}

impl NodeToolConfig {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn effective_approval_mode(&self) -> ApprovalMode {
        match self.approval_mode {
            Some(mode) => mode,
            None => ApprovalMode::Write,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDecision {
    AutoAllow,
    Prompt,
    Deny,
}

#[must_use]
pub const fn requires_approval(mode: ApprovalMode, tier: ToolTier) -> ToolDecision {
    decision_from_mode(mode, tier)
}

const fn decision_from_mode(mode: ApprovalMode, tier: ToolTier) -> ToolDecision {
    match mode {
        ApprovalMode::Yolo | ApprovalMode::ReadOnly => ToolDecision::AutoAllow,
        ApprovalMode::AlwaysAsk => ToolDecision::Prompt,
        ApprovalMode::Write => match tier {
            ToolTier::Read => ToolDecision::AutoAllow,
            ToolTier::Write => ToolDecision::Prompt,
        },
    }
}

#[must_use]
pub fn tool_tier_for_call(_config: &NodeToolConfig, tool_name: &str) -> ToolTier {
    default_tier_for_tool_name(tool_name)
}

fn default_tier_for_tool_name(tool_name: &str) -> ToolTier {
    match tool_name {
        "read" | "search" | "find" | "ast_grep" => ToolTier::Read,
        name if name.starts_with("mcp/") => ToolTier::Write,
        _ => ToolTier::Write,
    }
}

#[must_use]
pub fn tool_intent_from_arguments(arguments: &Value) -> Option<String> {
    let value = arguments
        .get("_i")
        .or_else(|| arguments.get("intent"))
        .and_then(Value::as_str)?
        .trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[must_use]
pub fn tool_decision_for_call(config: &NodeToolConfig, call: &ToolCall) -> ToolDecision {
    let tier = tool_tier_for_call(config, &call.name);
    decision_from_mode(config.effective_approval_mode(), tier)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub tier: ToolTier,
    pub concurrency: ToolConcurrency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Proposed,
    AwaitingApproval,
    Running,
    Completed,
    Blocked,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_meta: Option<ToolOutputMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutputMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<ToolTruncation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTruncation {
    pub strategy: ToolTruncationStrategy,
    pub total_bytes: usize,
    pub shown_bytes: usize,
    pub elided_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_lines: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown_lines: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elided_lines: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTruncationStrategy {
    Head,
    Tail,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingToolApproval {
    pub approval_id: String,
    pub node_id: NodeId,
    pub node_label: String,
    pub tool_call: ToolCall,
    pub tier: ToolTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Declared,
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubagentDeclaration {
    pub name: String,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub status: SubagentStatus,
}
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test fixtures use unwrap/expect for brevity"
)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn node_tool_config_defaults_to_write_mode() {
        let config = NodeToolConfig::default();
        assert!(config.is_enabled());
        assert_eq!(config.approval_mode, None);
        assert_eq!(config.effective_approval_mode(), ApprovalMode::Write);
    }

    #[test]
    fn node_tool_config_serde_defaults_backfill() {
        let config: NodeToolConfig = serde_json::from_value(json!({})).unwrap();
        assert_eq!(config, NodeToolConfig::default());
    }

    #[test]
    fn approval_mode_serializes_snake_case() {
        let value = serde_json::to_value(ApprovalMode::AlwaysAsk).unwrap();
        assert_eq!(value, json!("always_ask"));
        let value = serde_json::to_value(ApprovalMode::ReadOnly).unwrap();
        assert_eq!(value, json!("read_only"));
    }

    #[test]
    fn requires_approval_honours_modes_and_tiers() {
        for tier in [ToolTier::Read, ToolTier::Write] {
            assert_eq!(
                requires_approval(ApprovalMode::Yolo, tier),
                ToolDecision::AutoAllow
            );
            assert_eq!(
                requires_approval(ApprovalMode::ReadOnly, tier),
                ToolDecision::AutoAllow
            );
            assert_eq!(
                requires_approval(ApprovalMode::AlwaysAsk, tier),
                ToolDecision::Prompt
            );
        }

        assert_eq!(
            requires_approval(ApprovalMode::Write, ToolTier::Read),
            ToolDecision::AutoAllow
        );
        assert_eq!(
            requires_approval(ApprovalMode::Write, ToolTier::Write),
            ToolDecision::Prompt
        );
    }

    #[test]
    fn tool_decision_for_call_never_prompts_in_yolo_even_for_bash() {
        let config = NodeToolConfig {
            approval_mode: Some(ApprovalMode::Yolo),
        };
        let call = ToolCall {
            id: "call-bash".to_string(),
            name: "bash".to_string(),
            arguments: json!({"command": "rm -rf /"}),
        };
        assert_eq!(
            tool_decision_for_call(&config, &call),
            ToolDecision::AutoAllow
        );
    }

    #[test]
    fn tool_tier_uses_builtin_classification() {
        let config = NodeToolConfig::default();
        assert_eq!(tool_tier_for_call(&config, "read"), ToolTier::Read);
        assert_eq!(tool_tier_for_call(&config, "bash"), ToolTier::Write);
        assert_eq!(tool_tier_for_call(&config, "custom_write"), ToolTier::Write);
        assert_eq!(
            tool_tier_for_call(&config, "mcp/gh/search"),
            ToolTier::Write
        );
    }

    #[test]
    fn subagent_declaration_deserialize_valid() {
        let json = json!({
            "name": "Researcher",
            "purpose": "Investigate API behavior"
        });
        let dec: SubagentDeclaration = serde_json::from_value(json).unwrap();
        assert_eq!(dec.name, "Researcher");
        assert_eq!(dec.purpose, "Investigate API behavior");
    }

    #[test]
    fn extracts_tool_intent_from_i_field() {
        assert_eq!(
            tool_intent_from_arguments(&serde_json::json!({"_i": "inspect config"})),
            Some("inspect config".to_string())
        );
    }

    #[test]
    fn blank_tool_intent_is_ignored() {
        assert_eq!(
            tool_intent_from_arguments(&serde_json::json!({"_i": "   "})),
            None
        );
    }

    #[test]
    fn subagent_declaration_rejects_missing_name() {
        let json = json!({
            "purpose": "Investigate"
        });
        let result = serde_json::from_value::<SubagentDeclaration>(json);
        assert!(result.is_err());
    }

    #[test]
    fn subagent_declaration_rejects_missing_purpose() {
        let json = json!({
            "name": "Researcher"
        });
        let result = serde_json::from_value::<SubagentDeclaration>(json);
        assert!(result.is_err());
    }

    #[test]
    fn subagent_declaration_rejects_additional_properties() {
        let json = json!({
            "name": "Researcher",
            "purpose": "Investigate",
            "extra": "field"
        });
        let result = serde_json::from_value::<SubagentDeclaration>(json);
        assert!(result.is_err());
    }

    #[test]
    fn subagent_status_serializes_snake_case() {
        let value = serde_json::to_value(SubagentStatus::Declared).unwrap();
        assert_eq!(value, json!("declared"));
        let value = serde_json::to_value(SubagentStatus::Active).unwrap();
        assert_eq!(value, json!("active"));
        let value = serde_json::to_value(SubagentStatus::Completed).unwrap();
        assert_eq!(value, json!("completed"));
        let value = serde_json::to_value(SubagentStatus::Failed).unwrap();
        assert_eq!(value, json!("failed"));
    }

    #[test]
    fn subagent_summary_serializes_camel_case() {
        let summary = SubagentSummary {
            id: "node-1-subagent-1".to_string(),
            name: "Researcher".to_string(),
            purpose: "Investigate".to_string(),
            status: SubagentStatus::Declared,
        };
        let value = serde_json::to_value(&summary).unwrap();
        assert_eq!(value["id"], "node-1-subagent-1");
        assert_eq!(value["name"], "Researcher");
        assert_eq!(value["purpose"], "Investigate");
        assert_eq!(value["status"], "declared");
    }
}
