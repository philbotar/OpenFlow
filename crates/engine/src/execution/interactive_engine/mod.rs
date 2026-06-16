mod checkpoint;
mod completion;
#[cfg(test)]
mod tests;
mod tools;

pub use checkpoint::{
    collect_checkpoint_node_ids, validate_checkpoint_against_workflow, CheckpointError,
    InteractiveEngineCheckpoint,
};

use crate::conversation::{AgentTranscriptItem, ChatMessage, ChatRole};
use crate::execution::node_invocation::{
    build_agent_request, build_upstream_map, NodeInvocationContext,
};
use crate::execution::{
    NodeFailureKind, NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport,
};
use crate::graph::validation::{execution_layers, WorkflowValidationError};
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::{AgentRequest, AiPort, ToolPort};
use crate::tools::{FileChangeRecord, ToolCall};
use futures::future::join_all;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use std::time::Duration;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
/// Next action the host runtime must perform after [`InteractiveEngine::poll`].
#[derive(Debug, Clone)]
pub enum EnginePollResult {
    /// Invoke the model with the enclosed request, then call [`InteractiveEngine::on_ai_complete`].
    CallAi {
        node_id: NodeId,
        request: Box<AgentRequest>,
    },
    /// Pause until the user submits text via [`InteractiveEngine::on_human_input`].
    AwaitInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    /// Pause until the user approves or denies tools via [`InteractiveEngine::on_tool_decision`].
    AwaitToolApproval {
        approval_id: String,
        node_id: NodeId,
        label: String,
        tool_calls: Vec<ToolCall>,
    },
    /// Run tools without approval, then call [`InteractiveEngine::on_tool_results`].
    RunTools {
        node_id: NodeId,
        label: String,
        tool_calls: Vec<ToolCall>,
    },
    /// Pause until the host retries the node via [`InteractiveEngine::retry_node`].
    AwaitRetry(EngineRetryableNode),
    /// Workflow finished successfully.
    Completed(RunReport),
    /// Workflow stopped with a terminal error; further polls return the same failure.
    Failed(RunError),
}

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
    transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    events: Vec<RunEvent>,
    queued_nodes: BTreeSet<NodeId>,
    started_invocations_by_node: BTreeMap<NodeId, u8>,
    awaiting_nodes: BTreeSet<NodeId>,
    in_flight_ai: BTreeSet<NodeId>,
    pending_tool_batches: BTreeMap<String, PendingToolBatch>,
    retries_by_node: BTreeMap<NodeId, u8>,
    pending_retry_delay: Option<Duration>,
    submit_output_retries_by_node: BTreeMap<NodeId, u8>,
    request_input_retries_by_node: BTreeMap<NodeId, u8>,
    entrypoint_text: Option<String>,
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

impl InteractiveEngine {
    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn new(
        workflow: Workflow,
        entrypoint_text: Option<String>,
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
            transcripts: BTreeMap::new(),
            events: Vec::new(),
            queued_nodes: BTreeSet::new(),
            started_invocations_by_node: BTreeMap::new(),
            awaiting_nodes: BTreeSet::new(),
            in_flight_ai: BTreeSet::new(),
            pending_tool_batches: BTreeMap::new(),
            retries_by_node: BTreeMap::new(),
            pending_retry_delay: None,
            submit_output_retries_by_node: BTreeMap::new(),
            request_input_retries_by_node: BTreeMap::new(),
            entrypoint_text,
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
    }

    fn schedule_manual_nodes_in_layer(&mut self) {
        for node_id in self.current_layer_nodes().to_vec() {
            if self.is_node_blocked(&node_id) {
                continue;
            }
            let Some(node) = self.find_node(&node_id) else {
                continue;
            };
            if node.agent.auto_start || !self.transcript(&node_id).is_empty() {
                continue;
            }
            if self.queued_nodes.insert(node_id.clone()) {
                self.events.push(RunEvent {
                    node_id: node_id.clone(),
                    kind: RunEventKind::Queued,
                    message: "queued".to_string(),
                    output: None,
                });
            }
            self.awaiting_nodes.insert(node_id.clone());
            self.transcripts.entry(node_id.clone()).or_default();
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
            self.interrupted_nodes.insert(node_id.clone());
            self.events.push(RunEvent {
                node_id: node_id.clone(),
                kind: RunEventKind::Failed,
                message: "model invocation did not complete; node is retryable".to_string(),
                output: None,
            });
        }
        true
    }

