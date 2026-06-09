use crate::tools::{
    ArtifactStore, ToolExecutionContext, ToolExecutionRecord, ToolRegistry, ToolRunner,
    ToolRunnerError,
};
use crate::lsp::LspSettings;
use domain::{
    advance_subagent_invoke, build_predefined_subagent_summaries,
    filter_tool_turn_assistant_message, handle_declare_subagents, is_subagent_runtime_builtin,
    start_subagent_invoke, subagent_runtime_builtin_denied, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AiPort, CallableAgent, ChatRole, EditBatch,
    EnginePollResult, FileChangeRecord, InteractiveEngine, NodeId, RunTelemetry,
    SubagentInvokeStep, SubagentStartOutcome, SubagentSummary, ToolCall, Workflow,
    CALL_SUBAGENT_TOOL, DECLARE_SUBAGENTS_TOOL,
};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::subagents::{augment_call_subagent_tool_description, merge_subagent_summaries_into_map};
use super::{ExecutionAction, ExecutionEvent};

#[allow(clippy::too_many_arguments)]
pub(super) async fn drive_interactive_workflow<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    execution_cwd: PathBuf,
    ai: A,
    event_tx: UnboundedSender<ExecutionEvent>,
    mut action_rx: UnboundedReceiver<ExecutionAction>,
    agent_snapshots: BTreeMap<String, CallableAgent>,
    snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    cancel_token: CancellationToken,
    lsp: LspSettings,
    pending_engine_reverts: Arc<parking_lot::Mutex<Vec<EditBatch>>>,
) where
    A: AiPort,
{
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
    let tool_runner = ToolRunner::new(
        tool_registry,
        execution_cwd,
        artifacts,
        cancel_token.clone(),
        snapshot_store,
        lsp,
    );
    let mut declared_subagents: BTreeMap<String, SubagentSummary> = BTreeMap::new();
    let mut predefined_registered: HashSet<NodeId> = HashSet::new();
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
            &tool_runner,
        );
        match engine.poll() {
            EnginePollResult::CallAi {
                node_id,
                mut request,
            } => {
                if !predefined_registered.contains(&node_id) {
                    if let Some(node) = workflow.nodes.iter().find(|node| node.id == node_id) {
                        let summaries = build_predefined_subagent_summaries(node, &agent_snapshots);
                        if !summaries.is_empty() {
                            merge_subagent_summaries_into_map(&mut declared_subagents, &summaries);
                            let _ = event_tx.send(ExecutionEvent::SubagentsDeclared {
                                node_id: node_id.clone(),
                                summaries,
                            });
                        }
                    }
                    predefined_registered.insert(node_id.clone());
                }
                request.available_tools =
                    tool_runner.registry().definitions_for(&request.tool_config);
                if let Some(node) = workflow.nodes.iter().find(|node| node.id == node_id) {
                    augment_call_subagent_tool_description(
                        &mut request.available_tools,
                        node,
                        &declared_subagents,
                        &agent_snapshots,
                    );
                }
                send_node_start_events(&event_tx, &request);
                let Some(result) = invoke_ai_or_cancel(
                    &ai,
                    (*request).clone(),
                    &cancel_token,
                    &event_tx,
                    &mut aborted_emitted,
                )
                .await
                else {
                    return;
                };
                if let Ok(outcome) = &result {
                    emit_assistant_message(&event_tx, &node_id, outcome);
                }
                let invoke_error = result.as_ref().err().map(ToString::to_string);
                let label = request.node_label.clone();
                engine.on_ai_complete(&node_id, result);
                if let Some(output) = engine.node_output(&node_id) {
                    let _ = event_tx.send(ExecutionEvent::NodeCompleted {
                        node_id: NodeId(node_id.to_string()),
                        label,
                        output,
                    });
                } else if let Some(error) = invoke_error {
                    let _ = event_tx.send(ExecutionEvent::NodeFailed {
                        node_id: NodeId(node_id.to_string()),
                        label,
                        error,
                    });
                    return;
                }
            }
            EnginePollResult::AwaitInput {
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
            EnginePollResult::AwaitToolApproval {
                approval_id,
                node_id,
                label,
                tool_calls,
            } => {
                let mut approval_request = None;
                for tool_call in &tool_calls {
                    if proposed_tool_calls.insert(tool_call.id.clone()) {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                    }
                    let tier = tool_runner
                        .registry()
                        .get(&tool_call.name)
                        .map(|registered| registered.definition.tier)
                        .unwrap_or_else(|_| {
                            workflow
                                .nodes
                                .iter()
                                .find(|node| node.id == node_id)
                                .map(|node| {
                                    domain::tool_tier_for_call(&node.agent.tools, &tool_call.name)
                                })
                                .unwrap_or(domain::ToolTier::Write)
                        });
                    if approval_request.is_none() {
                        approval_request = Some(domain::PendingToolApproval {
                            approval_id: approval_id.clone(),
                            node_id: node_id.to_string(),
                            node_label: label.clone(),
                            tool_call: tool_call.clone(),
                            tier,
                        });
                    }
                }
                if let Some(request) = approval_request {
                    let _ = event_tx.send(ExecutionEvent::ToolApprovalRequested { request });
                }
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
            EnginePollResult::RunTools {
                node_id,
                label,
                tool_calls,
            } => {
                let node_config = workflow
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .map(|node| node.agent.tools.clone())
                    .unwrap_or_default();
                let mut results = Vec::new();
                for tool_call in tool_calls {
                    if tool_call.name == DECLARE_SUBAGENTS_TOOL {
                        results.push(run_declare_subagents_tool(
                            &event_tx,
                            &node_id,
                            &label,
                            &tool_call,
                            &mut declared_subagents,
                        ));
                        continue;
                    }
                    if tool_call.name == CALL_SUBAGENT_TOOL {
                        let Some(subagent_result) = run_call_subagent_tool(
                            &ai,
                            &tool_runner,
                            &mut engine,
                            &event_tx,
                            SubagentCallParams {
                                workflow: &workflow,
                                node_id: &node_id,
                                label: &label,
                                tool_call: &tool_call,
                                node_config: &node_config,
                                declared_subagents: &mut declared_subagents,
                                agent_snapshots: &agent_snapshots,
                            },
                            &cancel_token,
                            &mut aborted_emitted,
                        )
                        .await
                        else {
                            return;
                        };
                        results.push(subagent_result);
                        continue;
                    }
                    if proposed_tool_calls.insert(tool_call.id.clone()) {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                    }
                    if let Err(error) = tool_runner.registry().get(&tool_call.name) {
                        let record = tool_runner
                            .denied(tool_call.clone(), format!("Tool unavailable: {error}"));
                        let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                            node_id: node_id.clone(),
                            tool_call_id: record.result.tool_call_id.clone(),
                            tool_name: record.result.tool_name.clone(),
                            content: record.result.content.clone(),
                            is_error: true,
                            output_meta: None,
                            artifact_ids: Vec::new(),
                        });
                        results.push(record.result);
                        continue;
                    }
                    let _ = event_tx.send(ExecutionEvent::ToolStarted {
                        node_id: node_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    });
                    match execute_tool_or_cancel(
                        &tool_runner,
                        tool_call.clone(),
                        &node_id,
                        &cancel_token,
                        &event_tx,
                        &mut aborted_emitted,
                    )
                    .await
                    {
                        Some(Ok(record)) => {
                            if let Some(artifact) = record.artifact.clone() {
                                let _ = event_tx.send(ExecutionEvent::ToolArtifactCreated {
                                    node_id: node_id.clone(),
                                    artifact_id: artifact.artifact_id.clone(),
                                    tool_name: artifact.tool_name.clone(),
                                    path: artifact.path.clone(),
                                    size_bytes: artifact.size_bytes,
                                });
                            }
                            record_tool_file_changes(
                                &mut engine,
                                &event_tx,
                                &node_id,
                                &record.file_changes,
                                record.edit_batch,
                            );
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: record.result.tool_call_id.clone(),
                                tool_name: record.result.tool_name.clone(),
                                content: record.result.content.clone(),
                                is_error: record.result.is_error,
                                output_meta: record.result.output_meta.clone(),
                                artifact_ids: record.result.artifact_ids.clone(),
                            });
                            results.push(record.result);
                        }
                        Some(Err(error)) => {
                            let record =
                                tool_runner.denied(tool_call.clone(), render_tool_error(error));
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: record.result.tool_call_id.clone(),
                                tool_name: record.result.tool_name.clone(),
                                content: record.result.content.clone(),
                                is_error: true,
                                output_meta: None,
                                artifact_ids: Vec::new(),
                            });
                            results.push(record.result);
                        }
                        None => return,
                    }
                }
                if let Err(error) = engine.on_tool_results(&node_id, results) {
                    let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                    return;
                }
            }
            EnginePollResult::Completed(report) => {
                let _ = event_tx.send(ExecutionEvent::Finished(report));
                return;
            }
            EnginePollResult::Failed(error) => {
                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                return;
            }
        }
    }
}

