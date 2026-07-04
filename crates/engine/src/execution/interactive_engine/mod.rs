mod checkpoint;
mod completion;
#[cfg(test)]
mod tests;
mod tools;

pub use checkpoint::{validate_checkpoint_against_workflow, InteractiveEngineCheckpoint};

use crate::conversation::{AgentTranscriptItem, ChatMessage, ChatRole};
use crate::execution::node_invocation::{
    build_agent_request, build_upstream_map, NodeInvocationContext,
};
use crate::execution::tool_results::error_tool_result;
use crate::execution::{NodeFailureKind, NodeRunOutput, RunError, RunReport};
use crate::graph::validation::{execution_layers, WorkflowValidationError};
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::{AgentRequest, AiPort, ToolBatchOutput, ToolPort};
use crate::tools::{FileChangeRecord, ReadRecord, ToolCall};
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// Pause payload when a node needs human text input.
#[derive(Debug, Clone)]
pub struct EngineAwaitInput {
    pub node_id: NodeId,
    pub label: String,
    pub context: String,
    pub is_initial: bool,
}

/// Pause payload when a node needs tool approval.
#[derive(Debug, Clone)]
pub struct EngineAwaitApproval {
    pub approval_id: String,
    pub node_id: NodeId,
    pub label: String,
    pub tool_calls: Vec<ToolCall>,
}

/// Pause payload when a node failed or was interrupted and can be retried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRetryableNode {
    pub node_id: NodeId,
    pub label: String,
    pub error: String,
    pub interrupted: bool,
}

/// Terminal or pause outcome from [`InteractiveEngine::run`].
#[derive(Debug, Clone)]
pub enum EngineRunResult {
    /// One or more nodes paused for human input, tool approval, and/or retry.
    NeedsInteraction {
        inputs: Vec<EngineAwaitInput>,
        approvals: Vec<EngineAwaitApproval>,
        retryables: Vec<EngineRetryableNode>,
    },
    Completed(RunReport),
    Failed(RunError),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineInputError {
    #[error("no node awaiting input")]
    NoNodeAwaiting,
    #[error("expected input for {expected}, got {got}")]
    WrongNode { expected: NodeId, got: NodeId },
    #[error("no node awaiting tool results")]
    NoPendingTools,
    #[error("unknown approval id {0}")]
    UnknownApproval(String),
    #[error("node {0} is not retryable")]
    NodeNotRetryable(NodeId),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingToolBatch {
    approval_id: String,
    node_id: NodeId,
    tool_calls: Vec<ToolCall>,
    requires_approval: bool,
}

pub struct InteractiveEngine {
    workflow: Workflow,
    upstream_map: HashMap<NodeId, Vec<NodeId>>,
    node_index: HashMap<NodeId, usize>,
    layers: Vec<Vec<NodeId>>,
    layer_idx: usize,
    outputs: BTreeMap<NodeId, Value>,
    changed_files_by_node: BTreeMap<NodeId, Vec<FileChangeRecord>>,
    reads_by_node: BTreeMap<NodeId, Vec<ReadRecord>>,
    read_calls: u32,
    redundant_reads: u32,
    tokens_in: u32,
    transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    awaiting_nodes: BTreeSet<NodeId>,
    in_flight_ai: BTreeSet<NodeId>,
    pending_tool_batches: BTreeMap<String, PendingToolBatch>,
    /// Approval ids of tool batches currently executing (not persisted; a
    /// restored engine re-dispatches any pending non-approval batch).
    in_flight_tools: BTreeSet<String>,
    retries_by_node: BTreeMap<NodeId, u8>,
    /// Per-node: do not dispatch AI again until this instant (transient retry backoff).
    retry_after_by_node: BTreeMap<NodeId, Instant>,
    submit_output_retries_by_node: BTreeMap<NodeId, u8>,
    request_input_retries_by_node: BTreeMap<NodeId, u8>,
    entrypoint_text: Option<String>,
    project_repository_root: Option<String>,
    terminal_error: Option<RunError>,
    interrupted_nodes: BTreeSet<NodeId>,
    failed_nodes: BTreeMap<NodeId, String>,
}

pub(crate) const MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES: u8 = 3;
pub(crate) const MAX_MALFORMED_REQUEST_INPUT_RETRIES: u8 = 3;
pub(crate) const MALFORMED_REQUEST_INPUT_FEEDBACK: &str =
    "Your openflow_request_user_input call must set \
    assistant_message to one direct clarifying question for the human (typically ending with ?). \
    Do not send preamble, narration, or plans — ask the question in assistant_message now.";

enum WorkOutput {
    Ai {
        node_id: NodeId,
        outcome: Result<crate::ports::AgentTurnOutcome, crate::ports::AgentError>,
    },
    Tools {
        approval_id: String,
        node_id: NodeId,
        output: ToolBatchOutput,
    },
}

impl InteractiveEngine {
    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn new(
        workflow: Workflow,
        entrypoint_text: Option<String>,
        project_repository_root: Option<String>,
    ) -> Result<Self, WorkflowValidationError> {
        let layers = execution_layers(&workflow)?;
        let upstream_map = build_upstream_map(&workflow);
        let node_index = workflow
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.clone(), index))
            .collect();
        Ok(Self {
            workflow,
            upstream_map,
            node_index,
            layers,
            layer_idx: 0,
            outputs: BTreeMap::new(),
            changed_files_by_node: BTreeMap::new(),
            reads_by_node: BTreeMap::new(),
            read_calls: 0,
            redundant_reads: 0,
            tokens_in: 0,
            transcripts: BTreeMap::new(),
            awaiting_nodes: BTreeSet::new(),
            in_flight_ai: BTreeSet::new(),
            pending_tool_batches: BTreeMap::new(),
            in_flight_tools: BTreeSet::new(),
            retries_by_node: BTreeMap::new(),
            retry_after_by_node: BTreeMap::new(),
            submit_output_retries_by_node: BTreeMap::new(),
            request_input_retries_by_node: BTreeMap::new(),
            entrypoint_text,
            project_repository_root,
            terminal_error: None,
            interrupted_nodes: BTreeSet::new(),
            failed_nodes: BTreeMap::new(),
        })
    }

