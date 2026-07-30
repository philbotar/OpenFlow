//! Durable run checkpoint persistence helpers.

use crate::error::BackendError;
use crate::run::persistence::{
    PendingRunCheckpoint, RunCheckpointPayload, RunCheckpointReason, RunStatus, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::{AgentStatus, WorkflowRunState};
use chrono::Utc;

pub(super) fn load_replay_projection(
    run_store: &dyn RunCheckpointStore,
    roots: &[RunStoreRoot],
    run_id: &str,
) -> Result<WorkflowRunState, BackendError> {
    let (root, _) = run_store
        .load_record(roots, run_id)?
        .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
    let checkpoint = run_store
        .load_latest_checkpoint(&root, run_id)?
        .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
    Ok(checkpoint.projection.into_replay_projection())
}

pub(crate) fn status_for_checkpoint(reason: RunCheckpointReason) -> RunStatus {
    match reason {
        RunCheckpointReason::Started => RunStatus::Running,
        RunCheckpointReason::AwaitingInput
        | RunCheckpointReason::AwaitingToolApproval
        | RunCheckpointReason::AwaitingRetry => RunStatus::Paused,
        RunCheckpointReason::UserStopped => RunStatus::Stopped,
        RunCheckpointReason::Completed => RunStatus::Completed,
        RunCheckpointReason::Failed => RunStatus::Failed,
    }
}

pub(super) fn next_checkpoint_seq(
    store: &dyn RunCheckpointStore,
    root: &RunStoreRoot,
    run_id: &str,
) -> Result<u32, BackendError> {
    Ok(store
        .load_latest_checkpoint(root, run_id)?
        .map_or(1, |payload| payload.seq.saturating_add(1)))
}

pub(super) fn persist_pending_checkpoint(
    run_store: &dyn RunCheckpointStore,
    root: &RunStoreRoot,
    run_id: &str,
    projection: &WorkflowRunState,
    pending: PendingRunCheckpoint,
) -> Result<(), BackendError> {
    let now_ms = Utc::now().timestamp_millis();
    let reason = pending.reason;
    let payload = RunCheckpointPayload {
        seq: next_checkpoint_seq(run_store, root, run_id)?,
        created_at_ms: now_ms,
        reason,
        engine: pending.engine,
        projection: projection.clone(),
    };
    run_store.append_checkpoint(root, run_id, &payload)?;
    run_store.update_status(root, run_id, status_for_checkpoint(reason), now_ms)?;
    Ok(())
}

/// Return true once FIFO telemetry has fully projected the staged engine checkpoint.
///
/// The engine snapshot is staged before its pause/terminal telemetry. Waiting for every
/// checkpointed interaction item prevents an early event from persisting a partial UI projection.
pub(super) fn projection_ready_for_checkpoint(
    pending: &PendingRunCheckpoint,
    projection: &WorkflowRunState,
) -> bool {
    match pending.reason {
        RunCheckpointReason::Started => true,
        RunCheckpointReason::UserStopped
        | RunCheckpointReason::Completed
        | RunCheckpointReason::Failed => !projection.active,
        RunCheckpointReason::AwaitingInput
        | RunCheckpointReason::AwaitingToolApproval
        | RunCheckpointReason::AwaitingRetry => {
            let inputs_ready = pending
                .engine
                .awaiting_nodes
                .iter()
                .all(|node_id| projection.awaiting_node_ids.contains(node_id));
            let approvals_ready = pending
                .engine
                .pending_tool_batches
                .values()
                .filter(|batch| batch.requires_approval)
                .all(|batch| {
                    projection
                        .pending_approvals
                        .iter()
                        .any(|approval| approval.approval_id == batch.approval_id)
                });
            let interruptions_ready = pending.engine.interrupted_nodes.iter().all(|node_id| {
                projection.status_by_node.get(node_id) == Some(&AgentStatus::Interrupted)
            });
            let failures_ready = pending.engine.failed_nodes.keys().all(|node_id| {
                projection.status_by_node.get(node_id) == Some(&AgentStatus::Failed)
            });
            inputs_ready && approvals_ready && interruptions_ready && failures_ready
        }
    }
}
