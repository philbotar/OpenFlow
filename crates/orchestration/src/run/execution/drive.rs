use crate::run::persistence::{PendingRunCheckpoint, RunCheckpointReason};
use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner};
use engine::{
    AiPort, EditBatch, EngineRunResult, InteractiveEngine, InteractiveEngineCheckpoint, NodeId,
    PendingToolApproval, RunError, ToolCall, ToolTier, Workflow,
};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::ai_adapter::AiInvocationAdapter;
use super::tool_port::ToolPortImpl;
use super::{send_or_log, ExecutionAction, ExecutionEvent, InteractiveWorkflowRunParams};

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
        project_repository_root,
        artifact_root,
        resume_checkpoint,
        checkpoint_sink,
        ai,
        agent_snapshots,
        snapshot_store,
        lsp,
        pending_engine_reverts,
        node_interrupts,
        context_window_sizes,
        mcp,
    } = params;

    let mut engine = match build_engine(
        workflow.clone(),
        entrypoint,
        resume_checkpoint,
        project_repository_root
            .as_ref()
            .map(|path| path.display().to_string()),
    ) {
        Ok(engine) => engine,
        Err(error) => {
            send_or_log(&event_tx, ExecutionEvent::Error(error));
            return;
        }
    };

    let mut tool_registry = ToolRegistry::new();
    let effective_servers = crate::adapters::mcp::effective_mcp_servers(&mcp, &execution_cwd);
    let effective_mcp = crate::settings::model::McpSettings {
        servers: effective_servers,
        discover_external: mcp.discover_external,
        disabled_discovered_ids: mcp.disabled_discovered_ids.clone(),
    };
    let mcp_clients = match crate::adapters::mcp::McpRunClients::connect(&effective_mcp).await {
        Ok(clients) => clients,
        Err(error) => {
            send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
            return;
        }
    };
    match mcp_clients.list_all_tool_definitions().await {
        Ok(definitions) => {
            let mcp_tools = definitions
                .into_iter()
                .map(|definition| crate::tool::registry::RegisteredTool {
                    definition,
                    kind: crate::tool::registry::BuiltinToolKind::Mcp,
                })
                .collect();
            if let Err(error) = tool_registry.extend_mcp(mcp_tools) {
                send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
                return;
            }
        }
        Err(error) => {
            send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
            return;
        }
    }
    let artifacts = match ArtifactStore::new(artifact_root) {
        Ok(store) => store,
        Err(error) => {
            send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
            return;
        }
    };
    let tool_runner = Arc::new(
        ToolRunner::new(
            tool_registry,
            execution_cwd,
            artifacts,
            cancel_token.clone(),
            snapshot_store,
        )
        .with_mcp_clients(mcp_clients),
    );
    let workflow = Arc::new(workflow);
    let ai = Arc::new(ai);
    let node_interrupts_for_tools = node_interrupts.clone();
    let ai_adapter = Arc::new(AiInvocationAdapter::new(
        Arc::clone(&ai),
        event_tx.clone(),
        node_interrupts,
        cancel_token.clone(),
        context_window_sizes,
    ));
    let tool_port = ToolPortImpl::new(
        Arc::clone(&tool_runner),
        lsp,
        Arc::clone(&workflow),
        Arc::new(agent_snapshots),
        Arc::clone(&ai_adapter),
        cancel_token.clone(),
        event_tx.clone(),
        node_interrupts_for_tools,
    );
    let mut proposed_tool_calls: HashSet<String> = HashSet::new();
    let mut aborted_emitted = false;
    let mut emitted_retryables: HashSet<(NodeId, u8)> = HashSet::new();

    loop {
        if cancel_token.is_cancelled() {
            snapshot_and_abort(
                &mut engine,
                &checkpoint_sink,
                &event_tx,
                &mut aborted_emitted,
            );
            return;
        }
        apply_pending_engine_reverts(
            &pending_engine_reverts,
            &mut engine,
            tool_port.tool_runner(),
        );
        let run_result = engine.run(&*ai_adapter, &tool_port, &cancel_token).await;
        match run_result {
            EngineRunResult::NeedsInteraction {
                inputs,
                approvals,
                retryables,
            } => {
                for input in &inputs {
                    if input.is_initial {
                        send_or_log(
                            &event_tx,
                            ExecutionEvent::NodeQueued {
                                node_id: input.node_id.clone(),
                                label: input.label.clone(),
                            },
                        );
                    }
                    send_or_log(
                        &event_tx,
                        ExecutionEvent::NodeAwaitingInput {
                            node_id: input.node_id.clone(),
                            label: input.label.clone(),
                            context: input.context.clone(),
                            is_initial: input.is_initial,
                        },
                    );
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

                for retryable in &retryables {
                    let attempt = engine.model_attempt_for_node(&retryable.node_id);
                    if !emitted_retryables.insert((retryable.node_id.clone(), attempt)) {
                        continue;
                    }
                    if retryable.interrupted {
                        send_or_log(
                            &event_tx,
                            ExecutionEvent::NodeInterrupted {
                                node_id: retryable.node_id.clone(),
                                label: retryable.label.clone(),
                            },
                        );
                    } else {
                        send_or_log(
                            &event_tx,
                            ExecutionEvent::NodeErrored {
                                node_id: retryable.node_id.clone(),
                                label: retryable.label.clone(),
                                error: retryable.error.clone(),
                            },
                        );
                    }
                }

                let mut pending_inputs: HashSet<NodeId> =
                    inputs.iter().map(|input| input.node_id.clone()).collect();
                let mut pending_approvals: HashSet<String> = approvals
                    .iter()
                    .map(|approval| approval.approval_id.clone())
                    .collect();
                let mut pending_retryables: HashSet<NodeId> = retryables
                    .iter()
                    .map(|retryable| retryable.node_id.clone())
                    .collect();

                let reason = if !approvals.is_empty() {
                    RunCheckpointReason::AwaitingToolApproval
                } else if !retryables.is_empty() {
                    RunCheckpointReason::AwaitingRetry
                } else {
                    RunCheckpointReason::AwaitingInput
                };
                publish_checkpoint(&mut engine, &checkpoint_sink, reason);

                while !pending_inputs.is_empty()
                    || !pending_approvals.is_empty()
                    || !pending_retryables.is_empty()
                {
                    if cancel_token.is_cancelled() {
                        snapshot_and_abort(
                            &mut engine,
                            &checkpoint_sink,
                            &event_tx,
                            &mut aborted_emitted,
                        );
                        return;
                    }
                    let Some(action) = action_rx.recv().await else {
                        log::warn!("execution action channel closed; aborting run");
                        snapshot_and_abort(
                            &mut engine,
                            &checkpoint_sink,
                            &event_tx,
                            &mut aborted_emitted,
                        );
                        return;
                    };
                    match action {
                        ExecutionAction::Stop => {
                            snapshot_and_abort(
                                &mut engine,
                                &checkpoint_sink,
                                &event_tx,
                                &mut aborted_emitted,
                            );
                            return;
                        }
                        ExecutionAction::ProvideInput { node_id, text } => {
                            if !pending_inputs.contains(&node_id) {
                                send_or_log(
                                    &event_tx,
                                    ExecutionEvent::Error(format!(
                                        "ignored input for node {node_id}: not in current interaction pause"
                                    )),
                                );
                                return;
                            }
                            if let Err(error) = engine.on_human_input(&node_id, &text) {
                                send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
                                return;
                            }
                            pending_inputs.remove(&node_id);
                        }
                        ExecutionAction::ResolveApproval {
                            approval_id,
                            allow,
                            reason,
                        } => {
                            if !pending_approvals.contains(&approval_id) {
                                send_or_log(
                                    &event_tx,
                                    ExecutionEvent::Error(format!(
                                        "ignored approval {approval_id}: not in current interaction pause"
                                    )),
                                );
                                return;
                            }
                            let node_id = engine
                                .pending_tool_batch_node(&approval_id)
                                .or_else(|| approval_nodes.get(&approval_id).cloned())
                                .unwrap_or_else(|| NodeId("unknown".to_string()));
                            if let Err(error) =
                                engine.on_tool_decision(&approval_id, allow, reason.as_deref())
                            {
                                send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
                                return;
                            }
                            if let Some(tool_calls) = approval_tool_calls.get(&approval_id) {
                                for tool_call in tool_calls {
                                    if allow {
                                        send_or_log(
                                            &event_tx,
                                            ExecutionEvent::ToolApproved {
                                                approval_id: approval_id.clone(),
                                                node_id: node_id.clone(),
                                                tool_call_id: tool_call.id.clone(),
                                                tool_name: tool_call.name.clone(),
                                            },
                                        );
                                    } else {
                                        send_or_log(
                                            &event_tx,
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
                            pending_approvals.remove(&approval_id);
                        }
                        ExecutionAction::RetryNode { node_id } => {
                            if !pending_retryables.contains(&node_id) {
                                send_or_log(
                                    &event_tx,
                                    ExecutionEvent::Error(format!(
                                        "ignored retry for node {node_id}: not in current interaction pause"
                                    )),
                                );
                                return;
                            }
                            if let Err(error) = engine.retry_node(&node_id) {
                                send_or_log(&event_tx, ExecutionEvent::Error(error.to_string()));
                                return;
                            }
                            pending_retryables.remove(&node_id);
                        }
                    }
                }
            }
            EngineRunResult::Completed(report) => {
                publish_checkpoint(
                    &mut engine,
                    &checkpoint_sink,
                    RunCheckpointReason::Completed,
                );
                send_or_log(&event_tx, ExecutionEvent::Finished(report));
                return;
            }
            EngineRunResult::Failed(error) => {
                publish_checkpoint(&mut engine, &checkpoint_sink, RunCheckpointReason::Failed);
                match error {
                    RunError::NodeFailed { node_id, kind } => {
                        let label = workflow
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
                    &mut engine,
                    &checkpoint_sink,
                    &event_tx,
                    &mut aborted_emitted,
                );
                return;
            }
        }
    }
}

fn build_engine(
    workflow: Workflow,
    entrypoint: Option<String>,
    resume_checkpoint: Option<InteractiveEngineCheckpoint>,
    project_repository_root: Option<String>,
) -> Result<InteractiveEngine, String> {
    match resume_checkpoint {
        Some(checkpoint) => InteractiveEngine::from_checkpoint_with_run_context(
            workflow,
            checkpoint,
            project_repository_root,
        )
        .map(|mut engine| {
            let failures = engine.prepare_resume();
            if !failures.is_empty() {
                log::warn!("prepare_resume could not retry nodes: {failures:?}");
            }
            engine
        })
        .map_err(|error| error.to_string()),
        None => {
            InteractiveEngine::new_with_run_context(workflow, entrypoint, project_repository_root)
                .map_err(|error| error.to_string())
        }
    }
}

pub fn new_artifact_root() -> PathBuf {
    std::env::temp_dir().join(format!("openflow-run-{}", Uuid::new_v4()))
}

#[must_use]
pub fn new_in_memory_snapshot_store(
) -> Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore> {
    Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new())
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

fn apply_pending_engine_reverts(
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

fn publish_checkpoint(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    reason: RunCheckpointReason,
) {
    *checkpoint_sink.lock() = Some(PendingRunCheckpoint {
        reason,
        engine: engine.prepare_stop_checkpoint(),
    });
}

fn snapshot_and_abort(
    engine: &mut InteractiveEngine,
    checkpoint_sink: &Arc<Mutex<Option<PendingRunCheckpoint>>>,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) {
    publish_checkpoint(engine, checkpoint_sink, RunCheckpointReason::UserStopped);
    abort_run(event_tx, aborted_emitted);
}

fn abort_run(event_tx: &UnboundedSender<ExecutionEvent>, aborted_emitted: &mut bool) {
    if *aborted_emitted {
        return;
    }
    *aborted_emitted = true;
    send_or_log(event_tx, ExecutionEvent::Aborted);
}