    fn current_layer_nodes(&self) -> &[NodeId] {
        self.layers
            .get(self.layer_idx)
            .map_or(&[] as &[NodeId], Vec::as_slice)
    }

    fn current_layer_complete(&self) -> bool {
        self.current_layer_nodes()
            .iter()
            .all(|node_id| self.outputs.contains_key(node_id))
    }

    fn node_has_pending_tools(&self, node_id: &NodeId) -> bool {
        self.pending_tool_batches
            .values()
            .any(|batch| batch.node_id == *node_id)
    }

    fn is_node_blocked(&self, node_id: &NodeId) -> bool {
        self.outputs.contains_key(node_id)
            || self.awaiting_nodes.contains(node_id)
            || self.in_flight_ai.contains(node_id)
            || self.node_has_pending_tools(node_id)
            || self.interrupted_nodes.contains(node_id)
            || self.failed_nodes.contains_key(node_id)
            || self.is_node_in_retry_backoff(node_id)
    }

    fn is_node_in_retry_backoff(&self, node_id: &NodeId) -> bool {
        self.retry_after_by_node
            .get(node_id)
            .is_some_and(|deadline| Instant::now() < *deadline)
    }

    /// Shortest wait until any node leaves transient retry backoff.
    fn earliest_retry_delay(&self) -> Option<Duration> {
        let now = Instant::now();
        self.retry_after_by_node
            .values()
            .filter_map(|deadline| deadline.checked_duration_since(now))
            .min()
    }

    fn prune_elapsed_retries(&mut self) {
        let now = Instant::now();
        self.retry_after_by_node
            .retain(|_, deadline| now < *deadline);
    }

    fn schedule_manual_nodes_in_layer(&mut self) {
        for i in 0..self.current_layer_nodes().len() {
            let node_id = self.layers[self.layer_idx][i].clone();
            if self.is_node_blocked(&node_id) {
                continue;
            }
            let Some(node) = self.find_node(&node_id) else {
                continue;
            };
            if node.agent.auto_start || !self.transcript(&node_id).is_empty() {
                continue;
            }
            self.awaiting_nodes.insert(node_id.clone());
            self.transcripts.entry(node_id).or_default();
        }
    }

    /// Move stale in-flight markers for incomplete layer nodes to interrupted so the host can retry.
    fn recover_stale_in_flight_nodes(&mut self) -> bool {
        let stale: Vec<NodeId> = self
            .in_flight_ai
            .iter()
            .filter(|node_id| {
                self.current_layer_nodes().contains(*node_id)
                    && !self.outputs.contains_key(*node_id)
            })
            .cloned()
            .collect();
        if stale.is_empty() {
            return false;
        }
        for node_id in stale {
            self.in_flight_ai.remove(&node_id);
            if self.interrupted_nodes.contains(&node_id) || self.failed_nodes.contains_key(&node_id)
            {
                continue;
            }
            self.interrupted_nodes.insert(node_id);
        }
        true
    }

