use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner};
use async_trait::async_trait;
use engine::{
    filter_tool_turn_assistant_message, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, ChatRole, EditBatch,
    EngineRunResult, InteractiveEngine, NodeId, PendingToolApproval, ToolCall, ToolTier, Workflow,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::tool_port::ToolPortImpl;
use super::{ExecutionAction, ExecutionEvent, InteractiveWorkflowRunParams};

pub(super) async fn drive_interactive_workflow<A>(
    params: InteractiveWorkflowRunParams<A>,
    event_tx: UnboundedSender<ExecutionEvent>,
    mut action_rx: UnboundedReceiver<ExecutionAction>,
    cancel_token: CancellationToken,
) where
    A: AiPort + Send + Sync + 'static,
{
    let InteractiveWorkflowRunParams {
        workflow,
        entrypoint,
        execution_cwd,
        ai,
        agent_snapshots,
        snapshot_store,
        lsp,
        pending_engine_reverts,
    } = params;
    let mut engine = match InteractiveEngine::new(workflow.clone(), entrypoint) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };

    let tool_registry = ToolRegistry::new();
    let artifact_root = std::env::temp_dir().join(format!("openflow-run-{}", Uuid::new_v4()));
    let artifacts = match ArtifactStore::new(artifact_root) {
        Ok(store) => store,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };
    let tool_runner = Arc::new(ToolRunner::new(
        tool_registry,
        execution_cwd,
        artifacts,
        cancel_token.clone(),
        snapshot_store,
        lsp,
    ));
    let workflow = Arc::new(workflow);
    let ai = Arc::new(ai);
    let tool_port = ToolPortImpl::new(
        Arc::clone(&tool_runner),
        Arc::clone(&workflow),
        Arc::new(agent_snapshots),
        Arc::clone(&ai),
        cancel_token.clone(),
        event_tx.clone(),
    );
    let telemetry_ai = TelemetryAiPort {
        inner: ai,
        event_tx: event_tx.clone(),
    };
    let mut proposed_tool_calls: HashSet<String> = HashSet::new();
    let mut aborted_emitted = false;

    loop {
        if cancel_token.is_cancelled() {
            abort_run(&event_tx, &mut aborted_emitted);
            return;
        }
        apply_pending_engine_reverts(
            &pending_engine_reverts,
            &mut engine,
            tool_port.tool_runner(),
        );
        let run_result = engine.run(&telemetry_ai, &tool_port, &cancel_token).await;
        match run_result {
            EngineRunResult::NeedsInput {
                node_id,
                label,
                context,
                is_initial,
            } => {
                let awaiting_node_id = node_id.clone();
                if is_initial {
                    let _ = event_tx.send(ExecutionEvent::NodeQueued {
                        node_id: node_id.clone(),
                        label: label.clone(),
                    });
                }
                let _ = event_tx.send(ExecutionEvent::NodeAwaitingInput {
                    node_id,
                    label,
                    context,
                    is_initial,
                });
                let Some(text) = wait_for_input(
                    &mut action_rx,
                    &cancel_token,
                    &event_tx,
                    &mut aborted_emitted,
                )
                .await
                else {
                    return;
                };
                if let Err(error) = engine.on_human_input(&awaiting_node_id, &text) {
                    let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                    return;
                }
            }
            EngineRunResult::NeedsApproval {
                approval_id,
                node_id,
                label,
                tool_calls,
            } => {
                emit_approval_request(ApprovalRequestEmit {
                    event_tx: &event_tx,
                    workflow: &workflow,
                    tool_runner: tool_port.tool_runner(),
                    approval_id: &approval_id,
                    node_id: &node_id,
                    label: &label,
                    tool_calls: &tool_calls,
                    proposed_tool_calls: &mut proposed_tool_calls,
                });
                let Some(approved) = wait_for_approval(
                    &mut action_rx,
                    &approval_id,
                    &cancel_token,
                    &event_tx,
                    &mut aborted_emitted,
                )
                .await
                else {
                    return;
                };
                if let Err(error) = engine.on_tool_decision(&approval_id, approved) {
                    let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                    return;
                }
                for tool_call in &tool_calls {
                    if approved {
                        let _ = event_tx.send(ExecutionEvent::ToolApproved {
                            approval_id: approval_id.clone(),
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                        });
                    } else {
                        let _ = event_tx.send(ExecutionEvent::ToolDenied {
                            approval_id: approval_id.clone(),
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            reason: "denied by user".to_string(),
                        });
                    }
                }
            }
            EngineRunResult::Completed(report) => {
                let _ = event_tx.send(ExecutionEvent::Finished(report));
                return;
            }
            EngineRunResult::Failed(error) => {
                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                return;
            }
            EngineRunResult::Cancelled => {
                abort_run(&event_tx, &mut aborted_emitted);
                return;
            }
        }
    }
}

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

