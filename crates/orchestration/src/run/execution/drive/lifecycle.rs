use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointReason};
use engine::{EditBatch, InteractiveEngine};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::tools::ToolRunner;

use super::super::{abort_run, ExecutionEvent};

/// Apply editor undo batches that arrived while the run was paused.
/// Bumps the tool-runner cache epoch so subsequent tool reads see reverted files.
pub(super) fn apply_pending_engine_reverts(
    pending: &Arc<parking_lot::Mutex<Vec<EditBatch>>>,
    engine: &mut InteractiveEngine,
    tool_runner: &ToolRunner,
) {
    let batches = pending.lock().drain(..).collect::<Vec<_>>();
    if !batches.is_empty() {
        tool_runner.bump_cache_epoch();
    }
    for batch in batches {
        engine.revert_file_changes_for_batch(&batch.batch_id, &batch.node_id);
        crate::tools::edit::batch::sync_hashline_snapshots_after_revert(
            tool_runner.cwd(),
            tool_runner.snapshot_store().as_ref(),
            &batch,
        );
    }
}

/// Snapshot engine state into `checkpoint_sink` for coordinator persistence.
pub(super) fn publish_checkpoint(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    reason: RunCheckpointReason,
) {
    *checkpoint_sink.lock() = Some(PendingRunCheckpoint {
        reason,
        engine: engine.prepare_stop_checkpoint(),
    });
}

/// Checkpoint as user-stopped, then emit a single `Aborted` event.
pub(super) fn snapshot_and_abort(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    event_tx: &tokio::sync::mpsc::UnboundedSender<ExecutionEvent>,
    aborted_emitted: &Mutex<bool>,
) {
    publish_checkpoint(engine, checkpoint_sink, RunCheckpointReason::UserStopped);
    abort_run(event_tx, aborted_emitted);
}