    fn find_node(&self, node_id: &NodeId) -> Option<&Node> {
        self.node_index
            .get(node_id)
            .and_then(|index| self.workflow.nodes.get(*index))
    }

    fn try_advance_layer(&mut self) -> Option<RunReport> {
        if !self.current_layer_complete() {
            return None;
        }
        self.layer_idx += 1;
        if self.layer_idx >= self.layers.len() {
            return Some(RunReport {
                workflow_id: self.workflow.id.clone(),
                outputs: std::mem::take(&mut self.outputs)
                    .into_iter()
                    .map(|(node_id, output)| NodeRunOutput { node_id, output })
                    .collect(),
                read_calls: self.read_calls,
                redundant_reads: self.redundant_reads,
                tokens_in: self.tokens_in,
            });
        }
        None
    }

    /// Drive the engine until it needs host interaction or reaches a terminal state.
    ///
    /// All runnable work in the current layer (model calls and tool batches
    /// across nodes) executes concurrently; completions are applied to engine
    /// state one at a time as they arrive.
    pub async fn run<A: AiPort, T: ToolPort>(
        &mut self,
        ai: &A,
        tools: &T,
        cancel: &CancellationToken,
    ) -> EngineRunResult {
        type WorkFuture<'a> = Pin<Box<dyn Future<Output = WorkOutput> + Send + 'a>>;
        let mut in_flight: FuturesUnordered<WorkFuture<'_>> = FuturesUnordered::new();

        loop {
            if cancel.is_cancelled() {
                return EngineRunResult::Cancelled;
            }
            if let Some(error) = self.terminal_error.clone() {
                return EngineRunResult::Failed(error);
            }
            if in_flight.is_empty() {
                if let Some(report) = self.try_advance_layer() {
                    return EngineRunResult::Completed(report);
                }
            }

            for (approval_id, node_id, label, tool_calls) in self.take_ready_tool_batches() {
                in_flight.push(Box::pin(async move {
                    let output = tools.execute_batch(&node_id, &label, tool_calls).await;
                    WorkOutput::Tools {
                        approval_id,
                        node_id,
                        output,
                    }
                }));
            }
            for (node_id, mut request) in self.gather_call_ai_actions() {
                tools.augment_request(&node_id, &mut request);
                in_flight.push(Box::pin(async move {
                    let outcome = ai.invoke(request).await;
                    WorkOutput::Ai { node_id, outcome }
                }));
            }
            if let Some(error) = self.terminal_error.clone() {
                return EngineRunResult::Failed(error);
            }

            if in_flight.is_empty() {
                if let Some(delay) = self.earliest_retry_delay() {
                    tokio::select! {
                        biased;
                        () = cancel.cancelled() => return EngineRunResult::Cancelled,
                        () = tokio::time::sleep(delay) => {
                            self.prune_elapsed_retries();
                            continue;
                        }
                    }
                }
                self.schedule_manual_nodes_in_layer();
                self.recover_stale_in_flight_nodes();
                let inputs = self.gather_await_inputs();
                let approvals = self.gather_await_approvals();
                let retryables = self.gather_retryable_nodes();
                if !inputs.is_empty() || !approvals.is_empty() || !retryables.is_empty() {
                    return EngineRunResult::NeedsInteraction {
                        inputs,
                        approvals,
                        retryables,
                    };
                }
                if let Some(report) = self.try_advance_layer() {
                    return EngineRunResult::Completed(report);
                }
                return EngineRunResult::Failed(RunError::NodeFailed {
                    node_id: self
                        .current_layer_nodes()
                        .first()
                        .cloned()
                        .unwrap_or_else(|| NodeId("engine".to_string())),
                    kind: NodeFailureKind::NoRunnableNodesInLayer,
                });
            }

            let work = tokio::select! {
                biased;
                () = cancel.cancelled() => return EngineRunResult::Cancelled,
                Some(work) = in_flight.next() => work,
            };
            match work {
                WorkOutput::Ai { node_id, outcome } => {
                    self.on_ai_complete(&node_id, outcome);
                }
                WorkOutput::Tools {
                    approval_id,
                    node_id,
                    output,
                } => {
                    self.in_flight_tools.remove(&approval_id);
                    if let Err(error) = self.apply_tool_batch_output(&node_id, output) {
                        return EngineRunResult::Failed(RunError::NodeFailed {
                            node_id,
                            kind: NodeFailureKind::EngineInput(error.to_string()),
                        });
                    }
                }
            }
        }
    }

