//! Rich run telemetry events for interactive execution (UI projection and replay).

use crate::conversation::ChatRole;
use crate::graph::NodeId;
use crate::tools::{PendingToolApproval, SubagentSummary, ToolCall, ToolOutputMeta};
use super::artifacts::RunReport;
use serde_json::Value;

/// Atomic telemetry event during an interactive run.
///
/// Superset of lifecycle [`RunEventKind`] values plus chat, tool, and subagent detail.
/// Orchestration projects this into `WorkflowRunState`; the headless report still uses
/// compact [`crate::RunEvent`] records from the engine.
#[derive(Debug, Clone)]
pub enum RunTelemetry {
    NodeQueued {
        node_id: NodeId,
        label: String,
    },
    NodeStarted {
        node_id: NodeId,
        label: String,
    },
    ChatMessage {
        node_id: NodeId,
        role: ChatRole,
        content: String,
    },
    NodeAwaitingInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    ToolCallProposed {
        node_id: NodeId,
        label: String,
        tool_call: ToolCall,
    },
    ToolApprovalRequested {
        request: PendingToolApproval,
    },
    ToolApproved {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
    },
    ToolDenied {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        reason: String,
    },
    ToolStarted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCompleted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        content: String,
        is_error: bool,
        output_meta: Option<ToolOutputMeta>,
        artifact_ids: Vec<String>,
    },
    ToolArtifactCreated {
        node_id: NodeId,
        artifact_id: String,
        tool_name: String,
        path: String,
        size_bytes: usize,
    },
    NodeCompleted {
        node_id: NodeId,
        label: String,
        output: Value,
    },
    NodeFailed {
        node_id: NodeId,
        label: String,
        error: String,
    },
    Finished(RunReport),
    Aborted,
    Error(String),
    SubagentsDeclared {
        node_id: NodeId,
        summaries: Vec<SubagentSummary>,
    },
    SubagentStarted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentCompleted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentFailed {
        node_id: NodeId,
        subagent_id: String,
        error: String,
    },
}
