//! Rich run telemetry events for interactive execution (UI projection and replay).

use super::artifacts::RunReport;
use crate::conversation::ChatRole;
use crate::graph::NodeId;
use crate::tools::{
    EditBatch, FileChangeRecord, PendingToolApproval, SubagentSummary, ToolCall, ToolOutputMeta,
};
use serde_json::Value;

/// Atomic telemetry event during an interactive run.
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
    ChatMessageDelta {
        node_id: NodeId,
        message_id: String,
        role: ChatRole,
        delta: String,
        finalize: bool,
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
    ToolRetrying {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        attempt: u8,
        backoff_ms: u64,
    },
    ToolUpdated {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        content: String,
        output_meta: Option<ToolOutputMeta>,
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
    FileChanged {
        node_id: NodeId,
        record: FileChangeRecord,
    },
    EditBatchRecorded {
        node_id: NodeId,
        batch: EditBatch,
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
    /// Node was interrupted by the user; run stays active and the node is retryable.
    NodeInterrupted {
        node_id: NodeId,
        label: String,
    },
    /// Node failed but the run stays active; the node is retryable.
    NodeErrored {
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
    /// Completed phase timing for performance diagnosis (AI invoke, tool run, etc.).
    PhaseTimed {
        phase: String,
        label: String,
        node_id: Option<NodeId>,
        duration_ms: u64,
    },
    /// Token usage report received for a node after an LLM invocation.
    UsageReported {
        node_id: NodeId,
        usage: crate::UsageReport,
        model: String,
        max_context_tokens: Option<u32>,
    },
    /// LLM invocation failed for a workflow node (recorded for incident persistence).
    AiInvokeFailed {
        node_id: NodeId,
        label: String,
        error: String,
    },
    /// Overseer repair started for a malformed final-output submit (sanitized; no raw content).
    OutputRepairStarted {
        node_id: NodeId,
        model: String,
    },
    /// Overseer repair produced a completion-protocol-accepted candidate.
    OutputRepairSucceeded {
        node_id: NodeId,
        model: String,
    },
    /// Overseer repair failed; sanitized reason only. Does not set run `last_error`.
    OutputRepairFailed {
        node_id: NodeId,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_updated_debug() {
        let event = RunTelemetry::ToolUpdated {
            node_id: NodeId("n1".to_string()),
            tool_call_id: "tc-1".to_string(),
            tool_name: "bash".to_string(),
            content: "hello world".to_string(),
            output_meta: None,
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("ToolUpdated"));
        assert!(debug.contains("hello world"));
        assert!(debug.contains("tc-1"));
    }

    #[test]
    fn tool_retrying_debug() {
        let event = RunTelemetry::ToolRetrying {
            node_id: NodeId("n1".to_string()),
            tool_call_id: "tc-1".to_string(),
            tool_name: "bash".to_string(),
            attempt: 2,
            backoff_ms: 2000,
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("ToolRetrying"));
        assert!(debug.contains("attempt: 2"));
    }
}
