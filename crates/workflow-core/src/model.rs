use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub type WorkflowId = String;
pub type NodeId = String;
pub type EdgeId = String;

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
            id: Uuid::new_v4().to_string(),
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
            id: Uuid::new_v4().to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentNodeConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    pub model: String,
    pub output_schema: Value,
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
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunEvent {
    pub node_id: NodeId,
    pub kind: RunEventKind,
    pub message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}