    /// All non-approval tool batches not yet dispatched; marks them in flight.
    fn take_ready_tool_batches(&mut self) -> Vec<(String, NodeId, String, Vec<ToolCall>)> {
        let ready: Vec<(String, NodeId, String, Vec<ToolCall>)> = self
            .pending_tool_batches
            .iter()
            .filter(|(approval_id, batch)| {
                !batch.requires_approval && !self.in_flight_tools.contains(*approval_id)
            })
            .filter_map(|(approval_id, batch)| {
                let node = self.find_node(&batch.node_id)?;
                Some((
                    approval_id.clone(),
                    batch.node_id.clone(),
                    node.label.clone(),
                    batch.tool_calls.clone(),
                ))
            })
            .collect();
        for (approval_id, ..) in &ready {
            self.in_flight_tools.insert(approval_id.clone());
        }
        ready
    }

    fn gather_call_ai_actions(&mut self) -> Vec<(NodeId, AgentRequest)> {
        let mut actions = Vec::new();
        for i in 0..self.current_layer_nodes().len() {
            let node_id = self.layers[self.layer_idx][i].clone();
            if self.is_node_blocked(&node_id) {
                continue;
            }
            let Some(node) = self.find_node(&node_id) else {
                continue;
            };
            if let Some(missing) = self.missing_upstream_outputs(&node_id) {
                self.terminal_error = Some(RunError::NodeFailed {
                    node_id,
                    kind: NodeFailureKind::MissingUpstreamOutput(missing),
                });
                break;
            }
            if !node.agent.auto_start && self.transcript(&node_id).is_empty() {
                continue;
            }
            self.in_flight_ai.insert(node_id.clone());
            match self.build_request(&node_id) {
                Ok(request) => actions.push((node_id, request)),
                Err(error) => {
                    self.in_flight_ai.remove(&node_id);
                    self.terminal_error = Some(error);
                    break;
                }
            }
        }
        actions
    }

    fn gather_await_inputs(&self) -> Vec<EngineAwaitInput> {
        self.awaiting_nodes
            .iter()
            .filter_map(|awaiting_id| {
                let node = self.find_node(awaiting_id)?;
                Some(EngineAwaitInput {
                    node_id: node.id.clone(),
                    label: node.label.clone(),
                    context: self.assemble_context(&node.id),
                    is_initial: self.conversation_history(&node.id).is_empty(),
                })
            })
            .collect()
    }

    fn gather_await_approvals(&self) -> Vec<EngineAwaitApproval> {
        self.pending_tool_batches
            .values()
            .filter(|batch| batch.requires_approval)
            .filter_map(|batch| {
                let node = self.find_node(&batch.node_id)?;
                Some(EngineAwaitApproval {
                    approval_id: batch.approval_id.clone(),
                    node_id: node.id.clone(),
                    label: node.label.clone(),
                    tool_calls: batch.tool_calls.clone(),
                })
            })
            .collect()
    }

    fn gather_retryable_nodes(&self) -> Vec<EngineRetryableNode> {
        let mut retryables = Vec::new();
        for node_id in &self.interrupted_nodes {
            if let Some(node) = self.find_node(node_id) {
                retryables.push(EngineRetryableNode {
                    node_id: node_id.clone(),
                    label: node.label.clone(),
                    error: "interrupted by user".to_string(),
                    interrupted: true,
                });
            }
        }
        for (node_id, error) in &self.failed_nodes {
            if let Some(node) = self.find_node(node_id) {
                retryables.push(EngineRetryableNode {
                    node_id: node_id.clone(),
                    label: node.label.clone(),
                    error: error.clone(),
                    interrupted: false,
                });
            }
        }
        retryables
    }

    /// Current 1-based model invocation attempt for `node_id` (retries increment).
    #[must_use]
    pub fn model_attempt_for_node(&self, node_id: &NodeId) -> u8 {
        self.model_attempt_for(node_id)
    }

