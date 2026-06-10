use crate::run::state::WorkflowRunState;
use engine::CallableAgent;
use engine::{AiPort, Workflow};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use super::drive::drive_interactive_workflow;
use super::events::{apply_event_to_run_state, record_user_input};
use super::{
    resolve_execution_cwd, ApprovalResponse, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, ManualInput, NodeInterrupts, WorkflowExecutionError,
    WorkflowRunSnapshot,
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
    execution_cwd: Option<PathBuf>,
) -> Result<WorkflowRunSnapshot, WorkflowExecutionError>
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let execution_cwd = match execution_cwd {
        Some(path) => path.canonicalize().map_err(|error| {
            WorkflowExecutionError::Execution(format!(
                "execution folder is not a valid directory: {error}"
            ))
        })?,
        None => resolve_execution_cwd(None).map_err(WorkflowExecutionError::Execution)?,
    };
    let cancel_token = CancellationToken::new();
    let snapshot_store =
        Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new());
    let pending_engine_reverts = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let node_interrupts: NodeInterrupts = Arc::new(parking_lot::Mutex::new(BTreeMap::new()));
    let handle = tokio::spawn(drive_interactive_workflow(
        InteractiveWorkflowRunParams {
            workflow: workflow.clone(),
            entrypoint,
            execution_cwd,
            ai,
            agent_snapshots,
            snapshot_store,
            lsp: crate::lsp::LspSettings::from_env(),
            pending_engine_reverts,
            node_interrupts,
        },
        event_tx,
        action_rx,
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
                    .iter()
                    .any(|next| next.node_id == *node_id)
        );
        let awaiting_approval = matches!(
            &event,
            ExecutionEvent::ToolApprovalRequested { request }
                if approvals.iter().any(|next| {
                    next.approval_id.is_empty() || next.approval_id == request.approval_id
                })
        );

        apply_event_to_run_state(&workflow, &mut state, event.clone());

        if matches!(
            &event,
            ExecutionEvent::NodeErrored { .. } | ExecutionEvent::NodeInterrupted { .. }
        ) && state.active
        {
            let node_id = match &event {
                ExecutionEvent::NodeErrored { node_id, .. }
                | ExecutionEvent::NodeInterrupted { node_id, .. } => node_id.clone(),
                _ => unreachable!(),
            };
            return Err(WorkflowExecutionError::MissingRetry(node_id));
        }

        if let ExecutionEvent::NodeAwaitingInput { node_id, .. } = &event {
            if awaiting_input {
                let position = manual_inputs
                    .iter()
                    .position(|next| next.node_id == *node_id)
                    .expect("awaiting input matched queue");
                let input = manual_inputs.remove(position).expect("input queue entry");
                action_tx
                    .send(ExecutionAction::ProvideInput {
                        node_id: input.node_id.clone(),
                        text: input.text.clone(),
                    })
                    .map_err(|_| {
                        WorkflowExecutionError::Execution("run channel closed".to_string())
                    })?;
                record_user_input(&mut state, &input.node_id, input.text);
            }
        }
        if let ExecutionEvent::ToolApprovalRequested { request } = &event {
            if awaiting_approval {
                let position = approvals
                    .iter()
                    .position(|next| {
                        next.approval_id.is_empty() || next.approval_id == request.approval_id
                    })
                    .expect("awaiting approval matched queue");
                let approval = approvals.remove(position).expect("approval queue entry");
                let approval_id = if approval.approval_id.is_empty() {
                    request.approval_id.clone()
                } else {
                    approval.approval_id
                };
                action_tx
                    .send(ExecutionAction::ResolveApproval {
                        approval_id: approval_id.clone(),
                        allow: approval.allow,
                    })
                    .map_err(|_| {
                        WorkflowExecutionError::Execution("run channel closed".to_string())
                    })?;
                state
                    .pending_approvals
                    .retain(|pending| pending.approval_id != approval_id);
            }
        }

        if !state.active {
            break;
        }

        for node_id in &state.awaiting_node_ids {
            if !manual_inputs.iter().any(|item| item.node_id == *node_id) {
                return Err(WorkflowExecutionError::MissingManualInput(node_id.clone()));
            }
        }
        for pending in &state.pending_approvals {
            let matches_next = approvals
                .iter()
                .any(|item| item.approval_id.is_empty() || item.approval_id == pending.approval_id);
            if !matches_next {
                return Err(WorkflowExecutionError::MissingApproval(
                    pending.approval_id.clone(),
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
        changed_files: state.changed_files,
        edit_batches: state.edit_batches,
    })
}
