use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointReason};
use crate::tools::ToolRunner;
use engine::{
    EngineAwaitApproval, EngineAwaitInput, EngineRetryableNode, InteractiveEngine, NodeId,
    PendingToolApproval, ToolCall, ToolTier, Workflow,
};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

use super::super::{send_or_log, ExecutionAction, ExecutionEvent};
use super::lifecycle::snapshot_and_abort;

struct ApprovalRequestEmit<'a> {
    event_tx: &'a UnboundedSender<ExecutionEvent>,
    workflow: &'a Workflow,
    tool_runner: &'a ToolRunner,
    approval_id: &'a str,
    node_id: &'a NodeId,
    label: &'a str,
    tool_calls: &'a [ToolCall],
    proposed_tool_calls: &'a mut HashSet<String>,
}

/// Emit `ToolCallProposed` once per tool-call id, then a single `ToolApprovalRequested`
/// carrying the first call's tier (used by the approval UI).
fn emit_approval_request(ctx: ApprovalRequestEmit<'_>) {
    let mut approval_request = None;
    for tool_call in ctx.tool_calls {
        if ctx.proposed_tool_calls.insert(tool_call.id.clone()) {
            send_or_log(
                ctx.event_tx,
                ExecutionEvent::ToolCallProposed {
                    node_id: ctx.node_id.clone(),
                    label: ctx.label.to_string(),
                    tool_call: tool_call.clone(),
                },
            );
        }

        let tier = ctx
            .tool_runner
            .registry()
            .get(&tool_call.name)
            .map(|registered| registered.definition.tier)
            .unwrap_or_else(|_| {
                ctx.workflow
                    .nodes
                    .iter()
                    .find(|node| node.id == *ctx.node_id)
                    .map(|node| engine::tool_tier_for_call(&node.agent.tools, &tool_call.name))
                    .unwrap_or(ToolTier::Write)
            });

        if approval_request.is_none() {
            approval_request = Some(PendingToolApproval {
                approval_id: ctx.approval_id.to_string(),
                node_id: ctx.node_id.clone(),
                node_label: ctx.label.to_string(),
                tool_call: tool_call.clone(),
                tier,
            });
        }
    }

    if let Some(request) = approval_request {
        send_or_log(
            ctx.event_tx,
            ExecutionEvent::ToolApprovalRequested { request },
        );
    }
}

/// Tracks which interaction items the current pause still expects before `engine.run()` can resume.
pub(super) struct InteractionPause {
    inputs: HashSet<NodeId>,
    approvals: HashSet<String>,
    retryables: HashSet<NodeId>,
}

impl InteractionPause {
    fn new(
        inputs: &[EngineAwaitInput],
        approvals: &[EngineAwaitApproval],
        retryables: &[EngineRetryableNode],
    ) -> Self {
        Self {
            inputs: inputs.iter().map(|input| input.node_id.clone()).collect(),
            approvals: approvals
                .iter()
                .map(|approval| approval.approval_id.clone())
                .collect(),
            retryables: retryables
                .iter()
                .map(|retryable| retryable.node_id.clone())
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.approvals.is_empty() && self.retryables.is_empty()
    }

    /// Priority: approval > retry > input — matches what blocks progress first.
    pub(super) fn checkpoint_reason(&self) -> RunCheckpointReason {
        if !self.approvals.is_empty() {
            RunCheckpointReason::AwaitingToolApproval
        } else if !self.retryables.is_empty() {
            RunCheckpointReason::AwaitingRetry
        } else {
            RunCheckpointReason::AwaitingInput
        }
    }
}

pub(super) struct InteractionApprovalContext {
    tool_calls: HashMap<String, Vec<ToolCall>>,
    nodes: HashMap<String, NodeId>,
}

/// Project engine pause state into UI events. Returns the pending sets and approval context
/// consumed by [`await_interaction_actions`].
#[allow(
    clippy::too_many_arguments,
    reason = "pause projection threads engine state through the execution event channel"
)]
pub(super) fn emit_interaction_pause(
    inputs: &[EngineAwaitInput],
    approvals: &[EngineAwaitApproval],
    retryables: &[EngineRetryableNode],
    engine: &InteractiveEngine,
    event_tx: &UnboundedSender<ExecutionEvent>,
    workflow: &Workflow,
    tool_runner: &ToolRunner,
    proposed_tool_calls: &mut HashSet<String>,
    emitted_retryables: &mut HashSet<(NodeId, u8)>,
) -> (InteractionPause, InteractionApprovalContext) {
    for input in inputs {
        if input.is_initial {
            send_or_log(
                event_tx,
                ExecutionEvent::NodeQueued {
                    node_id: input.node_id.clone(),
                    label: input.label.clone(),
                },
            );
        }
        send_or_log(
            event_tx,
            ExecutionEvent::NodeAwaitingInput {
                node_id: input.node_id.clone(),
                label: input.label.clone(),
                context: input.context.clone(),
                is_initial: input.is_initial,
            },
        );
    }

    let mut approval_ctx = InteractionApprovalContext {
        tool_calls: HashMap::new(),
        nodes: HashMap::new(),
    };
    for approval in approvals {
        approval_ctx
            .tool_calls
            .insert(approval.approval_id.clone(), approval.tool_calls.clone());
        approval_ctx
            .nodes
            .insert(approval.approval_id.clone(), approval.node_id.clone());
        emit_approval_request(ApprovalRequestEmit {
            event_tx,
            workflow,
            tool_runner,
            approval_id: &approval.approval_id,
            node_id: &approval.node_id,
            label: &approval.label,
            tool_calls: &approval.tool_calls,
            proposed_tool_calls,
        });
    }

    for retryable in retryables {
        // One error/interrupt event per node attempt — retries bump the attempt counter.
        let attempt = engine.model_attempt_for_node(&retryable.node_id);
        if !emitted_retryables.insert((retryable.node_id.clone(), attempt)) {
            continue;
        }
        if retryable.interrupted {
            send_or_log(
                event_tx,
                ExecutionEvent::NodeInterrupted {
                    node_id: retryable.node_id.clone(),
                    label: retryable.label.clone(),
                },
            );
        } else {
            send_or_log(
                event_tx,
                ExecutionEvent::NodeErrored {
                    node_id: retryable.node_id.clone(),
                    label: retryable.label.clone(),
                    error: retryable.error.clone(),
                },
            );
        }
    }

    (
        InteractionPause::new(inputs, approvals, retryables),
        approval_ctx,
    )
}

