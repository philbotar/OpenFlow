use engine::{ChatMessage, NodeId, RunReport, SubagentSummary, Workflow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentStatus {
    Idle,
    Queued,
    Started,
    AwaitingInput,
    AwaitingToolApproval,
    RunningTool,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraceStatus {
    Queued,
    Running,
    Paused,
    Failed,
    Stopped,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTraceEntry {
    pub node_id: NodeId,
    pub node_label: String,
    pub status: TraceStatus,
    pub message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallSummary {
    pub tool_call_id: String,
    pub tool_name: String,
    pub status: engine::ToolCallStatus,
    pub arguments: Value,
    pub last_output: Option<String>,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolArtifactSummary {
    pub artifact_id: String,
    pub tool_name: String,
    pub path: String,
    pub size_bytes: usize,
}

/// Live run state pushed to the frontend via Tauri events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunState {
    pub active: bool,
    pub awaiting_node_id: Option<NodeId>,
    #[serde(default)]
    pub awaiting_node_ids: Vec<NodeId>,
    pub active_manual_node_id: Option<NodeId>,
    pub active_tool_call_id: Option<String>,
    pub pending_approvals: Vec<engine::PendingToolApproval>,
    pub tool_calls_by_node: BTreeMap<NodeId, Vec<ToolCallSummary>>,
    pub tool_artifacts: BTreeMap<String, ToolArtifactSummary>,
    pub exec_approval_granted: bool,
    pub status_by_node: BTreeMap<NodeId, AgentStatus>,
    pub subagents_by_node: BTreeMap<NodeId, Vec<SubagentSummary>>,
    pub last_report: Option<RunReport>,
    pub last_error: Option<String>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub run_trace: Vec<RunTraceEntry>,
    pub outputs: BTreeMap<NodeId, Value>,
    pub changed_files: Vec<engine::FileChangeRecord>,
    #[serde(default)]
    pub changed_files_by_node: BTreeMap<NodeId, Vec<engine::FileChangeRecord>>,
    #[serde(default)]
    pub edit_batches: Vec<engine::EditBatch>,
}

impl WorkflowRunState {
    #[must_use]
    pub fn running_for_workflow(workflow: &Workflow) -> Self {
        let status_by_node = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), AgentStatus::Idle))
            .collect();
        Self {
            active: true,
            awaiting_node_id: None,
            awaiting_node_ids: Vec::new(),
            active_manual_node_id: None,
            active_tool_call_id: None,
            pending_approvals: Vec::new(),
            tool_calls_by_node: BTreeMap::new(),
            tool_artifacts: BTreeMap::new(),
            exec_approval_granted: false,
            status_by_node,
            subagents_by_node: BTreeMap::new(),
            last_report: None,
            last_error: None,
            chat_logs: BTreeMap::new(),
            run_trace: Vec::new(),
            outputs: BTreeMap::new(),
            changed_files: Vec::new(),
            changed_files_by_node: BTreeMap::new(),
            edit_batches: Vec::new(),
        }
    }

    #[must_use]
    pub fn idle_for_workflow(workflow: &Workflow) -> Self {
        Self {
            active: false,
            ..Self::running_for_workflow(workflow)
        }
    }
}
