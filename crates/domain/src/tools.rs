use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolTier {
    Read,
    Write,
    Exec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolConcurrency {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPolicy {
    Allow,
    Prompt,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    AlwaysAsk,
    #[default]
    Write,
    Yolo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRef {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogSelection {
    #[serde(default = "default_tool_refs")]
    pub tools: Vec<ToolRef>,
}

impl Default for ToolCatalogSelection {
    fn default() -> Self {
        Self {
            tools: default_tool_refs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPolicyOverride {
    pub tool_name: String,
    pub policy: ToolPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeToolConfig {
    #[serde(default)]
    pub catalog: ToolCatalogSelection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<ApprovalMode>,
    #[serde(default)]
    pub overrides: Vec<ToolPolicyOverride>,
    #[serde(default = "default_max_tool_rounds")]
    pub max_tool_rounds: u8,
}

impl NodeToolConfig {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        !self.catalog.tools.is_empty()
    }
}

impl Default for NodeToolConfig {
    fn default() -> Self {
        Self {
            catalog: ToolCatalogSelection::default(),
            approval_mode: None,
            overrides: Vec::new(),
            max_tool_rounds: default_max_tool_rounds(),
        }
    }
}

const fn default_max_tool_rounds() -> u8 {
    8
}

fn default_tool_refs() -> Vec<ToolRef> {
    ["read", "search", "find", "ast_grep"]
        .into_iter()
        .map(|name| ToolRef {
            name: name.to_string(),
        })
        .collect()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
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
#[serde(rename_all = "snake_case")]
pub enum AgentTranscriptItem {
    AssistantMessage { content: String },
    UserMessage { content: String },
    ToolCall { call: ToolCall },
    ToolResult { result: ToolResult },
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
    pub node_id: String,
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn node_tool_config_defaults_enable_retrieval_tools() {
        let config = NodeToolConfig::default();
        assert!(config.is_enabled());
        assert_eq!(config.max_tool_rounds, 8);
        assert_eq!(config.approval_mode, None);
        assert_eq!(config.catalog.tools, default_tool_refs());
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