    fn layer_retryables(&self) -> Vec<EngineRetryableNode> {
        self.gather_retryable_nodes()
            .into_iter()
            .filter(|retryable| {
                self.current_layer_nodes().contains(&retryable.node_id)
                    && !self.outputs.contains_key(&retryable.node_id)
            })
            .collect()
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
                events: std::mem::take(&mut self.events),
                outputs: std::mem::take(&mut self.outputs)
                    .into_iter()
                    .map(|(node_id, output)| NodeRunOutput { node_id, output })
                    .collect(),
            });
        }
        None
    }

    /// Advance the engine one step and return the next action for the host runtime.
    ///
    /// Call repeatedly until the result is [`EnginePollResult::Completed`] or
    /// [`EnginePollResult::Failed`]. For [`EnginePollResult::CallAi`], invoke the model and
    /// pass the outcome to [`Self::on_ai_complete`]. For input, tool, and retry variants, call
    /// the matching `on_*` / [`Self::retry_node`] handler before polling again.
    #[allow(
        clippy::too_many_lines,
        reason = "poll dispatches the full engine state machine in one place"
    )]
    pub fn poll(&mut self) -> EnginePollResult {
        if let Some(error) = self.terminal_error.clone() {
            return EnginePollResult::Failed(error);
        }
        if let Some((node_id, message)) = self.failed_nodes.iter().next() {
            return EnginePollResult::Failed(RunError::NodeFailed {
                node_id: node_id.clone(),
                kind: NodeFailureKind::Agent(message.clone()),
            });
        }

        if let Some((approval_id, pending)) = self.pending_tool_batches.iter().next() {
            let Some(node) = self.find_node(&pending.node_id) else {
                let node_id = pending.node_id.clone();
                return self.fail_internal(&node_id, NodeFailureKind::PendingToolNodeNotFound);
            };
            if pending.requires_approval {
                return EnginePollResult::AwaitToolApproval {
                    approval_id: approval_id.clone(),
                    node_id: node.id.clone(),
                    label: node.label.clone(),
                    tool_calls: pending.tool_calls.clone(),
                };
            }
            return EnginePollResult::RunTools {
                node_id: node.id.clone(),
                label: node.label.clone(),
                tool_calls: pending.tool_calls.clone(),
            };
        }

        self.schedule_manual_nodes_in_layer();
        if let Some(awaiting_id) = self.awaiting_nodes.iter().next() {
            let Some(node) = self.find_node(awaiting_id) else {
                let awaiting_id = awaiting_id.clone();
                return self.fail_internal(&awaiting_id, NodeFailureKind::AwaitingNodeNotFound);
            };
            return EnginePollResult::AwaitInput {
                node_id: node.id.clone(),
                label: node.label.clone(),
                context: self.assemble_context(&node.id),
                is_initial: self.conversation_history(&node.id).is_empty(),
            };
        }

        if let Some(report) = self.try_advance_layer() {
            return EnginePollResult::Completed(report);
        }

        for node_id in self.current_layer_nodes().to_vec() {
            if self.is_node_blocked(&node_id) {
                continue;
            }
            let Some(node) = self.find_node(&node_id) else {
                return self.fail_internal(&node_id, NodeFailureKind::NodeIdFromLayersNotFound);
            };
            if let Some(missing) = self.missing_upstream_outputs(&node_id) {
                return self
                    .fail_internal(&node_id, NodeFailureKind::MissingUpstreamOutput(missing));
            }
            if node.agent.auto_start || !self.transcript(&node_id).is_empty() {
                if self.queued_nodes.insert(node_id.clone()) {
                    self.events.push(RunEvent {
                        node_id: node_id.clone(),
                        kind: RunEventKind::Queued,
                        message: "queued".to_string(),
                        output: None,
                    });
                }
                self.emit_started_for_current_attempt(&node_id);
                let request = match self.build_request(&node_id) {
                    Ok(r) => r,
                    Err(e) => return EnginePollResult::Failed(e),
                };
                self.in_flight_ai.insert(node_id.clone());
                return EnginePollResult::CallAi {
                    node_id: node_id.clone(),
                    request: Box::new(request),
                };
            }
        }

        if self.recover_stale_in_flight_nodes() {
            return self.poll();
        }
        if !self.in_flight_ai.is_empty() {
            return EnginePollResult::Failed(RunError::NodeFailed {
                node_id: self
                    .in_flight_ai
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| NodeId("engine".to_string())),
                kind: NodeFailureKind::LayerStalledInFlight,
            });
        }

        if let Some(retryable) = self.layer_retryables().into_iter().next() {
            return EnginePollResult::AwaitRetry(retryable);
        }

        EnginePollResult::Failed(RunError::NodeFailed {
            node_id: self
                .current_layer_nodes()
                .first()
                .cloned()
                .unwrap_or_else(|| NodeId("engine".to_string())),
            kind: NodeFailureKind::NoRunnableNodesInLayer,
        })
    }

    /// Drive the engine until it needs host interaction or reaches a terminal state.
    pub async fn run<A: AiPort, T: ToolPort>(
        &mut self,
        ai: &A,
        tools: &T,
        cancel: &CancellationToken,
    ) -> EngineRunResult {
        loop {
            if cancel.is_cancelled() {
                return EngineRunResult::Cancelled;
            }

            if let Some(report) = self.try_advance_layer() {
                return EngineRunResult::Completed(report);
            }

            while let Some((node_id, label, tool_calls)) = self.next_run_tools_action() {
                let results = tools
                    .execute_batch(self, &node_id, &label, tool_calls)
                    .await;
                if cancel.is_cancelled() {
                    return EngineRunResult::Cancelled;
                }
                if self.interrupted_nodes.contains(&node_id) {
                    continue;
                }
                if let Err(error) = self.on_tool_results(&node_id, results) {
                    return EngineRunResult::Failed(RunError::NodeFailed {
                        node_id,
                        kind: NodeFailureKind::EngineInput(error.to_string()),
                    });
                }
            }

            let ai_actions = self.gather_call_ai_actions();
            if !ai_actions.is_empty() {
                let mut augmented = Vec::with_capacity(ai_actions.len());
                for (node_id, mut request) in ai_actions {
                    tools.augment_request(&node_id, &mut request);
                    augmented.push((node_id, request));
                }
                let outcomes = tokio::select! {
                    biased;
                    () = cancel.cancelled() => return EngineRunResult::Cancelled,
                    outcomes = join_all(augmented.into_iter().map(|(node_id, request)| async {
                        let outcome = ai.invoke(request).await;
                        (node_id, outcome)
                    })) => outcomes,
                };
                for (node_id, outcome) in outcomes {
                    self.on_ai_complete(&node_id, outcome);
                }
                if let Some(error) = self.terminal_error.clone() {
                    return EngineRunResult::Failed(error);
                }
                if let Some(delay) = self.pending_retry_delay.take() {
                    tokio::select! {
                        biased;
                        () = cancel.cancelled() => return EngineRunResult::Cancelled,
                        () = tokio::time::sleep(delay) => {}
                    }
                }
                continue;
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
    }

    fn next_run_tools_action(&self) -> Option<(NodeId, String, Vec<ToolCall>)> {
        let approval_id = self
            .pending_tool_batches
            .iter()
            .find(|(_, batch)| !batch.requires_approval)
            .map(|(approval_id, _)| approval_id.clone())?;
        let batch = self.pending_tool_batches.get(&approval_id)?;
        let node = self.find_node(&batch.node_id)?;
        Some((
            batch.node_id.clone(),
            node.label.clone(),
            batch.tool_calls.clone(),
        ))
    }

    fn gather_call_ai_actions(&mut self) -> Vec<(NodeId, AgentRequest)> {
        let mut actions = Vec::new();
        for node_id in self.current_layer_nodes().to_vec() {
            if self.is_node_blocked(&node_id) {
                continue;
            }
            let Some(node) = self.find_node(&node_id) else {
                continue;
            };
            if let Some(missing) = self.missing_upstream_outputs(&node_id) {
                self.terminal_error = Some(RunError::NodeFailed {
                    node_id: node_id.clone(),
                    kind: NodeFailureKind::MissingUpstreamOutput(missing),
                });
                break;
            }
            if !node.agent.auto_start && self.transcript(&node_id).is_empty() {
                continue;
            }
            if self.queued_nodes.insert(node_id.clone()) {
                self.events.push(RunEvent {
                    node_id: node_id.clone(),
                    kind: RunEventKind::Queued,
                    message: "queued".to_string(),
                    output: None,
                });
            }
            self.emit_started_for_current_attempt(&node_id);
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
        self.pending_tool_batches
            .retain(|_, batch| batch.node_id != *node_id);
        self.interrupted_nodes.insert(node_id.clone());
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Failed,
            message: "interrupted by user".to_string(),
            output: None,
        });
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
        let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
        *retry_count += 1;
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
            entrypoint_text: self.entrypoint_text.as_deref(),
            transcript: self.transcript(&node.id),
            available_tools: &[],
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

    fn fail_internal(&mut self, node_id: &NodeId, kind: NodeFailureKind) -> EnginePollResult {
        let error = RunError::NodeFailed {
            node_id: node_id.clone(),
            kind,
        };
        self.terminal_error = Some(error.clone());
        EnginePollResult::Failed(error)
    }

    fn emit_started_for_current_attempt(&mut self, node_id: &NodeId) {
        let attempt = self.retries_by_node.get(node_id).copied().unwrap_or(0) + 1;
        let emitted = self
            .started_invocations_by_node
            .entry(node_id.clone())
            .or_default();
        if *emitted >= attempt {
            return;
        }
        *emitted = attempt;
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Started,
            message: "invoking model".to_string(),
            output: None,
        });
    }

    fn reject_misrouted_completion(&mut self, node_id: &NodeId, message: String) {
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Failed,
            message: message.clone(),
            output: None,
        });
        self.terminal_error = Some(RunError::NodeFailed {
            node_id: node_id.clone(),
            kind: NodeFailureKind::MisroutedCompletion(message),
        });
    }
}

impl crate::ports::inbound::HumanInputPort for InteractiveEngine {
    fn submit_human_input(
        &mut self,
        input: crate::ports::inbound::HumanInput,
    ) -> Result<(), EngineInputError> {
        self.on_human_input(&input.node_id, &input.text)
    }
}

impl crate::ports::inbound::ToolApprovalPort for InteractiveEngine {
    fn submit_tool_approval(
        &mut self,
        input: crate::ports::inbound::ToolApprovalInput,
    ) -> Result<(), EngineInputError> {
        self.on_tool_decision(&input.approval_id, input.allow, input.reason.as_deref())
    }
}
