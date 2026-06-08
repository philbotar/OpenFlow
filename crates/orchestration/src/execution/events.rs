use crate::state::{
    AgentStatus, RunTraceEntry, ToolArtifactSummary, ToolCallSummary, TraceStatus,
    WorkflowRunState,
};
use domain::{
    summary_from_node_output, ChatMessage, ChatRole, NodeId, SubagentStatus, ToolCallStatus,
    Workflow,
};

use super::ExecutionEvent;

pub fn apply_event_to_run_state(
    _workflow: &Workflow,
    state: &mut WorkflowRunState,
    event: ExecutionEvent,
) {
    match event {
        ExecutionEvent::NodeQueued { node_id, label } => {
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Queued);
            state.run_trace.push(RunTraceEntry {
                node_id,
                node_label: label,
                status: TraceStatus::Queued,
                message: "queued".to_string(),
                output: None,
            });
        }
        ExecutionEvent::NodeStarted { node_id, label } => {
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Started);
            state.run_trace.push(RunTraceEntry {
                node_id,
                node_label: label,
                status: TraceStatus::Running,
                message: "invoking model".to_string(),
                output: None,
            });
        }
        ExecutionEvent::ChatMessage {
            node_id,
            role,
            content,
        } => {
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(role, content));
        }
        ExecutionEvent::NodeAwaitingInput {
            node_id,
            label,
            context,
            ..
        } => {
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::AwaitingInput);
            state.awaiting_node_id = Some(node_id.clone());
            state.active_manual_node_id = None;
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label.clone(),
                status: TraceStatus::Paused,
                message: "paused for human input".to_string(),
                output: None,
            });
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Node '{label}' is awaiting human input."),
                ));
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::Thinking,
                    format!("Context:\n{context}"),
                ));
        }
        ExecutionEvent::ToolCallProposed {
            node_id, tool_call, ..
        } => {
            let calls = state.tool_calls_by_node.entry(node_id.clone()).or_default();
            calls.push(ToolCallSummary {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                status: ToolCallStatus::Proposed,
                arguments: tool_call.arguments.clone(),
                last_output: None,
                is_error: false,
            });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::tool_marker(tool_call.id.clone()));
        }
        ExecutionEvent::ToolApprovalRequested { request } => {
            state.awaiting_node_id = None;
            state.active_tool_call_id = Some(request.tool_call.id.clone());
            state.pending_approvals = vec![request.clone()];
            state.status_by_node.insert(
                NodeId(request.node_id.clone()),
                AgentStatus::AwaitingToolApproval,
            );
            state.run_trace.push(RunTraceEntry {
                node_id: NodeId(request.node_id.clone()),
                node_label: request.node_label.clone(),
                status: TraceStatus::Paused,
                message: format!("awaiting approval for {}", request.tool_call.name),
                output: None,
            });
            state
                .chat_logs
                .entry(NodeId(request.node_id.clone()))
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Approval required for tool '{}'.", request.tool_call.name),
                ));
            update_tool_status(
                state,
                &NodeId(request.node_id),
                &request.tool_call.id,
                ToolCallStatus::AwaitingApproval,
                None,
                false,
            );
        }
        ExecutionEvent::ToolApproved {
            node_id,
            tool_call_id,
            ..
        } => {
            state.pending_approvals.clear();
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                None,
                false,
            );
        }
        ExecutionEvent::ToolDenied {
            node_id,
            tool_call_id,
            reason,
            ..
        } => {
            state.pending_approvals.clear();
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Blocked,
                Some(reason),
                true,
            );
        }
        ExecutionEvent::ToolStarted {
            node_id,
            tool_call_id,
            tool_name,
            ..
        } => {
            state.active_tool_call_id = Some(tool_call_id.clone());
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::RunningTool);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: tool_name.clone(),
                status: TraceStatus::Running,
                message: format!("running tool {tool_name}"),
                output: None,
            });
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                None,
                false,
            );
        }
        ExecutionEvent::ToolCompleted {
            node_id,
            tool_call_id,
            tool_name: _,
            content,
            is_error,
            artifact_ids: _,
            ..
        } => {
            state.active_tool_call_id = None;
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                if is_error {
                    ToolCallStatus::Failed
                } else {
                    ToolCallStatus::Completed
                },
                Some(content),
                is_error,
            );
        }
        ExecutionEvent::ToolArtifactCreated {
            artifact_id,
            tool_name,
            path,
            size_bytes,
            ..
        } => {
            state.tool_artifacts.insert(
                artifact_id.clone(),
                ToolArtifactSummary {
                    artifact_id,
                    tool_name,
                    path,
                    size_bytes,
                },
            );
        }
        ExecutionEvent::NodeCompleted {
            node_id,
            label,
            output,
        } => {
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Completed);
            state.outputs.insert(node_id.clone(), output.clone());
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label,
                status: TraceStatus::Completed,
                message: "completed".to_string(),
                output: Some(output.clone()),
            });
            if let Some(summary) = summary_from_node_output(&output) {
                state
                    .chat_logs
                    .entry(node_id)
                    .or_default()
                    .push(ChatMessage::node_completed(summary));
            }
        }
        ExecutionEvent::NodeFailed {
            node_id,
            label,
            error,
        } => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Failed);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label,
                status: TraceStatus::Failed,
                message: error.clone(),
                output: None,
            });
            state.last_error = Some(error.clone());
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Failed: {error}"),
                ));
        }
        ExecutionEvent::Finished(report) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state.last_report = Some(report);
        }
        ExecutionEvent::Error(error) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state.last_error = Some(error);
        }
        ExecutionEvent::SubagentsDeclared { node_id, summaries } => {
            let count = summaries.len();
            let entry = state.subagents_by_node.entry(node_id.clone()).or_default();
            for summary in summaries {
                if let Some(existing) = entry.iter_mut().find(|item| item.id == summary.id) {
                    *existing = summary;
                } else {
                    entry.push(summary);
                }
            }
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Registered {count} subagent(s)."),
                ));
        }
        ExecutionEvent::SubagentStarted {
            node_id,
            subagent_id,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Active;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} started.", subagent_id),
                ));
        }
        ExecutionEvent::SubagentCompleted {
            node_id,
            subagent_id,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Completed;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} completed.", subagent_id),
                ));
        }
        ExecutionEvent::SubagentFailed {
            node_id,
            subagent_id,
            error,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Failed;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} failed: {}", subagent_id, error),
                ));
        }
    }
}

fn update_tool_status(
    state: &mut WorkflowRunState,
    node_id: &NodeId,
    tool_call_id: &str,
    status: ToolCallStatus,
    content: Option<String>,
    is_error: bool,
) {
    if let Some(call) = state
        .tool_calls_by_node
        .entry(node_id.clone())
        .or_default()
        .iter_mut()
        .find(|call| call.tool_call_id == tool_call_id)
    {
        call.status = status;
        call.is_error = is_error;
        if let Some(content) = content {
            call.last_output = Some(content);
        }
    }
}

pub fn record_user_input(state: &mut WorkflowRunState, node_id: &str, text: String) {
    state
        .chat_logs
        .entry(NodeId(node_id.to_string()))
        .or_default()
        .push(ChatMessage::text(ChatRole::User, text));
    state.awaiting_node_id = None;
    state.active_manual_node_id = None;
}