    /// Mark a node interrupted by the user while tools are executing.
    pub fn mark_node_interrupted(&mut self, node_id: &NodeId) {
        if self.interrupted_nodes.contains(node_id) {
            return;
        }
        let dangling_calls: Vec<ToolCall> = self
            .pending_tool_batches
            .values()
            .filter(|batch| batch.node_id == *node_id)
            .flat_map(|batch| batch.tool_calls.iter().cloned())
            .collect();
        if !dangling_calls.is_empty() {
            let transcript = self.transcripts.entry(node_id.clone()).or_default();
            for call in &dangling_calls {
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: error_tool_result(
                        call,
                        "tool execution did not complete (interrupted or cancelled)",
                    ),
                });
            }
        }
        self.pending_tool_batches
            .retain(|_, batch| batch.node_id != *node_id);
        self.interrupted_nodes.insert(node_id.clone());
    }

    /// Apply a completed tool batch: record effects, then feed results back.
    fn apply_tool_batch_output(
        &mut self,
        node_id: &NodeId,
        output: ToolBatchOutput,
    ) -> Result<(), EngineInputError> {
        for path in &output.effects.read_call_paths {
            self.note_read_call(node_id, path);
        }
        self.record_file_changes(node_id, output.effects.file_changes.clone());
        self.record_reads(node_id, output.effects.reads.clone());
        if output.effects.interrupted {
            self.mark_node_interrupted(node_id);
            return Ok(());
        }
        if self.interrupted_nodes.contains(node_id) {
            return Ok(());
        }
        self.on_tool_results(node_id, output.results)
    }

    #[must_use]
    pub fn pending_tool_batch_node(&self, approval_id: &str) -> Option<NodeId> {
        self.pending_tool_batches
            .get(approval_id)
            .map(|batch| batch.node_id.clone())
    }

    /// Retry a failed or interrupted node without clearing its transcript.
    ///
    /// # Errors
    /// Returns [`EngineInputError::NodeNotRetryable`] when the node is not paused for retry.
    pub fn retry_node(&mut self, node_id: &NodeId) -> Result<(), EngineInputError> {
        if !self.node_index.contains_key(node_id) {
            return Err(EngineInputError::NodeNotRetryable(node_id.clone()));
        }
        if !self.failed_nodes.contains_key(node_id) && !self.interrupted_nodes.contains(node_id) {
            return Err(EngineInputError::NodeNotRetryable(node_id.clone()));
        }
        self.failed_nodes.remove(node_id);
        self.interrupted_nodes.remove(node_id);
        self.retry_after_by_node.remove(node_id);
        let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
        *retry_count = retry_count.saturating_add(1);
        Ok(())
    }

    /// # Errors
    /// Returns an error if no node is awaiting input or the wrong node id is provided.
    pub fn on_human_input(&mut self, node_id: &NodeId, text: &str) -> Result<(), EngineInputError> {
        if !self.awaiting_nodes.remove(node_id) {
            let expected = self
                .awaiting_nodes
                .iter()
                .next()
                .cloned()
                .ok_or(EngineInputError::NoNodeAwaiting)?;
            return Err(EngineInputError::WrongNode {
                expected,
                got: node_id.clone(),
            });
        }
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: text.to_string(),
            },
        );
        Ok(())
    }

    pub fn record_file_changes(&mut self, node_id: &NodeId, records: Vec<FileChangeRecord>) {
        if records.is_empty() {
            return;
        }
        self.changed_files_by_node
            .entry(node_id.clone())
            .or_default()
            .extend(records);
    }

    pub fn record_reads(&mut self, node_id: &NodeId, records: Vec<ReadRecord>) {
        if records.is_empty() {
            return;
        }
        let node_reads = self.reads_by_node.entry(node_id.clone()).or_default();
        let mut by_path = node_reads
            .iter()
            .map(|record| (record.path.clone(), record.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        for record in records {
            crate::tools::merge_read_record(&mut by_path, record);
        }
        *node_reads = by_path.into_values().collect();
    }

    pub fn note_read_call(&mut self, node_id: &NodeId, path: &str) {
        self.read_calls = self.read_calls.saturating_add(1);
        let upstream =
            crate::execution::upstream_reads(&node_id.0, &self.upstream_map, &self.reads_by_node);
        if upstream.iter().any(|record| record.path == path) {
            self.redundant_reads = self.redundant_reads.saturating_add(1);
        }
    }

    pub const fn note_usage(&mut self, usage: &crate::UsageReport) {
        self.tokens_in = self.tokens_in.saturating_add(usage.prompt_tokens);
    }

    pub fn revert_file_changes_for_batch(&mut self, batch_id: &str, node_id: &NodeId) {
        if let Some(records) = self.changed_files_by_node.get_mut(node_id) {
            records.retain(|record| record.batch_id.as_deref() != Some(batch_id));
        }
    }

    #[must_use]
    pub fn node_output(&self, node_id: &NodeId) -> Option<Value> {
        self.outputs.get(node_id).cloned()
    }

    #[must_use]
    pub fn conversation_history(&self, node_id: &NodeId) -> Vec<ChatMessage> {
        self.transcript(node_id)
            .iter()
            .filter_map(|item| match item {
                AgentTranscriptItem::AssistantMessage { content } => {
                    Some(ChatMessage::text(ChatRole::Assistant, content.clone()))
                }
                AgentTranscriptItem::UserMessage { content } => {
                    Some(ChatMessage::text(ChatRole::User, content.clone()))
                }
                AgentTranscriptItem::ToolCall { .. } | AgentTranscriptItem::ToolResult { .. } => {
                    None
                }
            })
            .collect()
    }

    #[must_use]
    pub fn transcript(&self, node_id: &NodeId) -> &[AgentTranscriptItem] {
        self.transcripts.get(node_id).map_or(&[], Vec::as_slice)
    }

    fn build_request(&self, node_id: &NodeId) -> Result<AgentRequest, RunError> {
        let node = self
            .find_node(node_id)
            .ok_or_else(|| RunError::NodeFailed {
                node_id: node_id.clone(),
                kind: NodeFailureKind::NodeMustExist,
            })?;
        let ctx = NodeInvocationContext {
            workflow: &self.workflow,
            upstream_map: &self.upstream_map,
            outputs: &self.outputs,
            changed_files_by_node: &self.changed_files_by_node,
            reads_by_node: &self.reads_by_node,
            entrypoint_text: self.entrypoint_text.as_deref(),
            transcript: self.transcript(&node.id),
            available_tools: &[],
            project_repository_root: self.project_repository_root.as_deref(),
        };
        let mut request = build_agent_request(&ctx, node, true)?;
        request.model_attempt = self.model_attempt_for(&node.id);
        Ok(request)
    }

    fn model_attempt_for(&self, node_id: &NodeId) -> u8 {
        let retries = self.retries_by_node.get(node_id).copied().unwrap_or(0);
        retries.saturating_add(1)
    }

    fn assemble_context(&self, node_id: &NodeId) -> String {
        let upstream = self.upstream_map.get(node_id).cloned().unwrap_or_default();
        let mut context = String::new();
        for upstream_id in &upstream {
            if let Some(output) = self.outputs.get(upstream_id) {
                let _ = writeln!(context, "{upstream_id}: {output}");
            }
        }
        if context.is_empty() {
            if let Some(text) = self.entrypoint_text.as_deref() {
                let _ = writeln!(context, "Entrypoint: {text}");
            }
        }
        if let Some(node) = self.find_node(node_id) {
            let _ = write!(context, "\nTask: {}", node.agent.task_prompt);
        }
        context
    }

    fn missing_upstream_outputs(&self, node_id: &NodeId) -> Option<Vec<NodeId>> {
        let missing = self
            .upstream_map
            .get(node_id)?
            .iter()
            .filter(|upstream_id| !self.outputs.contains_key(*upstream_id))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            None
        } else {
            Some(missing)
        }
    }

    fn reject_misrouted_completion(&mut self, node_id: &NodeId, message: String) {
        self.terminal_error = Some(RunError::NodeFailed {
            node_id: node_id.clone(),
            kind: NodeFailureKind::MisroutedCompletion(message),
        });
    }
}

#[cfg(test)]
impl InteractiveEngine {
    pub(crate) fn test_insert_in_flight(&mut self, node_id: NodeId) {
        self.in_flight_ai.insert(node_id);
    }

    pub(crate) fn test_insert_output(&mut self, node_id: NodeId, output: Value) {
        self.outputs.insert(node_id, output);
    }

    pub(crate) fn test_insert_pending_batch(&mut self, batch: PendingToolBatch) {
        self.pending_tool_batches
            .insert(batch.approval_id.clone(), batch);
    }

    pub(crate) fn test_is_in_retry_backoff(&self, node_id: &NodeId) -> bool {
        self.is_node_in_retry_backoff(node_id)
    }

    pub(crate) fn test_gather_ai_node_ids(&mut self) -> Vec<NodeId> {
        self.gather_call_ai_actions()
            .into_iter()
            .map(|(node_id, _)| node_id)
            .collect()
    }
}
