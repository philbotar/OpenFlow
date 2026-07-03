use crate::run::state::{
    AgentStatus, ContextWindowSnapshot, RunTraceEntry, ToolArtifactSummary, ToolCallSummary,
    TraceStatus, WorkflowRunState,
};
use engine::{
    strip_tool_call_markup, summary_from_node_output, ChatMessage, ChatRole, NodeId,
    SubagentStatus, ToolCallStatus, Workflow,
};
use serde_json::json;

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
            clear_node_awaiting_and_approvals(state, &node_id);
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
        ExecutionEvent::ChatMessageDelta {
            node_id,
            message_id,
            role,
            delta,
            finalize,
        } => {
            let logs = state.chat_logs.entry(node_id.clone()).or_default();
            let mut drop_message_id: Option<String> = None;
            if let Some(message) = logs
                .iter_mut()
                .rev()
                .find(|message| message.id.as_deref() == Some(message_id.as_str()))
            {
                if !delta.is_empty() {
                    // Accumulate raw while streaming; the UI strips markup for
                    // display. Stripping the stored content on every delta is
                    // O(n²) over the stream and lossy when markup spans delta
                    // boundaries.
                    message.content.push_str(&delta);
                }
                if finalize {
                    message.streaming = false;
                    if message.role == ChatRole::Assistant {
                        message.content = strip_tool_call_markup(&message.content);
                    }
                    if message.content.trim().is_empty() {
                        drop_message_id = Some(message_id.clone());
                    }
                }
            } else if !finalize {
                let message = if role == ChatRole::Thinking {
                    ChatMessage::streaming_thinking(message_id, delta)
                } else {
                    ChatMessage::streaming_assistant(message_id, delta)
                };
                logs.push(message);
            }
            if let Some(id) = drop_message_id {
                logs.retain(|message| message.id.as_deref() != Some(id.as_str()));
            }
            if logs.is_empty() {
                state.chat_logs.remove(&node_id);
            }
        }
        ExecutionEvent::NodeAwaitingInput { node_id, label, .. } => {
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::AwaitingInput);
            add_awaiting_node(state, node_id.clone());
            state.active_manual_node_id = None;
            state.run_trace.push(RunTraceEntry {
                node_id,
                node_label: label,
                status: TraceStatus::Paused,
                message: "paused for human input".to_string(),
                output: None,
            });
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
                intent: engine::tool_intent_from_arguments(&tool_call.arguments),
                last_output: None,
                is_error: false,
                streaming: false,
            });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::tool_marker(tool_call.id.clone()));
        }
        ExecutionEvent::ToolApprovalRequested { request } => {
            remove_awaiting_node(state, &request.node_id);
            state.active_tool_call_id = Some(request.tool_call.id.clone());
            if !state
                .pending_approvals
                .iter()
                .any(|pending| pending.approval_id == request.approval_id)
            {
                state.pending_approvals.push(request.clone());
            }
            state
                .status_by_node
                .insert(request.node_id.clone(), AgentStatus::AwaitingToolApproval);
            state.run_trace.push(RunTraceEntry {
                node_id: request.node_id.clone(),
                node_label: request.node_label.clone(),
                status: TraceStatus::Paused,
                message: format!("awaiting approval for {}", request.tool_call.name),
                output: None,
            });
            state
                .chat_logs
                .entry(request.node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Approval required for tool '{}'.", request.tool_call.name),
                ));
            update_tool_status(
                state,
                &request.node_id,
                &request.tool_call.id,
                ToolCallStatus::AwaitingApproval,
                None,
                false,
            );
        }
        ExecutionEvent::ToolApproved {
            approval_id,
            node_id,
            tool_call_id,
            ..
        } => {
            remove_pending_approval(state, &approval_id);
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
            approval_id,
            node_id,
            tool_call_id,
            reason,
            ..
        } => {
            remove_pending_approval(state, &approval_id);
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Blocked,
                Some(reason),
                true,
            );
            restore_active_node_status(state, &node_id);
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
            if tool_name == "bash" {
                set_tool_streaming(state, &node_id, &tool_call_id, true);
            }
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                None,
                false,
            );
        }
        ExecutionEvent::ToolRetrying {
            node_id,
            tool_call_id,
            tool_name,
            attempt,
            backoff_ms,
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
                message: format!(
                    "retrying tool {tool_name} (attempt {attempt}, backoff {backoff_ms}ms)"
                ),
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
        ExecutionEvent::ToolUpdated {
            node_id,
            tool_call_id,
            tool_name: _,
            content,
            output_meta: _,
        } => {
            state.active_tool_call_id = Some(tool_call_id.clone());
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::RunningTool);
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                Some(content),
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
            set_tool_streaming(state, &node_id, &tool_call_id, false);
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
            restore_active_node_status(state, &node_id);
        }
        ExecutionEvent::FileChanged { node_id, record } => {
            state.changed_files.push(record.clone());
            state
                .changed_files_by_node
                .entry(node_id)
                .or_default()
                .push(record);
        }
        ExecutionEvent::EditBatchRecorded { batch, .. } => {
            state.edit_batches.push(batch);
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
            clear_node_session_focus(state, &node_id);
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
        ExecutionEvent::NodeInterrupted { node_id, label } => {
            clear_node_session_focus(state, &node_id);
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Interrupted);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label,
                status: TraceStatus::Paused,
                message: "interrupted by user".to_string(),
                output: None,
            });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    "Interrupted by user.".to_string(),
                ));
        }
        ExecutionEvent::NodeErrored {
            node_id,
            label,
            error,
        } => record_node_failure(state, node_id, label, error, false),
        ExecutionEvent::NodeFailed {
            node_id,
            label,
            error,
        } => record_node_failure(state, node_id, label, error, true),
        ExecutionEvent::Finished(report) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.awaiting_node_ids.clear();
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state.last_report = Some(report);
        }
        ExecutionEvent::Aborted => {
            state.active = false;
            state.awaiting_node_id = None;
            state.awaiting_node_ids.clear();
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            abort_in_progress_tools(state);
            if let Some((node_id, label)) = running_node_snapshot(state) {
                state
                    .status_by_node
                    .insert(node_id.clone(), AgentStatus::Stopped);
                if let Some(entry) = state.run_trace.iter_mut().rev().find(|entry| {
                    entry.node_id == node_id
                        && matches!(
                            entry.status,
                            TraceStatus::Running | TraceStatus::Paused | TraceStatus::Queued
                        )
                }) {
                    entry.status = TraceStatus::Stopped;
                    entry.message = "Stopped".to_string();
                } else {
                    state.run_trace.push(RunTraceEntry {
                        node_id,
                        node_label: label,
                        status: TraceStatus::Stopped,
                        message: "Stopped".to_string(),
                        output: None,
                    });
                }
            }
        }
        ExecutionEvent::Error(error) if error.starts_with("ignored ") => {
            // Stale UI actions during an interaction pause must not tear down the session.
            log::warn!("stale run action: {error}");
        }
        ExecutionEvent::Error(error) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.awaiting_node_ids.clear();
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
        ExecutionEvent::PhaseTimed {
            phase,
            label,
            node_id,
            duration_ms,
        } => {
            let message = format_phase_timed_message(&phase, &label, duration_ms);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.unwrap_or_else(|| NodeId("—".to_string())),
                node_label: label,
                status: TraceStatus::Completed,
                message,
                output: Some(json!({ "phase": phase, "durationMs": duration_ms })),
            });
        }
        ExecutionEvent::UsageReported {
            node_id,
            usage,
            model,
            max_context_tokens,
        } => {
            let max_tokens = max_context_tokens.unwrap_or(0);
            state.context_window_by_node.insert(
                node_id.clone(),
                ContextWindowSnapshot {
                    used_tokens: usage.total_tokens,
                    max_tokens,
                    model,
                    node_id,
                },
            );
        }
        ExecutionEvent::AiInvokeFailed {
            node_id: _,
            label: _,
            error: _,
        } => {}
    }
}

