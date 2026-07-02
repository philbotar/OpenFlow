use engine::{ChatMessage, NodeId, RunReport, SubagentSummary, Workflow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Queued,
    Started,
    AwaitingInput,
    AwaitingToolApproval,
    RunningTool,
    Completed,
    Failed,
    Interrupted,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    pub last_output: Option<String>,
    pub is_error: bool,
    #[serde(default)]
    pub streaming: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolArtifactSummary {
    pub artifact_id: String,
    pub tool_name: String,
    pub path: String,
    pub size_bytes: usize,
}

/// Per-node context window usage snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindowSnapshot {
    pub used_tokens: u32,
    pub max_tokens: u32,
    pub model: String,
    pub node_id: NodeId,
}

/// Live run state pushed to the frontend via Tauri events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunState {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
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
    /// Per-node context window usage snapshots for the bubble indicator.
    #[serde(default)]
    pub context_window_by_node: BTreeMap<NodeId, ContextWindowSnapshot>,
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
            run_id: None,
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
            context_window_by_node: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn idle_for_workflow(workflow: &Workflow) -> Self {
        Self {
            active: false,
            ..Self::running_for_workflow(workflow)
        }
    }

    /// Read-only replay view: durable history without live pause/approval handles.
    #[must_use]
    pub fn into_replay_projection(mut self) -> Self {
        self.active = false;
        self.awaiting_node_id = None;
        self.awaiting_node_ids.clear();
        self.active_manual_node_id = None;
        self.active_tool_call_id = None;
        self.pending_approvals.clear();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentStatus, WorkflowRunState};
    use engine::{NodeId, Workflow};

    #[test]
    fn agent_status_serializes_snake_case_for_frontend() {
        assert_eq!(
            serde_json::to_string(&AgentStatus::AwaitingInput).expect("serialize"),
            "\"awaiting_input\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::RunningTool).expect("serialize"),
            "\"running_tool\""
        );
    }

    #[test]
    fn into_replay_projection_clears_live_interaction_handles() {
        let workflow = Workflow::new("Replay");
        let mut state = WorkflowRunState::running_for_workflow(&workflow);
        state.awaiting_node_id = Some(NodeId("node-1".to_string()));
        state.awaiting_node_ids.push(NodeId("node-1".to_string()));
        state.active_manual_node_id = Some(NodeId("node-1".to_string()));
        state.active_tool_call_id = Some("call-1".to_string());
        state.pending_approvals.push(engine::PendingToolApproval {
            approval_id: "approval-1".to_string(),
            node_id: NodeId::from("node-1"),
            node_label: "Node".to_string(),
            tool_call: engine::ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({}),
            },
            tier: engine::ToolTier::Read,
        });

        let replay = state.into_replay_projection();

        assert!(!replay.active);
        assert!(replay.awaiting_node_id.is_none());
        assert!(replay.awaiting_node_ids.is_empty());
        assert!(replay.active_manual_node_id.is_none());
        assert!(replay.active_tool_call_id.is_none());
        assert!(replay.pending_approvals.is_empty());
    }
}