fn send_run_telemetry(
    event_tx: &UnboundedSender<ExecutionEvent>,
    events: impl IntoIterator<Item = RunTelemetry>,
) {
    for event in events {
        let _ = event_tx.send(event);
    }
}

fn run_declare_subagents_tool(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    label: &str,
    tool_call: &ToolCall,
    declared_subagents: &mut BTreeMap<String, SubagentSummary>,
) -> domain::ToolResult {
    let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
        node_id: node_id.clone(),
        label: label.to_string(),
        tool_call: tool_call.clone(),
    });
    let outcome = handle_declare_subagents(node_id, tool_call, declared_subagents);
    let _ = event_tx.send(ExecutionEvent::SubagentsDeclared {
        node_id: node_id.clone(),
        summaries: outcome.summaries.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::ToolStarted {
        node_id: node_id.clone(),
        tool_call_id: tool_call.id.clone(),
        tool_name: tool_call.name.clone(),
        arguments: tool_call.arguments.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::ToolCompleted {
        node_id: node_id.clone(),
        tool_call_id: tool_call.id.clone(),
        tool_name: tool_call.name.clone(),
        content: outcome.tool_result.content.clone(),
        is_error: false,
        output_meta: None,
        artifact_ids: Vec::new(),
    });
    outcome.tool_result
}

struct SubagentCallParams<'a> {
    workflow: &'a Workflow,
    node_id: &'a NodeId,
    label: &'a str,
    tool_call: &'a ToolCall,
    node_config: &'a domain::NodeToolConfig,
    declared_subagents: &'a mut BTreeMap<String, SubagentSummary>,
    agent_snapshots: &'a BTreeMap<String, CallableAgent>,
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

fn record_tool_file_changes(
    engine: &mut InteractiveEngine,
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    file_changes: &[FileChangeRecord],
    edit_batch: Option<EditBatch>,
) {
    if let Some(batch) = edit_batch {
        let _ = event_tx.send(ExecutionEvent::EditBatchRecorded {
            node_id: node_id.clone(),
            batch,
        });
    }
    if file_changes.is_empty() {
        return;
    }
    engine.record_file_changes(node_id, file_changes.to_vec());
    for change in file_changes {
        let _ = event_tx.send(ExecutionEvent::FileChanged {
            node_id: node_id.clone(),
            record: change.clone(),
        });
    }
}

async fn run_call_subagent_tool<A: AiPort>(
    ai: &A,
    tool_runner: &ToolRunner,
    engine: &mut InteractiveEngine,
    event_tx: &UnboundedSender<ExecutionEvent>,
    params: SubagentCallParams<'_>,
    cancel_token: &CancellationToken,
    aborted_emitted: &mut bool,
) -> Option<domain::ToolResult> {
    let SubagentCallParams {
        workflow,
        node_id,
        label,
        tool_call,
        node_config,
        declared_subagents,
        agent_snapshots,
    } = params;
    let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
        node_id: node_id.clone(),
        label: label.to_string(),
        tool_call: tool_call.clone(),
    });
    let available_tools = tool_runner.registry().definitions_for_subagent(node_config);
    let (mut session, startup_telemetry) = match start_subagent_invoke(
        workflow,
        node_id,
        tool_call,
        declared_subagents,
        agent_snapshots,
        available_tools,
    ) {
        SubagentStartOutcome::Started(session, telemetry) => (*session, telemetry),
        SubagentStartOutcome::Failed(tool_result) => {
            let _ = event_tx.send(ExecutionEvent::ToolStarted {
                node_id: node_id.clone(),
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            });
            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                node_id: node_id.clone(),
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                content: tool_result.content.clone(),
                is_error: true,
                output_meta: None,
                artifact_ids: Vec::new(),
            });
            return Some(tool_result);
        }
    };
    send_run_telemetry(event_tx, startup_telemetry);
    let _ = event_tx.send(ExecutionEvent::ToolStarted {
        node_id: node_id.clone(),
        tool_call_id: tool_call.id.clone(),
        tool_name: tool_call.name.clone(),
        arguments: tool_call.arguments.clone(),
    });

    let mut outcome = invoke_ai_or_cancel(
        ai,
        session.request.clone(),
        cancel_token,
        event_tx,
        aborted_emitted,
    )
    .await?;
    loop {
        let tool_results = if let Ok(AgentTurnOutcome::ToolCalls(batch)) = &outcome {
            execute_subagent_tool_batch(
                tool_runner,
                engine,
                node_id,
                batch,
                cancel_token,
                event_tx,
                aborted_emitted,
            )
            .await?
        } else {
            Vec::new()
        };
        match advance_subagent_invoke(session, outcome, tool_results) {
            SubagentInvokeStep::NeedAi(next_session) => {
                session = next_session;
                outcome = invoke_ai_or_cancel(
                    ai,
                    session.request.clone(),
                    cancel_token,
                    event_tx,
                    aborted_emitted,
                )
                .await?;
            }
            SubagentInvokeStep::Done {
                tool_result,
                subagent,
                telemetry,
            } => {
                declared_subagents.insert(subagent.id.clone(), subagent);
                send_run_telemetry(event_tx, telemetry);
                let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                    node_id: node_id.clone(),
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    content: tool_result.content.clone(),
                    is_error: tool_result.is_error,
                    output_meta: None,
                    artifact_ids: Vec::new(),
                });
                return Some(tool_result);
            }
        }
    }
}

