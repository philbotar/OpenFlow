mod interaction;
mod lifecycle;
mod setup;

use crate::run::persistence::RunCheckpointReason;
use engine::{AiPort, EngineRunResult, NodeId, RunError};
use std::collections::HashSet;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

use interaction::{await_interaction_actions, emit_interaction_pause};
use lifecycle::{apply_pending_engine_reverts, publish_checkpoint, snapshot_and_abort};
use setup::wire_run;

use super::{send_or_log, ExecutionAction, ExecutionEvent, InteractiveWorkflowRunParams};

pub use setup::{new_artifact_root, new_in_memory_snapshot_store};

/// Host loop for an interactive workflow run — the only place `InteractiveEngine` is driven.
///
/// Wires engine ports (AI, tools, LSP), then loops:
/// 1. `engine.run()` until it completes, fails, cancels, or pauses for human interaction
/// 2. On pause: emit UI events, persist a checkpoint, block on `action_rx` until every pending
///    input/approval/retry is resolved
/// 3. Resume from step 1
pub(super) async fn drive_interactive_workflow<A>(
    params: InteractiveWorkflowRunParams<A>,
    event_tx: UnboundedSender<ExecutionEvent>,
    mut action_rx: UnboundedReceiver<ExecutionAction>,
    cancel_token: CancellationToken,
) where
    A: AiPort + Send + Sync + 'static,
{
    let mut wiring = match wire_run(params, event_tx.clone(), cancel_token.clone()).await {
        Ok(wiring) => wiring,
        Err(error) => {
            send_or_log(&event_tx, ExecutionEvent::Error(error));
            return;
        }
    };

    let mut proposed_tool_calls: HashSet<String> = HashSet::new();
    let mut emitted_retryables: HashSet<(NodeId, u8)> = HashSet::new();
    let aborted_emitted = wiring.aborted_emitted.clone();

    loop {
        if cancel_token.is_cancelled() {
            snapshot_and_abort(
                &mut wiring.engine,
                &wiring.checkpoint_sink,
                &event_tx,
                aborted_emitted.as_ref(),
            );
            return;
        }
        apply_pending_engine_reverts(
            &wiring.pending_engine_reverts,
            &mut wiring.engine,
            wiring.tool_port.tool_runner(),
        );
        let run_result = wiring
            .engine
            .run(&*wiring.ai_adapter, &wiring.tool_port, &cancel_token)
            .await;
        match run_result {
            EngineRunResult::NeedsInteraction {
                inputs,
                approvals,
                retryables,
            } => {
                let (mut pause, approval_ctx) = emit_interaction_pause(
                    &inputs,
                    &approvals,
                    &retryables,
                    &wiring.engine,
                    &event_tx,
                    &wiring.workflow,
                    wiring.tool_port.tool_runner(),
                    &mut proposed_tool_calls,
                    &mut emitted_retryables,
                );
                publish_checkpoint(
                    &mut wiring.engine,
                    &wiring.checkpoint_sink,
                    pause.checkpoint_reason(),
                );
                if !await_interaction_actions(
                    &mut pause,
                    &approval_ctx,
                    &mut wiring.engine,
                    &mut action_rx,
                    &event_tx,
                    &wiring.checkpoint_sink,
                    &cancel_token,
                    aborted_emitted.as_ref(),
                )
                .await
                {
                    return;
                }
            }
            EngineRunResult::Completed(report) => {
                publish_checkpoint(
                    &mut wiring.engine,
                    &wiring.checkpoint_sink,
                    RunCheckpointReason::Completed,
                );
                send_or_log(&event_tx, ExecutionEvent::Finished(report));
                return;
            }
            EngineRunResult::Failed(error) => {
                publish_checkpoint(
                    &mut wiring.engine,
                    &wiring.checkpoint_sink,
                    RunCheckpointReason::Failed,
                );
                match error {
                    RunError::NodeFailed { node_id, kind } => {
                        let label = wiring
                            .workflow
                            .nodes
                            .iter()
                            .find(|node| node.id == node_id)
                            .map(|node| node.label.clone())
                            .unwrap_or_else(|| node_id.to_string());
                        send_or_log(
                            &event_tx,
                            ExecutionEvent::NodeFailed {
                                node_id,
                                label,
                                error: kind.to_string(),
                            },
                        );
                        return;
                    }
                    other => {
                        send_or_log(&event_tx, ExecutionEvent::Error(other.to_string()));
                        return;
                    }
                }
            }
            EngineRunResult::Cancelled => {
                snapshot_and_abort(
                    &mut wiring.engine,
                    &wiring.checkpoint_sink,
                    &event_tx,
                    aborted_emitted.as_ref(),
                );
                return;
            }
        }
    }
}
