use crate::tools::{ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError};
use async_trait::async_trait;
use engine::{
    advance_subagent_invoke, build_predefined_subagent_summaries, handle_declare_subagents,
    is_subagent_runtime_builtin, start_subagent_invoke, subagent_runtime_builtin_denied,
    AgentToolCallBatch, AgentTurnOutcome, AiPort, CallableAgent, InteractiveEngine, NodeId,
    NodeToolConfig, RunTelemetry, SubagentInvokeStep, SubagentStartOutcome, SubagentSummary,
    ToolCall, ToolPort, ToolResult, Workflow, CALL_SUBAGENT_TOOL, DECLARE_SUBAGENTS_TOOL,
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use super::subagents::{augment_call_subagent_tool_description, merge_subagent_summaries_into_map};
use super::ExecutionEvent;

pub struct ToolPortImpl<A> {
    tool_runner: Arc<ToolRunner>,
    workflow: Arc<Workflow>,
    agent_snapshots: Arc<BTreeMap<String, CallableAgent>>,
    ai: Arc<A>,
    declared_subagents: parking_lot::Mutex<BTreeMap<String, SubagentSummary>>,
    predefined_registered: parking_lot::Mutex<HashSet<NodeId>>,
    proposed_tool_calls: parking_lot::Mutex<HashSet<String>>,
    cancel_token: CancellationToken,
    event_tx: UnboundedSender<ExecutionEvent>,
    aborted_emitted: parking_lot::Mutex<bool>,
}

impl<A> ToolPortImpl<A>
where
    A: AiPort + Send + Sync + 'static,
{
    pub fn new(
        tool_runner: Arc<ToolRunner>,
        workflow: Arc<Workflow>,
        agent_snapshots: Arc<BTreeMap<String, CallableAgent>>,
        ai: Arc<A>,
        cancel_token: CancellationToken,
        event_tx: UnboundedSender<ExecutionEvent>,
    ) -> Self {
        let mut declared_subagents = BTreeMap::new();
        for node in &workflow.nodes {
            let summaries = build_predefined_subagent_summaries(node, &agent_snapshots);
            if !summaries.is_empty() {
                merge_subagent_summaries_into_map(&mut declared_subagents, &summaries);
            }
        }
        Self {
            tool_runner,
            workflow,
            agent_snapshots,
            ai,
            declared_subagents: parking_lot::Mutex::new(declared_subagents),
            predefined_registered: parking_lot::Mutex::new(HashSet::new()),
            proposed_tool_calls: parking_lot::Mutex::new(HashSet::new()),
            cancel_token,
            event_tx,
            aborted_emitted: parking_lot::Mutex::new(false),
        }
    }

    pub fn tool_runner(&self) -> &Arc<ToolRunner> {
        &self.tool_runner
    }
}

#[async_trait]
impl<A> ToolPort for ToolPortImpl<A>
where
    A: AiPort + Send + Sync + 'static,
{
    fn augment_request(&self, node_id: &NodeId, request: &mut engine::AgentRequest) {
        let mut predefined_registered = self.predefined_registered.lock();
        if !predefined_registered.contains(node_id) {
            if let Some(node) = self.workflow.nodes.iter().find(|node| node.id == *node_id) {
                let summaries = build_predefined_subagent_summaries(node, &self.agent_snapshots);
                if !summaries.is_empty() {
                    let mut declared = self.declared_subagents.lock();
                    merge_subagent_summaries_into_map(&mut declared, &summaries);
                    let _ = self.event_tx.send(ExecutionEvent::SubagentsDeclared {
                        node_id: node_id.clone(),
                        summaries,
                    });
                }
            }
            predefined_registered.insert(node_id.clone());
        }
        request.available_tools = self
            .tool_runner
            .registry()
            .definitions_for(&request.tool_config);
        if let Some(node) = self.workflow.nodes.iter().find(|node| node.id == *node_id) {
            let declared = self.declared_subagents.lock();
            augment_call_subagent_tool_description(
                &mut request.available_tools,
                node,
                &declared,
                &self.agent_snapshots,
            );
        }
    }

    async fn execute_batch(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> Vec<ToolResult> {
        let node_config = self
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == *node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        let mut results = Vec::with_capacity(calls.len());
        for tool_call in calls {
            if self.cancel_token.is_cancelled() {
                break;
            }
            let result = if tool_call.name == DECLARE_SUBAGENTS_TOOL {
                self.run_declare_subagents(node_id, label, &tool_call)
            } else if tool_call.name == CALL_SUBAGENT_TOOL {
                match self
                    .run_call_subagent(engine, node_id, label, &tool_call, &node_config)
                    .await
                {
                    Some(result) => result,
                    None => break,
                }
            } else {
                match self
                    .run_regular_tool(engine, node_id, label, tool_call, &node_config)
                    .await
                {
                    Some(result) => result,
                    None => break,
                }
            };
            results.push(result);
        }
        results
    }
}

impl<A> ToolPortImpl<A>
where
    A: AiPort + Send + Sync + 'static,
{
    fn run_declare_subagents(
        &self,
        node_id: &NodeId,
        label: &str,
        tool_call: &ToolCall,
    ) -> ToolResult {
        self.propose_tool_call(node_id, label, tool_call);
        let mut declared = self.declared_subagents.lock();
        let outcome = handle_declare_subagents(node_id, tool_call, &mut declared);
        let _ = self.event_tx.send(ExecutionEvent::SubagentsDeclared {
            node_id: node_id.clone(),
            summaries: outcome.summaries.clone(),
        });
        self.emit_tool_started(node_id, tool_call);
        self.emit_tool_completed(node_id, tool_call, &outcome.tool_result);
        outcome.tool_result
    }

    async fn run_regular_tool(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        tool_call: ToolCall,
        _node_config: &NodeToolConfig,
    ) -> Option<ToolResult> {
        self.propose_tool_call(node_id, label, &tool_call);
        if let Err(error) = self.tool_runner.registry().get(&tool_call.name) {
            let record = self
                .tool_runner
                .denied(tool_call.clone(), format!("Tool unavailable: {error}"));
            self.emit_tool_completed(node_id, &tool_call, &record.result);
            return Some(record.result);
        }
        self.emit_tool_started(node_id, &tool_call);
        match self
            .execute_tool_or_cancel(tool_call.clone(), node_id)
            .await
        {
            Some(Ok(record)) => {
                if let Some(artifact) = record.artifact.clone() {
                    let _ = self.event_tx.send(ExecutionEvent::ToolArtifactCreated {
                        node_id: node_id.clone(),
                        artifact_id: artifact.artifact_id.clone(),
                        tool_name: artifact.tool_name.clone(),
                        path: artifact.path.clone(),
                        size_bytes: artifact.size_bytes,
                    });
                }
                self.record_tool_file_changes(engine, node_id, &record);
                self.emit_tool_completed(node_id, &tool_call, &record.result);
                Some(record.result)
            }
            Some(Err(error)) => {
                let record = self
                    .tool_runner
                    .denied(tool_call.clone(), render_tool_error(error));
                self.emit_tool_completed(node_id, &tool_call, &record.result);
                Some(record.result)
            }
            None => None,
        }
    }

    async fn run_call_subagent(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        tool_call: &ToolCall,
        node_config: &NodeToolConfig,
    ) -> Option<ToolResult> {
        self.propose_tool_call(node_id, label, tool_call);
        let available_tools = self
            .tool_runner
            .registry()
            .definitions_for_subagent(node_config);
        let (mut session, startup_telemetry) = {
            let mut declared = self.declared_subagents.lock();
            match start_subagent_invoke(
                &self.workflow,
                node_id,
                tool_call,
                &mut declared,
                &self.agent_snapshots,
                available_tools,
            ) {
                SubagentStartOutcome::Started(session, telemetry) => (*session, telemetry),
                SubagentStartOutcome::Failed(tool_result) => {
                    self.emit_tool_started(node_id, tool_call);
                    self.emit_tool_completed(node_id, tool_call, &tool_result);
                    return Some(tool_result);
                }
            }
        };
        send_run_telemetry(&self.event_tx, startup_telemetry);
        self.emit_tool_started(node_id, tool_call);

        let mut outcome = self.invoke_ai_or_cancel(session.request.clone()).await?;
        loop {
            let tool_results = if let Ok(AgentTurnOutcome::ToolCalls(batch)) = &outcome {
                self.execute_subagent_tool_batch(engine, node_id, batch)
                    .await?
            } else {
                Vec::new()
            };
            match advance_subagent_invoke(session, outcome, tool_results) {
                SubagentInvokeStep::NeedAi(next_session) => {
                    session = next_session;
                    outcome = self.invoke_ai_or_cancel(session.request.clone()).await?;
                }
                SubagentInvokeStep::Done {
                    tool_result,
                    subagent,
                    telemetry,
                } => {
                    self.declared_subagents
                        .lock()
                        .insert(subagent.id.clone(), subagent);
                    send_run_telemetry(&self.event_tx, telemetry);
                    self.emit_tool_completed(node_id, tool_call, &tool_result);
                    return Some(tool_result);
                }
            }
        }
    }

    async fn execute_subagent_tool_batch(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        batch: &AgentToolCallBatch,
    ) -> Option<Vec<ToolResult>> {
        let mut results = Vec::new();
        for tool_call in &batch.tool_calls {
            if is_subagent_runtime_builtin(&tool_call.name) {
                results.push(subagent_runtime_builtin_denied(tool_call));
                continue;
            }
            match self
                .execute_tool_or_cancel(tool_call.clone(), node_id)
                .await
            {
                Some(Ok(record)) => {
                    self.record_tool_file_changes(engine, node_id, &record);
                    results.push(record.result);
                }
                Some(Err(err)) => results.push(ToolResult {
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

    fn propose_tool_call(&self, node_id: &NodeId, label: &str, tool_call: &ToolCall) {
        let mut proposed = self.proposed_tool_calls.lock();
        if proposed.insert(tool_call.id.clone()) {
            let _ = self.event_tx.send(ExecutionEvent::ToolCallProposed {
                node_id: node_id.clone(),
                label: label.to_string(),
                tool_call: tool_call.clone(),
            });
        }
    }

    fn emit_tool_started(&self, node_id: &NodeId, tool_call: &ToolCall) {
        let _ = self.event_tx.send(ExecutionEvent::ToolStarted {
            node_id: node_id.clone(),
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
        });
    }

    fn emit_tool_completed(&self, node_id: &NodeId, _tool_call: &ToolCall, result: &ToolResult) {
        let _ = self.event_tx.send(ExecutionEvent::ToolCompleted {
            node_id: node_id.clone(),
            tool_call_id: result.tool_call_id.clone(),
            tool_name: result.tool_name.clone(),
            content: result.content.clone(),
            is_error: result.is_error,
            output_meta: result.output_meta.clone(),
            artifact_ids: result.artifact_ids.clone(),
        });
    }

    fn record_tool_file_changes(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        record: &ToolExecutionRecord,
    ) {
        if let Some(batch) = record.edit_batch.clone() {
            let _ = self.event_tx.send(ExecutionEvent::EditBatchRecorded {
                node_id: node_id.clone(),
                batch,
            });
        }
        if record.file_changes.is_empty() {
            return;
        }
        engine.record_file_changes(node_id, record.file_changes.clone());
        for change in &record.file_changes {
            let _ = self.event_tx.send(ExecutionEvent::FileChanged {
                node_id: node_id.clone(),
                record: change.clone(),
            });
        }
    }

    async fn invoke_ai_or_cancel(
        &self,
        request: engine::AgentRequest,
    ) -> Option<Result<AgentTurnOutcome, engine::AgentError>> {
        let ai = Arc::clone(&self.ai);
        tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                abort_run(&self.event_tx, &self.aborted_emitted);
                None
            }
            result = ai.invoke(request) => Some(result),
        }
    }

    async fn execute_tool_or_cancel(
        &self,
        tool_call: ToolCall,
        node_id: &NodeId,
    ) -> Option<Result<ToolExecutionRecord, ToolRunnerError>> {
        let tool_runner = Arc::clone(&self.tool_runner);
        let ctx = ToolExecutionContext {
            node_id: node_id.clone(),
        };
        tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                abort_run(&self.event_tx, &self.aborted_emitted);
                None
            }
            result = tool_runner.execute(tool_call, Some(ctx)) => Some(result),
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

fn abort_run(
    event_tx: &UnboundedSender<ExecutionEvent>,
    aborted_emitted: &parking_lot::Mutex<bool>,
) {
    let mut emitted = aborted_emitted.lock();
    if *emitted {
        return;
    }
    *emitted = true;
    let _ = event_tx.send(ExecutionEvent::Aborted);
}

fn render_tool_error(error: ToolRunnerError) -> String {
    error.to_string()
}