fn emit_approval_request(ctx: ApprovalRequestEmit<'_>) {
    let mut approval_request = None;
    for tool_call in ctx.tool_calls {
        if ctx.proposed_tool_calls.insert(tool_call.id.clone()) {
            let _ = ctx.event_tx.send(ExecutionEvent::ToolCallProposed {
                node_id: ctx.node_id.clone(),
                label: ctx.label.to_string(),
                tool_call: tool_call.clone(),
            });
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
                node_id: ctx.node_id.to_string(),
                node_label: ctx.label.to_string(),
                tool_call: tool_call.clone(),
                tier,
            });
        }
    }
    if let Some(request) = approval_request {
        let _ = ctx
            .event_tx
            .send(ExecutionEvent::ToolApprovalRequested { request });
    }
}

fn apply_pending_engine_reverts(
    pending: &Arc<parking_lot::Mutex<Vec<EditBatch>>>,
    engine: &mut InteractiveEngine,
    tool_runner: &ToolRunner,
) {
    let batches = pending.lock().drain(..).collect::<Vec<_>>();
    for batch in batches {
        engine.revert_file_changes_for_batch(&batch.batch_id, &NodeId(batch.node_id.clone()));
        crate::tools::edit::batch::sync_hashline_snapshots_after_revert(
            tool_runner.cwd(),
            tool_runner.snapshot_store().as_ref(),
            &batch,
        );
    }
}

fn abort_run(event_tx: &UnboundedSender<ExecutionEvent>, aborted_emitted: &mut bool) {
    if *aborted_emitted {
        return;
    }
    *aborted_emitted = true;
    let _ = event_tx.send(ExecutionEvent::Aborted);
}

async fn wait_for_input(
    action_rx: &mut UnboundedReceiver<ExecutionAction>,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) -> Option<String> {
    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                abort_run(event_tx, aborted_emitted);
                return None;
            }
            action = action_rx.recv() => match action {
                Some(ExecutionAction::Stop) => {
                    abort_run(event_tx, aborted_emitted);
                    return None;
                }
                Some(ExecutionAction::ProvideInput(text)) => return Some(text),
                Some(ExecutionAction::ResolveApproval { .. }) => continue,
                None => return None,
            }
        }
    }
}

async fn wait_for_approval(
    action_rx: &mut UnboundedReceiver<ExecutionAction>,
    approval_id: &str,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) -> Option<bool> {
    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                abort_run(event_tx, aborted_emitted);
                return None;
            }
            action = action_rx.recv() => match action {
                Some(ExecutionAction::Stop) => {
                    abort_run(event_tx, aborted_emitted);
                    return None;
                }
                Some(ExecutionAction::ResolveApproval {
                    approval_id: received,
                    allow,
                }) if received == approval_id => return Some(allow),
                Some(ExecutionAction::ProvideInput(_)) => continue,
                Some(ExecutionAction::ResolveApproval { .. }) => continue,
                None => return Some(false),
            }
        }
    }
}

struct TelemetryAiPort<A> {
    inner: Arc<A>,
    event_tx: UnboundedSender<ExecutionEvent>,
}

#[async_trait]
impl<A> AiPort for TelemetryAiPort<A>
where
    A: AiPort + Send + Sync,
{
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        send_node_start_events(&self.event_tx, &request);
        let node_id = request.node_id.clone();
        let label = request.node_label.clone();
        let result = self.inner.invoke(request).await;
        if let Ok(outcome) = &result {
            emit_assistant_message(&self.event_tx, &node_id, outcome);
            if let AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. }) = outcome {
                let _ = self.event_tx.send(ExecutionEvent::NodeCompleted {
                    node_id,
                    label,
                    output: output.clone(),
                });
            }
        }
        result
    }
}

fn send_node_start_events(event_tx: &UnboundedSender<ExecutionEvent>, request: &AgentRequest) {
    let _ = event_tx.send(ExecutionEvent::NodeQueued {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::NodeStarted {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
}

fn emit_assistant_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &str,
    outcome: &AgentTurnOutcome,
) {
    let message = match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            assistant_message, ..
        }) => assistant_message.clone(),
        AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            assistant_message, ..
        }) => Some(assistant_message.clone()),
    };
    let message = filter_tool_turn_assistant_message(message);
    if let Some(content) = message.filter(|value| !value.trim().is_empty()) {
        let _ = event_tx.send(ExecutionEvent::ChatMessage {
            node_id: NodeId(node_id.to_string()),
            role: ChatRole::Assistant,
            content,
        });
    }
}
