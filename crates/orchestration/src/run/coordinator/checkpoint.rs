//! Durable run checkpoint persistence helpers.

use crate::error::BackendError;
use crate::run::persistence::{
    PendingRunCheckpoint, RunCheckpointPayload, RunCheckpointReason, RunStatus, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::WorkflowRunState;
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