async fn execute_subagent_tool_batch(
    tool_runner: &ToolRunner,
    engine: &mut InteractiveEngine,
    node_id: &NodeId,
    batch: &AgentToolCallBatch,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) -> Option<Vec<domain::ToolResult>> {
    let mut results = Vec::new();
    for tool_call in &batch.tool_calls {
        if is_subagent_runtime_builtin(&tool_call.name) {
            results.push(subagent_runtime_builtin_denied(tool_call));
            continue;
        }
        match execute_tool_or_cancel(
            tool_runner,
            tool_call.clone(),
            node_id,
            cancel_token,
            event_tx,
            aborted_emitted,
        )
        .await
        {
            Some(Ok(record)) => {
                record_tool_file_changes(
                    engine,
                    event_tx,
                    node_id,
                    &record.file_changes,
                    record.edit_batch,
                );
                results.push(record.result);
            }
            Some(Err(err)) => results.push(domain::ToolResult {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                content: err.to_string(),
                is_error: true,
                artifact_ids: Vec::new(),
                output_meta: None,
            }),
            None => return None,
        }
    }
    Some(results)
}

fn abort_run(event_tx: &UnboundedSender<ExecutionEvent>, aborted_emitted: &mut bool) {
    if *aborted_emitted {
        return;
    }
    *aborted_emitted = true;
    let _ = event_tx.send(ExecutionEvent::Aborted);
}

async fn invoke_ai_or_cancel<A: AiPort>(
    ai: &A,
    request: AgentRequest,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) -> Option<Result<AgentTurnOutcome, domain::AgentError>> {
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            abort_run(event_tx, aborted_emitted);
            None
        }
        result = ai.invoke(request) => Some(result),
    }
}

async fn execute_tool_or_cancel(
    tool_runner: &ToolRunner,
    tool_call: ToolCall,
    node_id: &NodeId,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &mut bool,
) -> Option<Result<ToolExecutionRecord, ToolRunnerError>> {
    let ctx = ToolExecutionContext {
        node_id: node_id.clone(),
    };
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            abort_run(event_tx, aborted_emitted);
            None
        }
        result = tool_runner.execute(tool_call, Some(ctx)) => Some(result),
    }
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

fn render_tool_error(error: ToolRunnerError) -> String {
    error.to_string()
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