/// Block until every pending input, approval, and retryable in `pause` is resolved.
/// Returns `false` when the run should exit (cancel, stop, channel closed, or fatal error).
#[allow(
    clippy::too_many_arguments,
    reason = "interaction loop coordinates engine, actions, checkpoints, and cancellation"
)]
pub(super) async fn await_interaction_actions(
    pause: &mut InteractionPause,
    approval_ctx: &InteractionApprovalContext,
    engine: &mut InteractiveEngine,
    action_rx: &mut UnboundedReceiver<ExecutionAction>,
    event_tx: &UnboundedSender<ExecutionEvent>,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    cancel_token: &CancellationToken,
    aborted_emitted: &Mutex<bool>,
) -> bool {
    while !pause.is_empty() {
        if cancel_token.is_cancelled() {
            snapshot_and_abort(engine, checkpoint_sink, event_tx, aborted_emitted);
            return false;
        }

        let Some(action) = action_rx.recv().await else {
            log::warn!("execution action channel closed; aborting run");
            snapshot_and_abort(engine, checkpoint_sink, event_tx, aborted_emitted);
            return false;
        };

        match action {
            ExecutionAction::Stop => {
                snapshot_and_abort(engine, checkpoint_sink, event_tx, aborted_emitted);
                return false;
            }
            ExecutionAction::ProvideInput { node_id, text } => {
                // Stale actions (wrong pause) are logged and skipped — UI may race with resume.
                if !pause.inputs.contains(&node_id) {
                    send_or_log(
                        event_tx,
                        ExecutionEvent::Error(format!(
                            "ignored input for node {node_id}: not in current interaction pause"
                        )),
                    );
                    continue;
                }
                if let Err(error) = engine.on_human_input(&node_id, &text) {
                    send_or_log(event_tx, ExecutionEvent::Error(error.to_string()));
                    return false;
                }
                pause.inputs.remove(&node_id);
            }
            ExecutionAction::ResolveApproval {
                approval_id,
                allow,
                reason,
            } => {
                if !pause.approvals.contains(&approval_id) {
                    send_or_log(
                        event_tx,
                        ExecutionEvent::Error(format!(
                            "ignored approval {approval_id}: not in current interaction pause"
                        )),
                    );
                    continue;
                }
                let Some(node_id) = engine
                    .pending_tool_batch_node(&approval_id)
                    .or_else(|| approval_ctx.nodes.get(&approval_id).cloned())
                else {
                    send_or_log(
                        event_tx,
                        ExecutionEvent::Error(format!(
                            "approval {approval_id}: node id missing from engine and pause context"
                        )),
                    );
                    return false;
                };
                if let Err(error) = engine.on_tool_decision(&approval_id, allow, reason.as_deref())
                {
                    send_or_log(event_tx, ExecutionEvent::Error(error.to_string()));
                    return false;
                }
                if let Some(tool_calls) = approval_ctx.tool_calls.get(&approval_id) {
                    for tool_call in tool_calls {
                        if allow {
                            send_or_log(
                                event_tx,
                                ExecutionEvent::ToolApproved {
                                    approval_id: approval_id.clone(),
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                },
                            );
                        } else {
                            send_or_log(
                                event_tx,
                                ExecutionEvent::ToolDenied {
                                    approval_id: approval_id.clone(),
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    reason: reason.clone().unwrap_or_else(|| {
                                        "Tool call denied by the user.".to_string()
                                    }),
                                },
                            );
                        }
                    }
                }
                pause.approvals.remove(&approval_id);
            }
            ExecutionAction::RetryNode { node_id } => {
                if !pause.retryables.contains(&node_id) {
                    send_or_log(
                        event_tx,
                        ExecutionEvent::Error(format!(
                            "ignored retry for node {node_id}: not in current interaction pause"
                        )),
                    );
                    continue;
                }
                if let Err(error) = engine.retry_node(&node_id) {
                    send_or_log(event_tx, ExecutionEvent::Error(error.to_string()));
                    return false;
                }
                pause.retryables.remove(&node_id);
            }
        }
    }
    true
}
