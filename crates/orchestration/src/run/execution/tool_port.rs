use crate::tool::blocking_ops::split_selector;
use crate::tool::retry::execute_with_retry;
use crate::tools::{ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError};
use async_trait::async_trait;
use engine::{
    augment_call_subagent_tool_description, build_predefined_subagent_summaries,
    handle_declare_subagents, merge_subagent_summaries as merge_subagent_summaries_into_map,
    AiPort, CallableAgent, NodeId, NodeToolConfig, RunTelemetry, SubagentSummary, ToolBatchEffects,
    ToolBatchOutput, ToolCall, ToolConcurrency, ToolPort, ToolResult, Workflow, CALL_SUBAGENT_TOOL,
    DECLARE_SUBAGENTS_TOOL,
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc::UnboundedSender, Semaphore};
use tokio_util::sync::CancellationToken;

use super::{abort_run, emit_phase_timed, send_or_log, ExecutionEvent, NodeInterrupts};

pub struct ToolPortImpl<A> {
    tool_runner: Arc<ToolRunner>,
    lsp: crate::settings::model::LspSettings,
    workflow: Arc<Workflow>,
    agent_snapshots: Arc<BTreeMap<String, CallableAgent>>,
    ai: Arc<A>,
    declared_subagents: parking_lot::Mutex<BTreeMap<String, SubagentSummary>>,
    predefined_registered: parking_lot::Mutex<HashSet<NodeId>>,
    proposed_tool_calls: parking_lot::Mutex<HashSet<String>>,
    cancel_token: CancellationToken,
    event_tx: UnboundedSender<ExecutionEvent>,
    node_interrupts: NodeInterrupts,
    aborted_emitted: Arc<parking_lot::Mutex<bool>>,
    exclusive_locks: Arc<ExclusiveLocks>,
}

