mod interaction;
mod lifecycle;
mod setup;

use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointReason};
use engine::{review_completed_run, AiPort, EngineRunResult, NodeId, PostRunReview, RunError};
use std::collections::{HashMap, HashSet};
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
            break;
        }
        apply_pending_engine_reverts(
            &wiring.pending_engine_reverts,
            &mut wiring.engine,
            wiring.tool_port.tool_runner(),
        );
        let run_result = {
            let run = wiring
                .engine
                .run(&*wiring.ai_adapter, &wiring.tool_port, &cancel_token);
            tokio::pin!(run);
            let mut pending_mcp =
                HashMap::<String, crate::adapters::mcp::McpRunClientRequest>::new();
            loop {
                tokio::select! {
                    result = &mut run => {
                        for (_, request) in pending_mcp.drain() {
                            super::mcp_callbacks::cancel(request);
                        }
                        break result;
                    }
                    request = recv_mcp_client_request(&mut wiring.mcp_client_request_rx) => {
                        let Some(request) = request else {
                            wiring.mcp_client_request_rx = None;
                            continue;
                        };
                        let request_id = request.pending.request_id.clone();
                        send_or_log(
                            &event_tx,
                            ExecutionEvent::McpClientRequestCreated {
                                request: request.pending.clone(),
                            },
                        );
                        if let Some(replaced) = pending_mcp.insert(request_id, request) {
                            super::mcp_callbacks::cancel(replaced);
                        }
                    }
                    action = action_rx.recv(), if !pending_mcp.is_empty() => {
                        let Some(action) = action else {
                            cancel_token.cancel();
                            continue;
                        };
                        match action {
                            ExecutionAction::ResolveMcpClientRequest { request_id, decision } => {
                                let Some(request) = pending_mcp.remove(&request_id) else {
                                    log::warn!("ignored MCP client response {request_id}: request is not pending");
                                    continue;
                                };
                                let (pending, outcome) = super::mcp_callbacks::resolve(
                                    request,
                                    decision,
                                    &*wiring.review_ai,
                                    &wiring.workflow,
                                    &cancel_token,
                                )
                                .await;
                                send_or_log(
                                    &event_tx,
                                    ExecutionEvent::McpClientRequestResolved {
                                        request_id: pending.request_id,
                                        node_id: pending.node_id,
                                        outcome,
                                    },
                                );
                            }
                            ExecutionAction::Stop => {
                                cancel_token.cancel();
                            }
                            other => {
                                log::warn!("ignored run action while MCP client request is pending: {other:?}");
                            }
                        }
                    }
                }
            }
        };
        match run_result {
            EngineRunResult::NeedsInteraction {
                inputs,
                approvals,
                retryables,
            } => {
                let checkpoint_reason = if !approvals.is_empty() {
                    RunCheckpointReason::AwaitingToolApproval
                } else if !retryables.is_empty() {
                    RunCheckpointReason::AwaitingRetry
                } else {
                    RunCheckpointReason::AwaitingInput
                };
                // Stage the engine snapshot before emitting projection events. The
                // coordinator keeps it pending until every checkpointed pause item
                // has reached the projection.
                publish_checkpoint(
                    &mut wiring.engine,
                    &wiring.checkpoint_sink,
                    checkpoint_reason,
                );
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
                    break;
                }
            }
            EngineRunResult::Completed(mut report) => {
                let checkpoint = wiring.engine.prepare_stop_checkpoint();
                let review = tokio::select! {
                    () = cancel_token.cancelled() => PostRunReview {
                        suggestions: Vec::new(),
                        error: Some("Post-run review was cancelled.".to_string()),
                    },
                    review = review_completed_run(
                        &*wiring.review_ai,
                        &wiring.workflow,
                        &checkpoint,
                        &report,
                    ) => review,
                };
                report.suggestions = review.suggestions;
                report.suggestions_error = review.error;
                *wiring.checkpoint_sink.lock() = Some(PendingRunCheckpoint {
                    reason: RunCheckpointReason::Completed,
                    engine: checkpoint,
                });
                send_or_log(&event_tx, ExecutionEvent::Finished(report));
                break;
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
                        break;
                    }
                    other => {
                        send_or_log(&event_tx, ExecutionEvent::Error(other.to_string()));
                        break;
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
                break;
            }
        }
    }

    if let Err(error) = wiring.tool_port.tool_runner().close_mcp_clients().await {
        log::warn!("failed to close MCP clients at run end: {error}");
    }
}

async fn recv_mcp_client_request(
    receiver: &mut Option<
        tokio::sync::mpsc::UnboundedReceiver<crate::adapters::mcp::McpRunClientRequest>,
    >,
) -> Option<crate::adapters::mcp::McpRunClientRequest> {
    match receiver {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}
