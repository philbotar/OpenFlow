#![allow(clippy::use_self, clippy::derive_partial_eq_without_eq)]

use crate::tools::NodeToolConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for NodeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        NodeId(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

impl From<NodeId> for String {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl PartialEq<str> for NodeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for NodeId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for NodeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub String);

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for EdgeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for EdgeId {
    fn from(s: String) -> Self {
        EdgeId(s)
    }
}

impl From<&str> for EdgeId {
    fn from(s: &str) -> Self {
        EdgeId(s.to_string())
    }
}

impl From<EdgeId> for String {
    fn from(id: EdgeId) -> Self {
        id.0
    }
}

impl PartialEq<str> for EdgeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for EdgeId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for EdgeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for WorkflowId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for WorkflowId {
    fn from(s: String) -> Self {
        WorkflowId(s)
    }
}

impl From<&str> for WorkflowId {
    fn from(s: &str) -> Self {
        WorkflowId(s.to_string())
    }
}

impl From<WorkflowId> for String {
    fn from(id: WorkflowId) -> Self {
        id.0
    }
}

impl PartialEq<str> for WorkflowId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for WorkflowId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for WorkflowId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub id: WorkflowId,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Workflow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: WorkflowId(Uuid::new_v4().to_string()),
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub kind: NodeKind,
    pub position: NodePosition,
    pub agent: AgentNodeConfig,
}