impl<A> ToolPortImpl<A>
where
    A: AiPort + Send + Sync + 'static,
{
    #[allow(
        clippy::too_many_arguments,
        reason = "ToolPortImpl wires nine run-scoped dependencies at the execution seam"
    )]
    pub fn new(
        tool_runner: Arc<ToolRunner>,
        lsp: crate::settings::model::LspSettings,
        workflow: Arc<Workflow>,
        agent_snapshots: Arc<BTreeMap<String, CallableAgent>>,
        ai: Arc<A>,
        cancel_token: CancellationToken,
        event_tx: UnboundedSender<ExecutionEvent>,
        node_interrupts: NodeInterrupts,
        aborted_emitted: Arc<parking_lot::Mutex<bool>>,
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
            lsp,
            workflow,
            agent_snapshots,
            ai,
            declared_subagents: parking_lot::Mutex::new(declared_subagents),
            predefined_registered: parking_lot::Mutex::new(HashSet::new()),
            proposed_tool_calls: parking_lot::Mutex::new(HashSet::new()),
            cancel_token,
            event_tx,
            node_interrupts,
            aborted_emitted,
            exclusive_locks: Arc::new(ExclusiveLocks::default()),
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
        node_id: &NodeId,
        label: &str,
        calls: Vec<ToolCall>,
    ) -> ToolBatchOutput {
        let node_config = self
            .workflow
            .nodes
            .iter()
            .find(|node| node.id == *node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        let mut effects = ToolBatchEffects::default();
        let mut results = Vec::with_capacity(calls.len());
        let mut index = 0usize;
        while index < calls.len() {
            if self.cancel_token.is_cancelled() {
                break;
            }
            if self.node_interrupt_is_cancelled(node_id) {
                effects.interrupted = true;
                break;
            }
            if self.is_parallel_shared_tool(&calls[index]) {
                let start = index;
                while index < calls.len() && self.is_parallel_shared_tool(&calls[index]) {
                    index += 1;
                }
                match self
                    .run_parallel_regular_tools(&mut effects, node_id, label, &calls[start..index])
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
                    .run_call_subagent(&mut effects, node_id, label, &tool_call, &node_config)
                    .await
                {
                    Some(result) => result,
                    None => break,
                }
            } else {
                match self
                    .run_regular_tool(&mut effects, node_id, label, tool_call, &node_config)
                    .await
                {
                    Some(result) => result,
                    None => break,
                }
            };
            results.push(result);
        }
        ToolBatchOutput { results, effects }
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
        effects: &mut ToolBatchEffects,
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
                self.note_read_call(effects, tool_call);
                self.emit_tool_started(node_id, tool_call);
                runnable_indices.push(index);
            }
        }

        let mut join_handles = Vec::with_capacity(runnable_indices.len());
        let lsp_settings = self.lsp.clone();
        let retry_policy = self.workflow.settings.retry_policy.clone();
        for &index in &runnable_indices {
            let tool_call = &tool_calls[index];
            let tool_runner = Arc::clone(&self.tool_runner);
            let cancel_token = self.cancel_token.clone();
            let node_id_for_task = node_id.clone();
            let call = tool_call.clone();
            let lsp = lsp_settings.clone();
            let event_tx = self.event_tx.clone();
            let retry_policy = retry_policy.clone();
            let exclusive_locks = Arc::clone(&self.exclusive_locks);
            let exclusive_keys = self.exclusive_lock_keys_for(node_id, &call);
            join_handles.push(tokio::spawn(async move {
                let _permits = exclusive_locks.acquire(exclusive_keys).await;
                let conversation_id = node_id_for_task.0.clone();
                let ctx = ToolExecutionContext {
                    node_id: node_id_for_task.clone(),
                    conversation_id,
                    lsp,
                    update_tx: None,
                };
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => None,
                    result = run_registered_tool_with_retry(
                        tool_runner,
                        &retry_policy,
                        &cancel_token,
                        &event_tx,
                        &node_id_for_task,
                        &call,
                        ctx,
                    ) => Some(result),
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
                    self.record_tool_file_changes(effects, node_id, &record);
                    self.record_tool_reads(effects, node_id, &record);
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
        let mut collected = Vec::with_capacity(results.len());
        for result in results {
            collected.push(result?);
        }
        Some(collected)
    }

    fn exclusive_lock_keys_for(&self, node_id: &NodeId, call: &ToolCall) -> Vec<String> {
        let Ok(registered) = self.tool_runner.registry().get(&call.name) else {
            return Vec::new();
        };
        exclusive_lock_keys(
            registered.kind,
            registered.definition.concurrency,
            node_id,
            call,
        )
    }

    async fn exclusive_permits(
        &self,
        node_id: &NodeId,
        call: &ToolCall,
    ) -> Vec<tokio::sync::OwnedSemaphorePermit> {
        let keys = self.exclusive_lock_keys_for(node_id, call);
        self.exclusive_locks.acquire(keys).await
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
        effects: &mut ToolBatchEffects,
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
            .execute_tool_or_cancel(effects, tool_call.clone(), node_id, &node_id.0)
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
                self.record_tool_file_changes(effects, node_id, &record);
                self.record_tool_reads(effects, node_id, &record);
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
        effects: &mut ToolBatchEffects,
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
        effects
            .file_changes
            .extend(record.file_changes.iter().cloned());
        for change in &record.file_changes {
            let _ = self.event_tx.send(ExecutionEvent::FileChanged {
                node_id: node_id.clone(),
                record: change.clone(),
            });
        }
    }

    fn record_tool_reads(
        &self,
        effects: &mut ToolBatchEffects,
        _node_id: &NodeId,
        record: &ToolExecutionRecord,
    ) {
        if record.reads.is_empty() {
            return;
        }
        effects.reads.extend(record.reads.iter().cloned());
    }

    fn note_read_call(&self, effects: &mut ToolBatchEffects, tool_call: &ToolCall) {
        if tool_call.name != "read" {
            return;
        }
        let Some(path) = tool_call
            .arguments
            .get("path")
            .and_then(|value| value.as_str())
        else {
            return;
        };
        let (path, _) = split_selector(path);
        if path.starts_with("http://")
            || path.starts_with("https://")
            || path.starts_with("artifact:")
        {
            return;
        }
        effects.read_call_paths.push(path.to_string());
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
        effects: &mut ToolBatchEffects,
        tool_call: ToolCall,
        node_id: &NodeId,
        conversation_id: &str,
    ) -> Option<Result<ToolExecutionRecord, ToolRunnerError>> {
        self.note_read_call(effects, &tool_call);

        let tool_runner = Arc::clone(&self.tool_runner);

        let tool_name = tool_call.name.clone();

        let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel();

        let node_id_for_task = node_id.clone();

        let ctx = ToolExecutionContext {
            node_id: node_id_for_task,
            conversation_id: conversation_id.to_string(),
            lsp: self.lsp.clone(),
            update_tx: Some(update_tx),
        };

        let event_tx = self.event_tx.clone();

        let update_node_id = node_id.clone();

        let update_tool_call_id = tool_call.id.clone();

        let update_tool_name = tool_call.name.clone();

        tokio::spawn(async move {
            while let Some(update) = update_rx.recv().await {
                let _ = event_tx.send(ExecutionEvent::ToolUpdated {
                    node_id: update_node_id.clone(),
                    tool_call_id: update_tool_call_id.clone(),
                    tool_name: update_tool_name.clone(),
                    content: update.content,
                    output_meta: update.output_meta,
                });
            }
        });
        let wait_started = Instant::now();
        let exclusive_permits = self.exclusive_permits(node_id, &tool_call).await;
        maybe_emit_exclusive_wait(&self.event_tx, node_id, &tool_name, wait_started.elapsed());
        let started = Instant::now();
        let node_token = self.node_interrupt_token(node_id);
        let policy = self.workflow.settings.retry_policy.clone();
        let cancel_for_retry = self.cancel_token.clone();
        let run_tool = run_registered_tool_with_retry(
            tool_runner,
            &policy,
            &cancel_for_retry,
            &self.event_tx,
            node_id,
            &tool_call,
            ctx,
        );
        let result = match node_token {
            Some(node_token) => {
                tokio::select! {
                    biased;
                    _ = self.cancel_token.cancelled() => {
                        abort_run(&self.event_tx, self.aborted_emitted.as_ref());
                        None
                    }
                    _ = node_token.cancelled() => {
                        effects.interrupted = true;
                        None
                    }
                    result = run_tool => Some(result),
                }
            }
            None => {
                tokio::select! {
                    biased;
                    _ = self.cancel_token.cancelled() => {
                        abort_run(&self.event_tx, self.aborted_emitted.as_ref());
                        None
                    }
                    result = run_tool => Some(result),
                }
            }
        };
        drop(exclusive_permits);
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

pub(super) fn send_run_telemetry(
    event_tx: &UnboundedSender<ExecutionEvent>,
    events: impl IntoIterator<Item = RunTelemetry>,
) {
    for event in events {
        let _ = event_tx.send(event);
    }
}

fn render_tool_error(error: ToolRunnerError) -> String {
    error.to_string()
}

/// Run-wide lock table for exclusive tool calls, keyed by lock key
/// (`path:<p>`, `tool:<name>`, or node-scoped tool keys). Keys must be
/// pre-sorted (they are, by `exclusive_lock_keys`) so overlapping
/// acquisitions cannot deadlock.
#[derive(Default)]
struct ExclusiveLocks {
    semaphores: parking_lot::Mutex<BTreeMap<String, Arc<Semaphore>>>,
}

impl ExclusiveLocks {
    async fn acquire(&self, keys: Vec<String>) -> Vec<tokio::sync::OwnedSemaphorePermit> {
        let mut permits = Vec::with_capacity(keys.len());
        for key in keys {
            let semaphore = {
                let mut semaphores = self.semaphores.lock();
                semaphores
                    .entry(key)
                    .or_insert_with(|| Arc::new(Semaphore::new(1)))
                    .clone()
            };
            if let Ok(permit) = semaphore.acquire_owned().await {
                permits.push(permit);
            }
        }
        permits
    }
}

/// Normalize a tool-supplied relative path for use as a lock key.
/// This is lexical only — the runner still does real path validation.
fn normalize_lock_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn path_keys(paths: Vec<String>) -> Vec<String> {
    let mut keys: Vec<String> = paths
        .into_iter()
        .map(|path| format!("path:{}", normalize_lock_path(&path)))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

fn hashline_paths(input: &str) -> Vec<String> {
    input
        .lines()
        .filter_map(|line| line.strip_prefix('¶'))
        .filter_map(|rest| rest.split('#').next())
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

fn apply_patch_paths(input: &str) -> Vec<String> {
    const MARKERS: [&str; 3] = ["*** Update File: ", "*** Add File: ", "*** Delete File: "];
    input
        .lines()
        .filter_map(|line| MARKERS.iter().find_map(|marker| line.strip_prefix(marker)))
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect()
}

fn tool_fallback_key(concurrency: ToolConcurrency, node_id: &NodeId, tool_name: &str) -> String {
    match concurrency {
        ToolConcurrency::NodeExclusive => format!("{}\u{1f}tool:{}", node_id.0, tool_name),
        _ => format!("tool:{}", tool_name),
    }
}

/// Lock keys a call must hold before executing. Empty = no cross-node lock.
/// Keys are sorted; acquiring in order prevents deadlock between calls
/// that lock overlapping path sets.
fn exclusive_lock_keys(
    kind: crate::tool::registry::BuiltinToolKind,
    concurrency: ToolConcurrency,
    node_id: &NodeId,
    call: &ToolCall,
) -> Vec<String> {
    use crate::tool::registry::BuiltinToolKind as Kind;
    if concurrency == ToolConcurrency::Shared {
        return Vec::new();
    }
    let fallback = vec![tool_fallback_key(concurrency, node_id, &call.name)];
    if concurrency == ToolConcurrency::NodeExclusive {
        return fallback;
    }
    match kind {
        Kind::Write => match call.arguments.get("path").and_then(|v| v.as_str()) {
            Some(path) => path_keys(vec![path.to_string()]),
            None => fallback,
        },
        Kind::Edit => {
            if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
                return path_keys(vec![path.to_string()]);
            }
            if let Some(input) = call.arguments.get("input").and_then(|v| v.as_str()) {
                let paths = hashline_paths(input);
                if !paths.is_empty() {
                    return path_keys(paths);
                }
            }
            fallback
        }
        Kind::ApplyPatch => {
            let paths = call
                .arguments
                .get("input")
                .and_then(|v| v.as_str())
                .map(apply_patch_paths)
                .unwrap_or_default();
            if paths.is_empty() {
                fallback
            } else {
                path_keys(paths)
            }
        }
        _ => fallback,
    }
}

/// Queue waits shorter than this are normal scheduling jitter, not contention.
const EXCLUSIVE_WAIT_REPORT_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(100);

fn maybe_emit_exclusive_wait(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    tool_name: &str,
    waited: std::time::Duration,
) {
    if waited < EXCLUSIVE_WAIT_REPORT_THRESHOLD {
        return;
    }
    let duration_ms = waited.as_millis() as u64;
    log::info!("[perf] tool-wait · {tool_name}: {duration_ms}ms");
    let _ = event_tx.send(RunTelemetry::PhaseTimed {
        phase: "tool-wait".to_string(),
        label: tool_name.to_string(),
        node_id: Some(node_id.clone()),
        duration_ms,
    });
}

fn emit_tool_retrying(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    tool_call_id: &str,
    tool_name: &str,
    attempt: u8,
    delay: std::time::Duration,
) {
    let _ = event_tx.send(ExecutionEvent::ToolRetrying {
        node_id: node_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        attempt,
        backoff_ms: delay.as_millis() as u64,
    });
}

async fn run_registered_tool_with_retry(
    tool_runner: Arc<ToolRunner>,
    policy: &engine::RetryPolicy,
    cancel_token: &CancellationToken,
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    call: &ToolCall,
    ctx: ToolExecutionContext,
) -> Result<ToolExecutionRecord, ToolRunnerError> {
    let node_id = node_id.clone();
    let tool_call_id = call.id.clone();
    let tool_name = call.name.clone();
    let event_tx = event_tx.clone();
    execute_with_retry(
        policy,
        cancel_token,
        |attempt, delay| {
            emit_tool_retrying(
                &event_tx,
                &node_id,
                &tool_call_id,
                &tool_name,
                attempt,
                delay,
            );
        },
        || {
            let tool_runner = Arc::clone(&tool_runner);
            let call = call.clone();
            let ctx = ctx.clone();
            async move { tool_runner.execute(call, Some(ctx)).await }
        },
    )
    .await
}

#[path = "subagent_session.rs"]
mod subagent_session;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::registry::BuiltinToolKind;
    use std::time::Duration;

    fn node(id: &str) -> NodeId {
        NodeId(id.to_string())
    }

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "tc1".to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn shared_tools_take_no_lock() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Read,
            ToolConcurrency::Shared,
            &node("a"),
            &call("read", serde_json::json!({"path": "a.txt"})),
        );
        assert!(keys.is_empty());
    }

    #[test]
    fn bash_falls_back_to_node_scoped_tool_key() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Bash,
            ToolConcurrency::NodeExclusive,
            &node("n1"),
            &call("bash", serde_json::json!({"command": "cargo test"})),
        );
        assert_eq!(keys, vec!["n1\u{1f}tool:bash".to_string()]);
    }

    #[test]
    fn node_exclusive_bash_differs_per_node() {
        let a = exclusive_lock_keys(
            BuiltinToolKind::Bash,
            ToolConcurrency::NodeExclusive,
            &node("a"),
            &call("bash", serde_json::json!({"command": "echo"})),
        );
        let b = exclusive_lock_keys(
            BuiltinToolKind::Bash,
            ToolConcurrency::NodeExclusive,
            &node("b"),
            &call("bash", serde_json::json!({"command": "echo"})),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn write_locks_its_path() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Write,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call(
                "write",
                serde_json::json!({"path": "./src/lib.rs", "content": "x"}),
            ),
        );
        assert_eq!(keys, vec!["path:src/lib.rs".to_string()]);
    }

    #[test]
    fn edit_replace_mode_locks_its_path() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Edit,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call(
                "edit",
                serde_json::json!({"path": "src/a.rs", "edits": [{"old_text": "x", "new_text": "y"}]}),
            ),
        );
        assert_eq!(keys, vec!["path:src/a.rs".to_string()]);
    }

    #[test]
    fn edit_hashline_mode_locks_each_section_path_sorted_deduped() {
        let input = "¶src/b.rs#A1\nline\n¶src/a.rs#B2\nline\n¶src/a.rs#C3\nline\n";
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Edit,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("edit", serde_json::json!({"input": input})),
        );
        assert_eq!(
            keys,
            vec!["path:src/a.rs".to_string(), "path:src/b.rs".to_string()]
        );
    }

    #[test]
    fn edit_with_unparsable_args_falls_back_to_tool_key() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Edit,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("edit", serde_json::json!({})),
        );
        assert_eq!(keys, vec!["tool:edit".to_string()]);
    }

    #[test]
    fn apply_patch_locks_paths_from_envelope() {
        let input = "*** Begin Patch\n*** Update File: src/b.rs\n@@\n-x\n+y\n*** Add File: src/a.rs\n+new\n*** End Patch";
        let keys = exclusive_lock_keys(
            BuiltinToolKind::ApplyPatch,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("apply_patch", serde_json::json!({"input": input})),
        );
        assert_eq!(
            keys,
            vec!["path:src/a.rs".to_string(), "path:src/b.rs".to_string()]
        );
    }

    #[test]
    fn apply_patch_without_recognizable_paths_falls_back_to_tool_key() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::ApplyPatch,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("apply_patch", serde_json::json!({"input": "garbage"})),
        );
        assert_eq!(keys, vec!["tool:apply_patch".to_string()]);
    }

    #[test]
    fn exclusive_mcp_tool_keeps_per_tool_lock() {
        let keys = exclusive_lock_keys(
            BuiltinToolKind::Mcp,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("some_mcp_tool", serde_json::json!({})),
        );
        assert_eq!(keys, vec!["tool:some_mcp_tool".to_string()]);
    }

    #[test]
    fn exclusive_tools_on_different_nodes_share_path_locks() {
        let a = exclusive_lock_keys(
            BuiltinToolKind::Edit,
            ToolConcurrency::Exclusive,
            &node("a"),
            &call("edit", serde_json::json!({"path": "src/a.rs"})),
        );
        let b = exclusive_lock_keys(
            BuiltinToolKind::Edit,
            ToolConcurrency::Exclusive,
            &node("b"),
            &call("edit", serde_json::json!({"path": "src/a.rs"})),
        );
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn permits_for_disjoint_paths_do_not_contend() {
        let locks = ExclusiveLocks::default();
        let a = locks.acquire(vec!["path:src/a.rs".to_string()]).await;
        let b = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            locks.acquire(vec!["path:src/b.rs".to_string()]),
        )
        .await
        .expect("disjoint path lock must not block");
        drop(a);
        drop(b);
    }

    #[tokio::test]
    async fn permits_for_same_path_serialize() {
        let locks = ExclusiveLocks::default();
        let a = locks.acquire(vec!["path:src/a.rs".to_string()]).await;
        let contended = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            locks.acquire(vec!["path:src/a.rs".to_string()]),
        )
        .await;
        assert!(contended.is_err(), "same path must block until released");
        drop(a);
        let b = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            locks.acquire(vec!["path:src/a.rs".to_string()]),
        )
        .await
        .expect("released path must be acquirable");
        drop(b);
    }

    #[tokio::test]
    async fn empty_key_list_acquires_nothing() {
        let locks = ExclusiveLocks::default();
        let permits = locks.acquire(Vec::new()).await;
        assert!(permits.is_empty());
    }

    #[test]
    fn exclusive_wait_below_threshold_is_silent() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        maybe_emit_exclusive_wait(&tx, &node("n1"), "bash", Duration::from_millis(5));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn exclusive_wait_above_threshold_emits_phase_timed() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        maybe_emit_exclusive_wait(&tx, &node("n1"), "bash", Duration::from_millis(250));
        match rx.try_recv().expect("expected a PhaseTimed event") {
            RunTelemetry::PhaseTimed {
                phase,
                label,
                node_id,
                duration_ms,
            } => {
                assert_eq!(phase, "tool-wait");
                assert_eq!(label, "bash");
                assert_eq!(node_id, Some(node("n1")));
                assert_eq!(duration_ms, 250);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
