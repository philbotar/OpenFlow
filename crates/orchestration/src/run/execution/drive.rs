use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner};
use engine::{
    AiPort, EditBatch, EngineRunResult, InteractiveEngine, NodeId, PendingToolApproval, ToolCall,
    ToolTier, Workflow,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::ai_adapter::AiInvocationAdapter;
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
    let ai_adapter = Arc::new(AiInvocationAdapter::new(Arc::clone(&ai), event_tx.clone()));
    let tool_port = ToolPortImpl::new(
        Arc::clone(&tool_runner),
        Arc::clone(&workflow),
        Arc::new(agent_snapshots),
        Arc::clone(&ai_adapter),
        cancel_token.clone(),
        event_tx.clone(),
    );
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
        let run_result = engine.run(&*ai_adapter, &tool_port, &cancel_token).await;
        match run_result {
            EngineRunResult::NeedsInteraction { inputs, approvals } => {
                for input in &inputs {
                    if input.is_initial {
                        let _ = event_tx.send(ExecutionEvent::NodeQueued {
                            node_id: input.node_id.clone(),
                            label: input.label.clone(),
                        });
                    }
                    let _ = event_tx.send(ExecutionEvent::NodeAwaitingInput {
                        node_id: input.node_id.clone(),
                        label: input.label.clone(),
                        context: input.context.clone(),
                        is_initial: input.is_initial,
                    });
                }
                let mut approval_tool_calls: HashMap<String, Vec<ToolCall>> = HashMap::new();
                let mut approval_nodes: HashMap<String, NodeId> = HashMap::new();
                for approval in &approvals {
                    approval_tool_calls
                        .insert(approval.approval_id.clone(), approval.tool_calls.clone());
                    approval_nodes.insert(approval.approval_id.clone(), approval.node_id.clone());
                    emit_approval_request(ApprovalRequestEmit {
                        event_tx: &event_tx,
                        workflow: &workflow,
                        tool_runner: tool_port.tool_runner(),
                        approval_id: &approval.approval_id,
                        node_id: &approval.node_id,
                        label: &approval.label,
                        tool_calls: &approval.tool_calls,
                        proposed_tool_calls: &mut proposed_tool_calls,
                    });
                }

                let mut pending_inputs: HashSet<NodeId> =
                    inputs.iter().map(|input| input.node_id.clone()).collect();
                let mut pending_approvals: HashSet<String> = approvals
                    .iter()
                    .map(|approval| approval.approval_id.clone())
                    .collect();

                while !pending_inputs.is_empty() || !pending_approvals.is_empty() {
                    if cancel_token.is_cancelled() {
                        abort_run(&event_tx, &mut aborted_emitted);
                        return;
                    }
                    let Some(action) = action_rx.recv().await else {
                        return;
                    };
                    match action {
                        ExecutionAction::Stop => {
                            abort_run(&event_tx, &mut aborted_emitted);
                            return;
                        }
                        ExecutionAction::ProvideInput { node_id, text } => {
                            if !pending_inputs.contains(&node_id) {
                                let _ = event_tx.send(ExecutionEvent::Error(format!(
                                    "ignored input for node {node_id}: not in current interaction pause"
                                )));
                                return;
                            }
                            if let Err(error) = engine.on_human_input(&node_id, &text) {
                                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                                return;
                            }
                            pending_inputs.remove(&node_id);
                        }
                        ExecutionAction::ResolveApproval { approval_id, allow } => {
                            if !pending_approvals.contains(&approval_id) {
                                let _ = event_tx.send(ExecutionEvent::Error(format!(
                                    "ignored approval {approval_id}: not in current interaction pause"
                                )));
                                return;
                            }
                            if let Err(error) = engine.on_tool_decision(&approval_id, allow) {
                                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                                return;
                            }
                            let node_id = approval_nodes
                                .get(&approval_id)
                                .cloned()
                                .unwrap_or_else(|| NodeId("unknown".to_string()));
                            if let Some(tool_calls) = approval_tool_calls.get(&approval_id) {
                                for tool_call in tool_calls {
                                    if allow {
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
                            pending_approvals.remove(&approval_id);
                        }
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
    if !batches.is_empty() {
        // Reverts change files without passing through write tools.
        tool_runner.bump_cache_epoch();
    }
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
