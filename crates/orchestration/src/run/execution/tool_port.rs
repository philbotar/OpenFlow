use crate::tools::{ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError};
use async_trait::async_trait;
use engine::{
    advance_subagent_invoke, build_predefined_subagent_summaries, handle_declare_subagents,
    is_subagent_runtime_builtin, start_subagent_invoke, subagent_runtime_builtin_denied,
    AgentToolCallBatch, AgentTurnOutcome, AiPort, CallableAgent, InteractiveEngine, NodeId,
    NodeToolConfig, RunTelemetry, SubagentInvokeStep, SubagentStartOutcome, SubagentSummary,
    ToolCall, ToolConcurrency, ToolPort, ToolResult, Workflow, CALL_SUBAGENT_TOOL,
    DECLARE_SUBAGENTS_TOOL,
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc::UnboundedSender, Semaphore};
use tokio_util::sync::CancellationToken;

use super::subagents::{augment_call_subagent_tool_description, merge_subagent_summaries_into_map};
use super::timing::emit_phase_timed;
use super::{send_or_log, ExecutionEvent, NodeInterrupts};

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
    node_interrupts: NodeInterrupts,
    aborted_emitted: parking_lot::Mutex<bool>,
    exclusive_semaphores: parking_lot::Mutex<BTreeMap<String, Arc<Semaphore>>>,
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
        node_interrupts: NodeInterrupts,
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
            node_interrupts,
            aborted_emitted: parking_lot::Mutex::new(false),
            exclusive_semaphores: parking_lot::Mutex::new(BTreeMap::new()),
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
        let mut index = 0usize;
        while index < calls.len() {
            if self.cancel_token.is_cancelled() {
                break;
            }
            if self.node_interrupt_is_cancelled(node_id) {
                engine.mark_node_interrupted(&node_id.0);
                break;
            }
            if self.is_parallel_shared_tool(&calls[index]) {
                let start = index;
                while index < calls.len() && self.is_parallel_shared_tool(&calls[index]) {
                    index += 1;
                }
                match self
                    .run_parallel_regular_tools(engine, node_id, label, &calls[start..index])
                    .await
                {
                    Some(batch_results) => results.extend(batch_results),
                    None => break,
                }
                continue;
            }

            let tool_call = calls[index].clone();
            index += 1;
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
    fn is_parallel_shared_tool(&self, tool_call: &ToolCall) -> bool {
        if tool_call.name == DECLARE_SUBAGENTS_TOOL || tool_call.name == CALL_SUBAGENT_TOOL {
            return false;
        }
        self.tool_runner
            .registry()
            .get(&tool_call.name)
            .map(|registered| registered.definition.concurrency == ToolConcurrency::Shared)
            .unwrap_or(false)
    }

    async fn run_parallel_regular_tools(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        tool_calls: &[ToolCall],
    ) -> Option<Vec<ToolResult>> {
        let mut results: Vec<Option<ToolResult>> = vec![None; tool_calls.len()];
        let mut runnable_indices = Vec::new();

        for (index, tool_call) in tool_calls.iter().enumerate() {
            self.propose_tool_call(node_id, label, tool_call);
            if self.tool_runner.registry().get(&tool_call.name).is_err() {
                let record = self.tool_runner.denied(
                    tool_call.clone(),
                    format!("Tool unavailable: {}", tool_call.name),
                );
                self.emit_tool_completed(node_id, tool_call, &record.result);
                results[index] = Some(record.result);
            } else {
                self.emit_tool_started(node_id, tool_call);
                runnable_indices.push(index);
            }
        }

        let mut join_handles = Vec::with_capacity(runnable_indices.len());
        for &index in &runnable_indices {
            let tool_call = &tool_calls[index];
            let tool_runner = Arc::clone(&self.tool_runner);
            let cancel_token = self.cancel_token.clone();
            let node_id_for_task = node_id.clone();
            let call = tool_call.clone();
            let exclusive_permit = self.exclusive_permit(&call.name).await;
            join_handles.push(tokio::spawn(async move {
                let _permit = exclusive_permit;
                let conversation_id = node_id_for_task.0.clone();
                let ctx = ToolExecutionContext {
                    node_id: node_id_for_task,
                    conversation_id,
                };
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => None,
                    result = tool_runner.execute(call, Some(ctx)) => Some(result),
                }
            }));
        }
        for (index, handle) in runnable_indices.into_iter().zip(join_handles) {
            let tool_call = &tool_calls[index];
            let outcome = match handle.await {
                Ok(value) => value,
                Err(_) => return None,
            };
            match outcome {
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
                    self.emit_tool_completed(node_id, tool_call, &record.result);
                    results[index] = Some(record.result);
                }
                Some(Err(error)) => {
                    let record = self
                        .tool_runner
                        .denied(tool_call.clone(), render_tool_error(error));
                    self.emit_tool_completed(node_id, tool_call, &record.result);
                    results[index] = Some(record.result);
                }
                None => return None,
            }
        }
        Some(
            results
                .into_iter()
                .map(|result| result.expect("every parallel tool call is denied or executed"))
                .collect(),
        )
    }

    async fn exclusive_permit(&self, tool_name: &str) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let concurrency = self
            .tool_runner
            .registry()
            .get(tool_name)
            .ok()?
            .definition
            .concurrency;
        if concurrency != ToolConcurrency::Exclusive {
            return None;
        }
        let semaphore = {
            let mut semaphores = self.exclusive_semaphores.lock();
            semaphores
                .entry(tool_name.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };
        semaphore.acquire_owned().await.ok()
    }

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
            .execute_tool_or_cancel(engine, tool_call.clone(), node_id, &node_id.0)
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

        // Subagents share the parent's node id but have their own transcript,
        // so they get a distinct conversation id for the tool result cache.
        let conversation_id = format!("subagent:{}", uuid::Uuid::new_v4());
        let mut outcome = self.invoke_ai_or_cancel(session.request.clone()).await?;
        loop {
            let tool_results = if let Ok(AgentTurnOutcome::ToolCalls(batch)) = &outcome {
                self.execute_subagent_tool_batch(engine, node_id, &conversation_id, batch)
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
        conversation_id: &str,
        batch: &AgentToolCallBatch,
    ) -> Option<Vec<ToolResult>> {
        let mut results = Vec::new();
        for tool_call in &batch.tool_calls {
            if is_subagent_runtime_builtin(&tool_call.name) {
                results.push(subagent_runtime_builtin_denied(tool_call));
                continue;
            }
            match self
                .execute_tool_or_cancel(engine, tool_call.clone(), node_id, conversation_id)
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
        send_or_log(
            &self.event_tx,
            ExecutionEvent::ToolCompleted {
                node_id: node_id.clone(),
                tool_call_id: result.tool_call_id.clone(),
                tool_name: result.tool_name.clone(),
                content: result.content.clone(),
                is_error: result.is_error,
                output_meta: result.output_meta.clone(),
                artifact_ids: result.artifact_ids.clone(),
            },
        );
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
        let label = format!("subagent · {}", request.node_label);
        let node_id = request.node_id.clone();
        let started = Instant::now();
        let result = tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                abort_run(&self.event_tx, &self.aborted_emitted);
                None
            }
            result = ai.invoke(request) => Some(result),
        };
        if result.is_some() {
            emit_phase_timed(&self.event_tx, "ai_invoke", &label, Some(node_id), started);
        }
        result
    }

    fn node_interrupt_token(&self, node_id: &NodeId) -> Option<CancellationToken> {
        self.node_interrupts
            .lock()
            .get(node_id)
            .map(|(_, token)| token.clone())
    }

    fn node_interrupt_is_cancelled(&self, node_id: &NodeId) -> bool {
        self.node_interrupt_token(node_id)
            .is_some_and(|token| token.is_cancelled())
    }

    async fn execute_tool_or_cancel(
        &self,
        engine: &mut InteractiveEngine,
        tool_call: ToolCall,
        node_id: &NodeId,
        conversation_id: &str,
    ) -> Option<Result<ToolExecutionRecord, ToolRunnerError>> {
        let tool_runner = Arc::clone(&self.tool_runner);
        let tool_name = tool_call.name.clone();
        let ctx = ToolExecutionContext {
            node_id: node_id.clone(),
            conversation_id: conversation_id.to_string(),
        };
        let started = Instant::now();
        let exclusive_permit = self.exclusive_permit(&tool_name).await;
        let node_token = self.node_interrupt_token(node_id);
        let result = match node_token {
            Some(node_token) => {
                tokio::select! {
                    biased;
                    _ = self.cancel_token.cancelled() => {
                        abort_run(&self.event_tx, &self.aborted_emitted);
                        None
                    }
                    _ = node_token.cancelled() => {
                        engine.mark_node_interrupted(&node_id.0);
                        None
                    }
                    result = tool_runner.execute(tool_call, Some(ctx)) => Some(result),
                }
            }
            None => {
                tokio::select! {
                    biased;
                    _ = self.cancel_token.cancelled() => {
                        abort_run(&self.event_tx, &self.aborted_emitted);
                        None
                    }
                    result = tool_runner.execute(tool_call, Some(ctx)) => Some(result),
                }
            }
        };
        drop(exclusive_permit);
        if result.is_some() {
            emit_phase_timed(
                &self.event_tx,
                "tool",
                &tool_name,
                Some(node_id.clone()),
                started,
            );
        }
        result
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