fn format_phase_timed_message(phase: &str, label: &str, duration_ms: u64) -> String {
    let duration = if duration_ms >= 1000 {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    } else {
        format!("{duration_ms}ms")
    };
    format!("{phase}: {label} · {duration}")
}

fn find_tool_call_mut<'a>(
    state: &'a mut WorkflowRunState,
    node_id: &NodeId,
    tool_call_id: &str,
) -> Option<&'a mut ToolCallSummary> {
    state
        .tool_calls_by_node
        .entry(node_id.clone())
        .or_default()
        .iter_mut()
        .find(|call| call.tool_call_id == tool_call_id)
}

fn set_tool_streaming(
    state: &mut WorkflowRunState,
    node_id: &NodeId,
    tool_call_id: &str,
    streaming: bool,
) {
    if let Some(call) = find_tool_call_mut(state, node_id, tool_call_id) {
        call.streaming = streaming;
    }
}

fn abort_in_progress_tools(state: &mut WorkflowRunState) {
    for calls in state.tool_calls_by_node.values_mut() {
        for call in calls.iter_mut() {
            if matches!(
                call.status,
                ToolCallStatus::Running
                    | ToolCallStatus::Proposed
                    | ToolCallStatus::AwaitingApproval
            ) {
                call.status = ToolCallStatus::Aborted;
            }
        }
    }
}

