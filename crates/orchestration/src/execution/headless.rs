use crate::state::WorkflowRunState;
use domain::CallableAgent;
use domain::{AiPort, Workflow};
use std::collections::{BTreeMap, VecDeque};

use super::drive::drive_interactive_workflow;
use super::events::{apply_event_to_run_state, record_user_input};
use super::{
    resolve_execution_cwd, ApprovalResponse, ExecutionAction, ExecutionEvent, ManualInput,
    WorkflowExecutionError, WorkflowRunSnapshot,
};
use tokio_util::sync::CancellationToken;

/// # Errors
/// Returns an error if the workflow execution fails.
pub async fn run_workflow_headless<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    ai: A,
    manual_inputs: Vec<ManualInput>,
    approvals: Vec<ApprovalResponse>,
    agent_snapshots: BTreeMap<String, CallableAgent>,
) -> Result<WorkflowRunSnapshot, WorkflowExecutionError>
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let execution_cwd = resolve_execution_cwd(None).map_err(WorkflowExecutionError::Execution)?;
    let cancel_token = CancellationToken::new();
    let handle = tokio::spawn(drive_interactive_workflow(
        workflow.clone(),
        entrypoint,
        execution_cwd,
        ai,
        event_tx,
        action_rx,
        agent_snapshots,
        cancel_token,
    ));
    let mut manual_inputs = VecDeque::from(manual_inputs);
    let mut approvals = VecDeque::from(approvals);
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    while let Some(event) = event_rx.recv().await {
        let awaiting_input = matches!(
            &event,
            ExecutionEvent::NodeAwaitingInput { node_id, .. }
                if manual_inputs
                    .front()
                    .map(|next| next.node_id == *node_id)
                    .unwrap_or(false)
        );
        let awaiting_approval = matches!(
            &event,
            ExecutionEvent::ToolApprovalRequested { request }
                if approvals
                    .front()
                    .map(|next| next.approval_id.is_empty() || next.approval_id == request.approval_id)
                    .unwrap_or(false)
        );

        apply_event_to_run_state(&workflow, &mut state, event);

        if awaiting_input {
            let input = manual_inputs.pop_front().unwrap();
            action_tx
                .send(ExecutionAction::ProvideInput(input.text.clone()))
                .map_err(|_| WorkflowExecutionError::Execution("run channel closed".to_string()))?;
            record_user_input(&mut state, &input.node_id, input.text);
        }
        if awaiting_approval {
            let approval = approvals.pop_front().unwrap();
            let approval_id = if approval.approval_id.is_empty() {
                state
                    .pending_approvals
                    .first()
                    .map(|item| item.approval_id.clone())
                    .unwrap_or_default()
            } else {
                approval.approval_id
            };
            action_tx
                .send(ExecutionAction::ResolveApproval {
                    approval_id,
                    allow: approval.allow,
                })
                .map_err(|_| WorkflowExecutionError::Execution("run channel closed".to_string()))?;
            state.pending_approvals.clear();
        }

        if !state.active {
            break;
        }

        if let Some(node_id) = state.awaiting_node_id.clone() {
            if manual_inputs.front().map(|item| item.node_id.clone()) != Some(node_id.clone()) {
                return Err(WorkflowExecutionError::MissingManualInput(node_id));
            }
        }
        if let Some(approval) = state.pending_approvals.first() {
            let matches_next = approvals.front().is_some_and(|item| {
                item.approval_id.is_empty() || item.approval_id == approval.approval_id
            });
            if !matches_next {
                return Err(WorkflowExecutionError::MissingApproval(
                    approval.approval_id.clone(),
                ));
            }
        }
    }

    handle.abort();
    if let Some(error) = state.last_error.clone() {
        return Err(WorkflowExecutionError::Execution(error));
    }
    let report = state
        .last_report
        .clone()
        .ok_or_else(|| WorkflowExecutionError::Execution("run did not finish".to_string()))?;
    Ok(WorkflowRunSnapshot {
        report,
        run_trace: state.run_trace,
        chat_logs: state.chat_logs,
        outputs: state.outputs,
        pending_approvals: state.pending_approvals,
        tool_calls_by_node: state.tool_calls_by_node,
        tool_artifacts: state.tool_artifacts,
    })
}
