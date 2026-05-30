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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentNodeConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    pub model: String,
    pub output_schema: Value,
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
}

const fn default_auto_start() -> bool {
    true
}

impl Default for AgentNodeConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are a focused AI agent in a node workflow.".to_string(),
            task_prompt: "Return a concise JSON object for this node.".to_string(),
            model: "gpt-5.5".to_string(),
            output_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            auto_start: true,
        }
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
        assert_eq!(node.agent.model, "gpt-5.5");
        assert_eq!(node.agent.output_schema["required"], json!(["summary"]));
    }

    #[test]
    fn agent_node_defaults_auto_start_true() {
        let node = Node::agent("Plan", 0.0, 0.0);
        assert!(node.agent.auto_start);
    }

    #[test]
    fn chat_message_serde_roundtrip() {
        let msg = ChatMessage {
            role: ChatRole::Thinking,
            content: "Preparing request...".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }
}
