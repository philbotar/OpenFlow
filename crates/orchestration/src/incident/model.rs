use engine::NodeId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentCategory {
    Tool,
    AiInvoke,
    Node,
    Subagent,
    Run,
    Conversation,
    Workflow,
    Backend,
    Persistence,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IncidentScope {
    App,
    Project {
        project_id: String,
    },
    Run {
        run_id: String,
        workflow_id: String,
    },
    Node {
        run_id: String,
        workflow_id: String,
        node_id: NodeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentRecord {
    pub id: String,
    pub created_at_ms: u64,
    pub severity: IncidentSeverity,
    pub category: IncidentCategory,
    pub scope: IncidentScope,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub retryable: bool,
    #[serde(default)]
    pub context: BTreeMap<String, Value>,
    #[serde(default)]
    pub resolved: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncidentContext {
    pub run_id: Option<String>,
    pub workflow_id: Option<String>,
    pub project_id: Option<String>,
    pub node_id: Option<NodeId>,
    pub node_label: Option<String>,
}
