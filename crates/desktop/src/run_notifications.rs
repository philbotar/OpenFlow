use orchestration::run::execution::ExecutionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunNotificationKind {
    NeedsInput,
    ToolApproval,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunNotification {
    pub kind: RunNotificationKind,
    pub title: String,
    pub body: String,
}

#[must_use]
pub fn notification_for_event(
    event: &ExecutionEvent,
    workflow_name: &str,
) -> Option<RunNotification> {
    let workflow_name = display_workflow_name(workflow_name);
    match event {
        ExecutionEvent::NodeAwaitingInput { label, .. } => Some(RunNotification {
            kind: RunNotificationKind::NeedsInput,
            title: "Workflow needs input".to_string(),
            body: format!("{workflow_name} is waiting for input at {label}."),
        }),
        ExecutionEvent::ToolApprovalRequested { request } => Some(RunNotification {
            kind: RunNotificationKind::ToolApproval,
            title: "Tool approval needed".to_string(),
            body: format!(
                "{} wants to run {} in {workflow_name}.",
                request.node_label, request.tool_call.name
            ),
        }),
        ExecutionEvent::Finished(report) => Some(RunNotification {
            kind: RunNotificationKind::Completed,
            title: "Workflow complete".to_string(),
            body: format!(
                "{workflow_name} completed {} {}.",
                report.outputs.len(),
                pluralize("node", report.outputs.len())
            ),
        }),
        ExecutionEvent::Error(message) => Some(RunNotification {
            kind: RunNotificationKind::Failed,
            title: "Workflow stopped with an error".to_string(),
            body: format!("{workflow_name} stopped: {message}"),
        }),
        ExecutionEvent::Aborted => Some(RunNotification {
            kind: RunNotificationKind::Failed,
            title: "Workflow stopped".to_string(),
            body: format!("{workflow_name} stopped before completing."),
        }),
        _ => None,
    }
}

fn display_workflow_name(workflow_name: &str) -> &str {
    let trimmed = workflow_name.trim();
    if trimmed.is_empty() {
        "Workflow"
    } else {
        trimmed
    }
}

fn pluralize(noun: &str, count: usize) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

pub fn show_run_notification<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    notification: &RunNotification,
) {
    use tauri_plugin_notification::NotificationExt;

    if let Err(error) = app
        .notification()
        .builder()
        .title(notification.title.clone())
        .body(notification.body.clone())
        .show()
    {
        log::warn!(
            "failed to show {:?} run notification: {error}",
            notification.kind
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestration::{
        NodeId, NodeRunOutput, PendingToolApproval, RunReport, RunTelemetry as ExecutionEvent,
        ToolCall, ToolTier, WorkflowId,
    };
    use serde_json::json;

    #[test]
    fn notifies_when_node_awaits_human_input() {
        let event = ExecutionEvent::NodeAwaitingInput {
            node_id: NodeId("node-1".to_string()),
            label: "Review plan".to_string(),
            context: "Please review the plan.".to_string(),
            is_initial: false,
        };

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::NeedsInput);
        assert_eq!(notification.title, "Workflow needs input");
        assert_eq!(
            notification.body,
            "Launch Flow is waiting for input at Review plan."
        );
    }

    #[test]
    fn notifies_when_tool_approval_is_requested() {
        let event = ExecutionEvent::ToolApprovalRequested {
            request: PendingToolApproval {
                approval_id: "approval-1".to_string(),
                node_id: NodeId("node-1".to_string()),
                node_label: "Implementer".to_string(),
                tool_call: ToolCall {
                    id: "tool-1".to_string(),
                    name: "bash".to_string(),
                    arguments: json!({ "cmd": "cargo test -p desktop" }),
                },
                tier: ToolTier::Write,
            },
        };

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::ToolApproval);
        assert_eq!(notification.title, "Tool approval needed");
        assert_eq!(
            notification.body,
            "Implementer wants to run bash in Launch Flow."
        );
    }

    #[test]
    fn notifies_when_workflow_finishes() {
        let event = ExecutionEvent::Finished(RunReport {
            workflow_id: WorkflowId("workflow-1".to_string()),
            outputs: vec![
                NodeRunOutput {
                    node_id: NodeId("node-1".to_string()),
                    output: json!({ "ok": true }),
                },
                NodeRunOutput {
                    node_id: NodeId("node-2".to_string()),
                    output: json!({ "ok": true }),
                },
            ],
            read_calls: 0,
            redundant_reads: 0,
            tokens_in: 0,
        });

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Completed);
        assert_eq!(notification.title, "Workflow complete");
        assert_eq!(notification.body, "Launch Flow completed 2 nodes.");
    }

    #[test]
    fn notifies_when_workflow_errors() {
        let event = ExecutionEvent::Error("provider request failed".to_string());

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Failed);
        assert_eq!(notification.title, "Workflow stopped with an error");
        assert_eq!(
            notification.body,
            "Launch Flow stopped: provider request failed"
        );
    }

    #[test]
    fn notifies_when_workflow_aborts() {
        let event = ExecutionEvent::Aborted;

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Failed);
        assert_eq!(notification.title, "Workflow stopped");
        assert_eq!(notification.body, "Launch Flow stopped before completing.");
    }

    #[test]
    fn ignores_non_attention_events() {
        let event = ExecutionEvent::NodeStarted {
            node_id: NodeId("node-1".to_string()),
            label: "Implementer".to_string(),
        };

        assert_eq!(notification_for_event(&event, "Launch Flow"), None);
    }
}