impl Node {
    pub fn agent(label: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id: NodeId(Uuid::new_v4().to_string()),
            label: label.into(),
            kind: NodeKind::Agent,
            position: NodePosition { x, y },
            agent: AgentNodeConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeKind {
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatRole {
    System,
    Thinking,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, rename = "toolCallId", skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    #[must_use]
    pub fn text(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_call_id: None,
        }
    }

    #[must_use]
    pub fn tool_marker(tool_call_id: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Thinking,
            content: String::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// True when assistant text only echoes structured tool invocation markup.
#[must_use]
pub fn is_redundant_tool_call_markup(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.starts_with("<tool_call")
        || trimmed.starts_with("```tool_call")
        || (trimmed.contains("<function=") && trimmed.contains("</tool_call>"))
}

/// Drop assistant text that duplicates structured tool calls in chat/transcript.
#[must_use]
pub fn filter_tool_turn_assistant_message(message: Option<String>) -> Option<String> {
    message.filter(|content| !is_redundant_tool_call_markup(content))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentNodeConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    #[serde(default)]
    pub model: String,
    pub output_schema: Value,
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
    #[serde(default)]
    pub tools: NodeToolConfig,
}

const fn default_auto_start() -> bool {
    true
}

impl Default for AgentNodeConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are a focused AI agent in a node workflow.".to_string(),
            task_prompt: "Return a concise JSON object for this node.".to_string(),
            model: String::new(),
            output_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            auto_start: true,
            tools: NodeToolConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub config: AgentNodeConfig,
}

impl NodeTemplate {
    #[must_use]
    pub fn builtin_defaults() -> Vec<NodeTemplate> {
        vec![
            NodeTemplate {
                id: "builtin.task-runner".to_string(),
                name: "Task Runner".to_string(),
                description: "Executes a single focused task".to_string(),
                config: AgentNodeConfig::default(),
            },
            NodeTemplate {
                id: "builtin.code-assistant".to_string(),
                name: "Code Assistant".to_string(),
                description: "Writes and reviews code".to_string(),
                config: AgentNodeConfig::default(),
            },
            NodeTemplate {
                id: "builtin.writer".to_string(),
                name: "Writer".to_string(),
                description: "Generates prose, docs, and content".to_string(),
                config: AgentNodeConfig::default(),
            },
            NodeTemplate {
                id: "builtin.analyst".to_string(),
                name: "Analyst".to_string(),
                description: "Analyzes data and provides insights".to_string(),
                config: AgentNodeConfig::default(),
            },
            NodeTemplate {
                id: "builtin.translator".to_string(),
                name: "Translator".to_string(),
                description: "Translates between formats and languages".to_string(),
                config: AgentNodeConfig::default(),
            },
            NodeTemplate {
                id: "builtin.ideator".to_string(),
                name: "Ideator".to_string(),
                description: "Generates creative ideas and approaches".to_string(),
                config: AgentNodeConfig::default(),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
}

impl Edge {
    pub fn new(from: impl Into<NodeId>, to: impl Into<NodeId>) -> Self {
        Self {
            id: EdgeId(Uuid::new_v4().to_string()),
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRunOutput {
    pub node_id: NodeId,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunEventKind {
    Queued,
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunEvent {
    pub node_id: NodeId,
    pub kind: RunEventKind,
    pub message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunReport {
    pub workflow_id: WorkflowId,
    pub events: Vec<RunEvent>,
    pub outputs: Vec<NodeRunOutput>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_workflow_starts_empty_with_name() {
        let workflow = Workflow::new("Feature planner");

        assert_eq!(workflow.name, "Feature planner");
        assert!(workflow.nodes.is_empty());
        assert!(workflow.edges.is_empty());
        assert!(!workflow.id.is_empty());
    }

    #[test]
    fn agent_node_defaults_to_structured_summary_output() {
        let node = Node::agent("Plan", 24.0, 48.0);

        assert_eq!(node.label, "Plan");
        assert_eq!(node.kind, NodeKind::Agent);
        assert_eq!(node.position, NodePosition { x: 24.0, y: 48.0 });
        assert_eq!(node.agent.model, "");
        assert_eq!(node.agent.output_schema["required"], json!(["summary"]));
    }

    #[test]
    fn agent_node_defaults_auto_start_true() {
        let node = Node::agent("Plan", 0.0, 0.0);
        assert!(node.agent.auto_start);
    }

    #[test]
    fn agent_node_defaults_tools_enabled() {
        let node = Node::agent("Plan", 0.0, 0.0);
        assert!(node.agent.tools.is_enabled());
        assert_eq!(node.agent.tools.catalog.tools.len(), 4);
    }

    #[test]
    fn agent_node_config_serde_backfills_new_tool_fields() {
        let config: AgentNodeConfig = serde_json::from_value(json!({
            "system_prompt": "sys",
            "task_prompt": "task",
            "model": "gpt-test",
            "output_schema": { "type": "object" }
        }))
        .unwrap();

        assert!(config.auto_start);
        assert_eq!(config.tools, NodeToolConfig::default());
    }

    #[test]
    fn chat_message_serde_roundtrip() {
        let msg = ChatMessage::text(ChatRole::Thinking, "Preparing request...");
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);

        let marker = ChatMessage::tool_marker("call-1");
        let marker_json = serde_json::to_string(&marker).unwrap();
        assert!(marker_json.contains("\"toolCallId\":\"call-1\""));
        let marker_back: ChatMessage = serde_json::from_str(&marker_json).unwrap();
        assert_eq!(marker, marker_back);
    }

    #[test]
    fn redundant_tool_call_markup_detects_xml_echoes() {
        assert!(is_redundant_tool_call_markup(
            "<tool_call>\n<function=search>\n</function>\n</tool_call>"
        ));
        assert!(!is_redundant_tool_call_markup("Let me search the repo for TODOs."));
    }

    #[test]
    fn filter_tool_turn_assistant_message_keeps_human_text() {
        assert_eq!(
            filter_tool_turn_assistant_message(Some("Checking README.".to_string())),
            Some("Checking README.".to_string())
        );
        assert_eq!(
            filter_tool_turn_assistant_message(Some(
                "<tool_call><function=read></function></tool_call>".to_string()
            )),
            None
        );
    }

    #[test]
    fn builtin_defaults_has_six_templates() {
        let templates = NodeTemplate::builtin_defaults();
        assert_eq!(templates.len(), 6);
    }

    #[test]
    fn builtin_defaults_all_have_builtin_prefix() {
        for template in NodeTemplate::builtin_defaults() {
            assert!(
                template.id.starts_with("builtin."),
                "expected id to start with 'builtin.': {}",
                template.id
            );
        }
    }

    #[test]
    fn builtin_defaults_have_unique_ids() {
        let ids: Vec<String> = NodeTemplate::builtin_defaults()
            .into_iter()
            .map(|t| t.id)
            .collect();
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len());
    }

    #[test]
    fn node_template_serialization_roundtrip() {
        let template = NodeTemplate {
            id: "builtin.writer".to_string(),
            name: "Writer".to_string(),
            description: "Generates prose, docs, and content".to_string(),
            config: AgentNodeConfig::default(),
        };
        let json = serde_json::to_string(&template).unwrap();
        let back: NodeTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(template, back);
    }
}