fn running_node_snapshot(state: &WorkflowRunState) -> Option<(NodeId, String)> {
    state
        .status_by_node
        .iter()
        .find(|(_, status)| {
            matches!(
                status,
                AgentStatus::Started
                    | AgentStatus::RunningTool
                    | AgentStatus::AwaitingInput
                    | AgentStatus::AwaitingToolApproval
                    | AgentStatus::Queued
            )
        })
        .map(|(node_id, _)| {
            let label = state
                .run_trace
                .iter()
                .rev()
                .find(|entry| entry.node_id == *node_id)
                .map(|entry| entry.node_label.clone())
                .unwrap_or_else(|| node_id.0.clone());
            (node_id.clone(), label)
        })
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

fn add_awaiting_node(state: &mut WorkflowRunState, node_id: NodeId) {
    if !state.awaiting_node_ids.contains(&node_id) {
        state.awaiting_node_ids.push(node_id.clone());
    }
    if state.awaiting_node_id.is_none() {
        state.awaiting_node_id = Some(node_id);
    }
}

fn remove_awaiting_node(state: &mut WorkflowRunState, node_id: &NodeId) {
    state.awaiting_node_ids.retain(|id| id != node_id);
    if state.awaiting_node_id.as_ref() == Some(node_id) {
        state.awaiting_node_id = state.awaiting_node_ids.first().cloned();
    }
}

fn clear_node_awaiting_and_approvals(state: &mut WorkflowRunState, node_id: &NodeId) {
    remove_awaiting_node(state, node_id);
    state
        .pending_approvals
        .retain(|approval| approval.node_id != *node_id);
    if state.active_manual_node_id.as_ref() == Some(node_id) {
        state.active_manual_node_id = None;
    }
}

fn clear_node_session_focus(state: &mut WorkflowRunState, node_id: &NodeId) {
    clear_node_awaiting_and_approvals(state, node_id);
    state.active_tool_call_id = None;
}

fn record_node_failure(
    state: &mut WorkflowRunState,
    node_id: NodeId,
    label: String,
    error: String,
    deactivate_run: bool,
) {
    if deactivate_run {
        state.active = false;
    }
    clear_node_session_focus(state, &node_id);
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

fn remove_pending_approval(state: &mut WorkflowRunState, approval_id: &str) {
    state
        .pending_approvals
        .retain(|approval| approval.approval_id != approval_id);
}

/// Manual roots receive the same kickoff text via `record_user_input` when the UI
/// auto-flushes to the single awaiting node — skip duplicate chat entry here.
pub fn should_record_entrypoint_in_chat(workflow: &Workflow, root_id: &NodeId) -> bool {
    workflow
        .nodes
        .iter()
        .find(|node| node.id == *root_id)
        .is_some_and(|node| node.agent.auto_start)
}

pub fn record_entrypoint_message(state: &mut WorkflowRunState, node_id: &str, text: String) {
    let node_id = NodeId(node_id.to_string());
    state
        .chat_logs
        .entry(node_id)
        .or_default()
        .push(ChatMessage::text(ChatRole::User, text));
}

pub fn record_user_input(state: &mut WorkflowRunState, node_id: &str, text: String) {
    let node_id = NodeId(node_id.to_string());
    state
        .chat_logs
        .entry(node_id.clone())
        .or_default()
        .push(ChatMessage::text(ChatRole::User, text));
    remove_awaiting_node(state, &node_id);
    state.active_manual_node_id = None;
    state.status_by_node.insert(node_id, AgentStatus::Started);
}

fn restore_active_node_status(state: &mut WorkflowRunState, node_id: &NodeId) {
    if state.awaiting_node_ids.contains(node_id) || state.awaiting_node_id.as_ref() == Some(node_id)
    {
        state
            .status_by_node
            .insert(node_id.clone(), AgentStatus::AwaitingInput);
        return;
    }
    if state
        .pending_approvals
        .iter()
        .any(|approval| approval.node_id == *node_id)
    {
        state
            .status_by_node
            .insert(node_id.clone(), AgentStatus::AwaitingToolApproval);
        return;
    }
    state
        .status_by_node
        .insert(node_id.clone(), AgentStatus::Started);
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Node;

    #[test]
    fn manual_root_skips_entrypoint_chat_record() {
        let mut workflow = Workflow::new("w");
        let mut node = Node::agent("Root", 0.0, 0.0);
        node.agent.auto_start = false;
        workflow.nodes = vec![node.clone()];
        assert!(!should_record_entrypoint_in_chat(&workflow, &node.id));
    }

    #[test]
    fn auto_start_root_records_entrypoint_in_chat() {
        let mut workflow = Workflow::new("w");
        let node = Node::agent("Root", 0.0, 0.0);
        workflow.nodes = vec![node.clone()];
        assert!(should_record_entrypoint_in_chat(&workflow, &node.id));
    }

    #[test]
    fn ignored_error_does_not_stop_active_run() {
        use crate::run::state::WorkflowRunState;
        use engine::tools::{ToolCall, ToolTier};

        let workflow = Workflow::new("w");
        let mut state = WorkflowRunState::running_for_workflow(&workflow);
        state.pending_approvals.push(engine::PendingToolApproval {
            approval_id: "approval-1".to_string(),
            node_id: NodeId("plan".to_string()),
            node_label: "Plan".to_string(),
            tool_call: ToolCall {
                id: "call-1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({}),
            },
            tier: ToolTier::Read,
        });

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::Error(
                "ignored retry for node design: not in current interaction pause".to_string(),
            ),
        );

        assert!(state.active);
        assert_eq!(state.pending_approvals.len(), 1);
        assert!(state.last_error.is_none());
    }

    #[test]
    fn phase_timed_message_formats_milliseconds_and_seconds() {
        assert_eq!(
            format_phase_timed_message("ai_invoke", "Planner", 842),
            "ai_invoke: Planner · 842ms"
        );
        assert_eq!(
            format_phase_timed_message("tool", "search", 2400),
            "tool: search · 2.4s"
        );
    }
}
