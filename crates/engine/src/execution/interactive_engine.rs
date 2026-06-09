use crate::conversation::{
    filter_tool_turn_assistant_message, AgentTranscriptItem, ChatMessage, ChatRole,
};
use crate::execution::node_invocation::{
    build_agent_request, build_upstream_map, NodeInvocationContext,
};
use crate::execution::{NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport};
use crate::graph::validation::{execution_layers, WorkflowValidationError};
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ToolPort,
};
use crate::tools::{
    override_policy_for_call, requires_approval, tool_tier_for_call, ApprovalMode,
    FileChangeRecord, ToolCall, ToolDecision, ToolResult,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

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
    /// Workflow finished successfully.
    Completed(RunReport),
    /// Workflow stopped with a terminal error; further polls return the same failure.
    Failed(RunError),
}

/// Terminal or pause outcome from [`InteractiveEngine::run`].
#[derive(Debug, Clone)]
pub enum EngineRunResult {
    NeedsInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    NeedsApproval {
        approval_id: String,
        node_id: NodeId,
        label: String,
        tool_calls: Vec<ToolCall>,
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
}

#[derive(Debug, Clone)]
struct PendingToolBatch {
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
    node_idx: usize,
    outputs: BTreeMap<NodeId, Value>,
    changed_files_by_node: BTreeMap<NodeId, Vec<FileChangeRecord>>,
    transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    events: Vec<RunEvent>,
    queued_nodes: BTreeSet<NodeId>,
    started_invocations_by_node: BTreeMap<NodeId, u8>,
    awaiting_node: Option<NodeId>,
    pending_tool_batch: Option<PendingToolBatch>,
    tool_rounds_by_node: BTreeMap<NodeId, u8>,
    retries_by_node: BTreeMap<NodeId, u8>,
    submit_output_retries_by_node: BTreeMap<NodeId, u8>,
    entrypoint_text: Option<String>,
    terminal_error: Option<RunError>,
}

const MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES: u8 = 3;

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
            node_idx: 0,
            outputs: BTreeMap::new(),
            changed_files_by_node: BTreeMap::new(),
            transcripts: BTreeMap::new(),
            events: Vec::new(),
            queued_nodes: BTreeSet::new(),
            started_invocations_by_node: BTreeMap::new(),
            awaiting_node: None,
            pending_tool_batch: None,
            tool_rounds_by_node: BTreeMap::new(),
            retries_by_node: BTreeMap::new(),
            submit_output_retries_by_node: BTreeMap::new(),
            entrypoint_text,
            terminal_error: None,
        })
    }

    fn advance(&mut self) {
        self.node_idx += 1;
        if self.node_idx >= self.layers.get(self.layer_idx).map_or(0, Vec::len) {
            self.node_idx = 0;
            self.layer_idx += 1;
        }
    }

    fn current_node_id(&self) -> Option<NodeId> {
        self.layers
            .get(self.layer_idx)
            .and_then(|layer| layer.get(self.node_idx))
            .cloned()
    }

    fn find_node(&self, node_id: &str) -> Option<&Node> {
        self.node_index
            .get(node_id)
            .and_then(|index| self.workflow.nodes.get(*index))
    }

    /// Advance the engine one step and return the next action for the host runtime.
    ///
    /// Call repeatedly until the result is [`EnginePollResult::Completed`] or
    /// [`EnginePollResult::Failed`]. For [`EnginePollResult::CallAi`], invoke the model and
    /// pass the outcome to [`Self::on_ai_complete`]. For input and tool variants, call the
    /// matching `on_*` handler before polling again.
    pub fn poll(&mut self) -> EnginePollResult {
        if let Some(error) = self.terminal_error.clone() {
            return EnginePollResult::Failed(error);
        }

        if let Some(awaiting_id) = &self.awaiting_node {
            let Some(node) = self.find_node(awaiting_id) else {
                let awaiting_id = awaiting_id.clone();
                return self.fail_internal(&awaiting_id, "awaiting node no longer exists");
            };
            return EnginePollResult::AwaitInput {
                node_id: node.id.clone(),
                label: node.label.clone(),
                context: self.assemble_context(&node.id),
                is_initial: self.conversation_history(&node.id).is_empty(),
            };
        }

        if let Some(pending) = &self.pending_tool_batch {
            let Some(node) = self.find_node(&pending.node_id) else {
                let node_id = pending.node_id.clone();
                return self.fail_internal(&node_id, "pending tool node no longer exists");
            };
            if pending.requires_approval {
                return EnginePollResult::AwaitToolApproval {
                    approval_id: pending.approval_id.clone(),
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

        let Some(node_id) = self.current_node_id() else {
            return EnginePollResult::Completed(RunReport {
                workflow_id: self.workflow.id.clone(),
                events: std::mem::take(&mut self.events),
                outputs: std::mem::take(&mut self.outputs)
                    .into_iter()
                    .map(|(node_id, output)| NodeRunOutput { node_id, output })
                    .collect(),
            });
        };

        let Some(node) = self.find_node(&node_id) else {
            return self.fail_internal(&node_id, "node id from layers not found in workflow");
        };
        let node_id = node.id.clone();
        let node_label = node.label.clone();

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
            return EnginePollResult::CallAi {
                node_id: node_id.clone(),
                request: Box::new(match self.build_request(&node_id) {
                    Ok(r) => r,
                    Err(e) => return EnginePollResult::Failed(e),
                }),
            };
        }

        if self.queued_nodes.insert(node_id.clone()) {
            self.events.push(RunEvent {
                node_id: node_id.clone(),
                kind: RunEventKind::Queued,
                message: "queued".to_string(),
                output: None,
            });
        }
        self.awaiting_node = Some(node_id.clone());
        self.transcripts.entry(node_id.clone()).or_default();
        EnginePollResult::AwaitInput {
            node_id: node_id.clone(),
            label: node_label,
            context: self.assemble_context(&node_id),
            is_initial: true,
        }
    }

    /// Drive the engine until it needs host interaction or reaches a terminal state.
    pub async fn run<A: AiPort, T: ToolPort>(
        &mut self,
        ai: &A,
        tools: &T,
        cancel: &CancellationToken,
    ) -> EngineRunResult {
        loop {
            match self.poll() {
                EnginePollResult::CallAi {
                    node_id,
                    mut request,
                } => {
                    tools.augment_request(&node_id, &mut request);
                    let outcome = tokio::select! {
                        result = ai.invoke(*request) => result,
                        _ = cancel.cancelled() => return EngineRunResult::Cancelled,
                    };
                    self.on_ai_complete(&node_id, outcome);
                }
                EnginePollResult::RunTools {
                    node_id,
                    label,
                    tool_calls,
                } => {
                    let results = tools
                        .execute_batch(self, &node_id, &label, tool_calls)
                        .await;
                    if cancel.is_cancelled() {
                        return EngineRunResult::Cancelled;
                    }
                    if let Err(error) = self.on_tool_results(&node_id, results) {
                        return EngineRunResult::Failed(RunError::NodeFailed {
                            node_id,
                            message: error.to_string(),
                        });
                    }
                }
                EnginePollResult::AwaitInput {
                    node_id,
                    label,
                    context,
                    is_initial,
                } => {
                    return EngineRunResult::NeedsInput {
                        node_id,
                        label,
                        context,
                        is_initial,
                    };
                }
                EnginePollResult::AwaitToolApproval {
                    approval_id,
                    node_id,
                    label,
                    tool_calls,
                } => {
                    return EngineRunResult::NeedsApproval {
                        approval_id,
                        node_id,
                        label,
                        tool_calls,
                    };
                }
                EnginePollResult::Completed(report) => return EngineRunResult::Completed(report),
                EnginePollResult::Failed(error) => return EngineRunResult::Failed(error),
            }
        }
    }

    /// Apply a model turn outcome for the node currently awaiting completion.
    ///
    /// Misrouted completions (wrong `node_id` or no active node) set a terminal workflow error
    /// instead of panicking.
    pub fn on_ai_complete(&mut self, node_id: &str, result: Result<AgentTurnOutcome, AgentError>) {
        let Some(expected) = self.current_node_id() else {
            self.reject_misrouted_completion(node_id, "no node is awaiting model completion");
            return;
        };
        if expected != node_id {
            self.reject_misrouted_completion(
                node_id,
                &format!("expected model completion for {expected}, got {node_id}"),
            );
            return;
        }

        match result {
            Ok(AgentTurnOutcome::Completed(success)) => {
                self.apply_completion(node_id, success);
            }
            Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                self.apply_tool_calls(node_id, batch);
            }
            Ok(AgentTurnOutcome::NeedsUserInput(input)) => {
                self.apply_user_input_request(node_id, input);
            }
            Err(error) => {
                let node_id = NodeId(node_id.to_string());
                if error.is_malformed_submit_output() {
                    let retry_count = self
                        .submit_output_retries_by_node
                        .entry(node_id.clone())
                        .or_default();
                    if *retry_count < MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES {
                        *retry_count += 1;
                        self.transcripts
                            .entry(node_id.clone())
                            .or_default()
                            .push(AgentTranscriptItem::UserMessage {
                                content: format!(
                                    "Your openflow_submit_node_output call was invalid ({error}). \
                                     Call openflow_submit_node_output again with arguments shaped as \
                                     {{\"output\": <object matching the node output schema>, \"assistant_message\": null}}. \
                                     Put schema fields under \"output\", not at the top level."
                                ),
                            });
                        self.events.push(RunEvent {
                            node_id: node_id.clone(),
                            kind: RunEventKind::Retrying,
                            message: format!(
                                "retrying after malformed submit-output tool call ({}/{MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES})",
                                *retry_count
                            ),
                            output: None,
                        });
                        return;
                    }
                }
                if error.is_retryable() {
                    let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
                    if *retry_count < self.workflow.settings.retry_policy.max_attempts {
                        *retry_count += 1;
                        self.events.push(RunEvent {
                            node_id,
                            kind: RunEventKind::Retrying,
                            message: format!(
                                "retrying after transient failure; backoff_ms={}",
                                self.workflow.settings.retry_policy.backoff_ms
                            ),
                            output: None,
                        });
                        return;
                    }
                }
                let run_error = RunError::NodeFailed {
                    node_id: node_id.clone(),
                    message: error.to_string(),
                };
                self.events.push(RunEvent {
                    node_id,
                    kind: RunEventKind::Failed,
                    message: error.to_string(),
                    output: None,
                });
                self.terminal_error = Some(run_error);
            }
        }
    }

    fn apply_completion(&mut self, node_id: &str, success: AgentTurnSuccess) {
        if let Some(message) = filter_tool_turn_assistant_message(success.assistant_message)
            .filter(|message| !message.trim().is_empty())
        {
            self.transcripts
                .entry(NodeId(node_id.to_string()))
                .or_default()
                .push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        self.outputs
            .insert(NodeId(node_id.to_string()), success.output.clone());
        self.events.push(RunEvent {
            node_id: NodeId(node_id.to_string()),
            kind: RunEventKind::Completed,
            message: "completed".to_string(),
            output: Some(success.output),
        });
        self.advance();
    }

    fn apply_tool_calls(&mut self, node_id: &str, batch: AgentToolCallBatch) {
        let max_tool_rounds = if let Some(node) = self.find_node(node_id) {
            node.agent.tools.max_tool_rounds
        } else {
            self.terminal_error = Some(RunError::NodeFailed {
                node_id: NodeId(node_id.to_string()),
                message: "tool-call node no longer exists".to_string(),
            });
            return;
        };
        let round_count = self
            .tool_rounds_by_node
            .entry(NodeId(node_id.to_string()))
            .or_default();
        if *round_count >= max_tool_rounds {
            let message = format!("node exceeded max tool rounds ({max_tool_rounds})");
            self.events.push(RunEvent {
                node_id: NodeId(node_id.to_string()),
                kind: RunEventKind::Failed,
                message,
                output: None,
            });
            self.advance();
            return;
        }
        *round_count += 1;

        let config = self
            .find_node(node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        let approval_mode = config.approval_mode.unwrap_or(ApprovalMode::Write);
        let transcript = self
            .transcripts
            .entry(NodeId(node_id.to_string()))
            .or_default();
        if let Some(message) = filter_tool_turn_assistant_message(batch.assistant_message)
            .filter(|message| !message.trim().is_empty())
        {
            transcript.push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        let mut pending_calls = Vec::new();
        let mut requires_approval_for_batch = false;
        for call in batch.tool_calls {
            transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
            let tier = tool_tier_for_call(&config, &call.name);
            let override_policy = override_policy_for_call(&config, &call.name);
            match requires_approval(approval_mode, tier, override_policy) {
                ToolDecision::AutoAllow => pending_calls.push(call),
                ToolDecision::Prompt => {
                    requires_approval_for_batch = true;
                    pending_calls.push(call);
                }
                ToolDecision::Deny => {
                    transcript.push(AgentTranscriptItem::ToolResult {
                        result: denied_tool_result(&call, "denied by policy"),
                    });
                }
            }
        }
        if pending_calls.is_empty() {
            return;
        }
        self.pending_tool_batch = Some(PendingToolBatch {
            approval_id: Uuid::new_v4().to_string(),
            node_id: NodeId(node_id.to_string()),
            tool_calls: pending_calls,
            requires_approval: requires_approval_for_batch,
        });
    }

    fn apply_user_input_request(&mut self, node_id: &str, input: AgentNeedUserInput) {
        self.transcripts
            .entry(NodeId(node_id.to_string()))
            .or_default()
            .push(AgentTranscriptItem::AssistantMessage {
                content: input.assistant_message,
            });
        self.awaiting_node = Some(NodeId(node_id.to_string()));
    }

    /// # Errors
    /// Returns an error if no node is awaiting input or the wrong node id is provided.
    pub fn on_human_input(&mut self, node_id: &str, text: &str) -> Result<(), EngineInputError> {
        let expected = self
            .awaiting_node
            .as_ref()
            .ok_or(EngineInputError::NoNodeAwaiting)?;
        if expected != node_id {
            return Err(EngineInputError::WrongNode {
                expected: expected.clone(),
                got: NodeId(node_id.to_string()),
            });
        }
        self.awaiting_node = None;
        self.transcripts
            .entry(NodeId(node_id.to_string()))
            .or_default()
            .push(AgentTranscriptItem::UserMessage {
                content: text.to_string(),
            });
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

    /// # Errors
    /// Returns an error if no tool calls are pending or the wrong node id is provided.
    pub fn on_tool_results(
        &mut self,
        node_id: &str,
        results: Vec<ToolResult>,
    ) -> Result<(), EngineInputError> {
        let pending = self
            .pending_tool_batch
            .as_ref()
            .ok_or(EngineInputError::NoPendingTools)?;
        if pending.requires_approval {
            return Err(EngineInputError::NoPendingTools);
        }
        if pending.node_id != node_id {
            return Err(EngineInputError::WrongNode {
                expected: pending.node_id.clone(),
                got: NodeId(node_id.to_string()),
            });
        }
        let transcript = self
            .transcripts
            .entry(NodeId(node_id.to_string()))
            .or_default();
        for result in results {
            transcript.push(AgentTranscriptItem::ToolResult { result });
        }
        self.pending_tool_batch = None;
        Ok(())
    }

    /// # Errors
    /// Returns an error if no matching approval batch is awaiting a decision.
    pub fn on_tool_decision(
        &mut self,
        approval_id: &str,
        allow: bool,
    ) -> Result<(), EngineInputError> {
        let pending = self
            .pending_tool_batch
            .as_mut()
            .ok_or(EngineInputError::NoPendingTools)?;
        if !pending.requires_approval {
            return Err(EngineInputError::NoPendingTools);
        }
        if pending.approval_id != approval_id {
            return Err(EngineInputError::UnknownApproval(approval_id.to_string()));
        }
        if allow {
            pending.requires_approval = false;
            return Ok(());
        }

        let node_id = pending.node_id.clone();
        let denied = pending
            .tool_calls
            .iter()
            .map(|call| AgentTranscriptItem::ToolResult {
                result: denied_tool_result(call, "denied by user"),
            })
            .collect::<Vec<_>>();
        self.transcripts.entry(node_id).or_default().extend(denied);
        self.pending_tool_batch = None;
        Ok(())
    }

    #[must_use]
    pub fn node_output(&self, node_id: &str) -> Option<Value> {
        self.outputs.get(node_id).cloned()
    }

    #[must_use]
    pub fn conversation_history(&self, node_id: &str) -> Vec<ChatMessage> {
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
    pub fn transcript(&self, node_id: &str) -> &[AgentTranscriptItem] {
        self.transcripts.get(node_id).map_or(&[], Vec::as_slice)
    }

    fn build_request(&self, node_id: &str) -> Result<AgentRequest, RunError> {
        let node = self
            .find_node(node_id)
            .ok_or_else(|| RunError::NodeFailed {
                node_id: NodeId(node_id.to_string()),
                message: "node must exist".to_string(),
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
        build_agent_request(&ctx, node, true)
    }

    fn assemble_context(&self, node_id: &str) -> String {
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

    fn fail_internal(&mut self, node_id: &NodeId, message: &str) -> EnginePollResult {
        let error = RunError::NodeFailed {
            node_id: node_id.clone(),
            message: message.to_string(),
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

    fn reject_misrouted_completion(&mut self, node_id: &str, message: &str) {
        let node_id = NodeId(node_id.to_string());
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Failed,
            message: message.to_string(),
            output: None,
        });
        self.terminal_error = Some(RunError::NodeFailed {
            node_id,
            message: message.to_string(),
        });
    }
}

fn denied_tool_result(call: &ToolCall, content: &str) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: content.to_string(),
        is_error: true,
        artifact_ids: Vec::new(),
        output_meta: None,
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
        self.on_tool_decision(&input.approval_id, input.allow)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::graph::Node;
    use serde_json::json;

    fn node(id: &str) -> Node {
        let mut node = Node::agent(id, 0.0, 0.0);
        node.id = NodeId(id.to_string());
        node.agent.model = "test-model".to_string();
        node
    }

    #[test]
    fn revert_file_changes_for_batch_removes_only_matching_records() {
        let mut workflow = Workflow::new("revert");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();
        let node_id = NodeId("idea".to_string());
        engine.record_file_changes(
            &node_id,
            vec![
                FileChangeRecord {
                    path: "a.txt".to_string(),
                    op: crate::tools::FileChangeOp::Update,
                    rename_to: None,
                    diff_summary: None,
                    batch_id: Some("batch-1".to_string()),
                    timestamp_ms: 1,
                },
                FileChangeRecord {
                    path: "a.txt".to_string(),
                    op: crate::tools::FileChangeOp::Update,
                    rename_to: None,
                    diff_summary: None,
                    batch_id: Some("batch-2".to_string()),
                    timestamp_ms: 2,
                },
            ],
        );

        engine.revert_file_changes_for_batch("batch-1", &node_id);

        let records = engine.changed_files_by_node.get(&node_id).expect("records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].batch_id.as_deref(), Some("batch-2"));
    }

    #[test]
    fn shared_context_is_appended_to_system_prompt() {
        let mut workflow = Workflow::new("shared");
        workflow.settings.shared_context = "Always follow the style guide.".to_string();
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        let EnginePollResult::CallAi { request, .. } = engine.poll() else {
            panic!("expected CallAi");
        };
        assert!(request.system_prompt.contains("--- Workflow context ---"));
        assert!(request
            .system_prompt
            .contains("Always follow the style guide."));
    }

    #[test]
    fn auto_start_node_runs_ai_and_completes() {
        let mut workflow = Workflow::new("test");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        let result = engine.poll();
        assert!(matches!(
            result,
            EnginePollResult::CallAi { ref node_id, .. } if node_id == "idea"
        ));

        let EnginePollResult::CallAi { request, .. } = result else {
            panic!("expected CallAi");
        };
        assert_eq!(request.node_id, "idea");
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "...".to_string(),
                assistant_message: None,
            })),
        );

        let final_result = engine.poll();
        assert!(matches!(final_result, EnginePollResult::Completed(_)));
    }

    #[test]
    fn non_auto_start_node_pauses_awaiting_input() {
        let mut workflow = Workflow::new("test");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];

        let mut engine = InteractiveEngine::new(workflow, None).unwrap();
        let result = engine.poll();
        assert!(matches!(
            result,
            EnginePollResult::AwaitInput { ref node_id, is_initial: true, .. } if node_id == "idea"
        ));
    }

    #[test]
    fn awaiting_manual_node_repeats_context_until_input_arrives() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        idea.agent.task_prompt = "Choose the product direction".to_string();
        workflow.nodes = vec![idea];
        let mut engine =
            InteractiveEngine::new(workflow, Some("Launch planning kickoff".to_string())).unwrap();

        let first = engine.poll();
        let second = engine.poll();

        match (first, second) {
            (
                EnginePollResult::AwaitInput {
                    node_id: first_id,
                    context: first_context,
                    ..
                },
                EnginePollResult::AwaitInput {
                    node_id: second_id,
                    context: second_context,
                    ..
                },
            ) => {
                assert_eq!(first_id, "idea");
                assert_eq!(second_id, "idea");
                assert_eq!(first_context, second_context);
                assert!(first_context.contains("Entrypoint: Launch planning kickoff"));
                assert!(first_context.contains("Task: Choose the product direction"));
            }
            _ => panic!("expected repeated AwaitInput results"),
        }
    }

    #[test]
    fn wrong_node_human_input_is_rejected_without_advancing() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();
        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));

        let error = engine.on_human_input("other", "Wrong node").unwrap_err();
        let result = engine.poll();

        assert_eq!(
            error,
            EngineInputError::WrongNode {
                expected: NodeId("idea".to_string()),
                got: NodeId("other".to_string())
            }
        );
        assert!(matches!(
            result,
            EnginePollResult::AwaitInput { ref node_id, .. } if node_id == "idea"
        ));
        assert!(engine.node_output("idea").is_none());
    }

    #[test]
    fn manual_node_user_input_starts_ai_request() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
        engine
            .on_human_input("idea", "Need a smaller launch scope")
            .unwrap();

        let result = engine.poll();
        let EnginePollResult::CallAi { request, .. } = result else {
            panic!("expected ai request");
        };
        assert_eq!(request.node_id, "idea");
        assert_eq!(
            request.transcript,
            vec![AgentTranscriptItem::UserMessage {
                content: "Need a smaller launch scope".to_string(),
            }]
        );
    }

    #[test]
    fn conversation_follow_up_repauses_same_node() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
        engine
            .on_human_input("idea", "Need a smaller launch scope")
            .unwrap();
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: "...".to_string(),
                assistant_message: "Which approval step is mandatory?".to_string(),
            })),
        );

        let result = engine.poll();
        assert!(matches!(
            result,
            EnginePollResult::AwaitInput { ref node_id, is_initial: false, .. } if node_id == "idea"
        ));
        assert_eq!(
            engine.conversation_history("idea"),
            vec![
                ChatMessage::text(ChatRole::User, "Need a smaller launch scope"),
                ChatMessage::text(ChatRole::Assistant, "Which approval step is mandatory?"),
            ]
        );
    }

    #[test]
    fn tool_calls_pause_for_approval_and_resume_after_results() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.catalog.tools = vec![crate::ToolRef {
            name: "read".to_string(),
            tier: Some(crate::ToolTier::Read),
        }];
        idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "...".to_string(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: Some("Reading repo overview".to_string()),
                }],
            })),
        );

        let pending = engine.poll();
        let EnginePollResult::AwaitToolApproval {
            ref approval_id,
            ref node_id,
            ..
        } = pending
        else {
            panic!("expected approval");
        };
        assert_eq!(node_id, "idea");
        let approval_id = approval_id.clone();

        engine.on_tool_decision(&approval_id, true).unwrap();
        let runnable = engine.poll();
        assert!(matches!(
            runnable,
            EnginePollResult::RunTools { ref node_id, .. } if node_id == "idea"
        ));

        engine
            .on_tool_results(
                "idea",
                vec![ToolResult {
                    tool_call_id: "call-1".to_string(),
                    tool_name: "read".to_string(),
                    content: "# README".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                }],
            )
            .unwrap();

        let resumed = engine.poll();
        let EnginePollResult::CallAi { request, .. } = resumed else {
            panic!("expected resumed ai request");
        };
        assert!(matches!(
            request.transcript.as_slice(),
            [
                AgentTranscriptItem::ToolCall { .. },
                AgentTranscriptItem::ToolResult { .. }
            ]
        ));
    }

    #[test]
    fn conversation_completion_sets_output_and_advances() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        let final_node = node("final");
        workflow.nodes = vec![idea, final_node];
        workflow.edges = vec![crate::Edge::new("idea", "final")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
        engine
            .on_human_input("idea", "Workflow execution with approvals")
            .unwrap();
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                raw_text: "...".to_string(),
                assistant_message: Some("Locked. Advancing.".to_string()),
                output: json!({"summary": "Workflow execution with approvals"}),
            })),
        );

        assert_eq!(
            engine.node_output("idea"),
            Some(json!({"summary": "Workflow execution with approvals"}))
        );
        let next = engine.poll();
        assert!(matches!(
            next,
            EnginePollResult::CallAi { ref node_id, .. } if node_id == "final"
        ));
    }

    #[test]
    fn poll_targets_first_manual_node_in_layer_order() {
        let mut workflow = Workflow::new("indexed");
        let mut first = node("first");
        first.agent.auto_start = false;
        let mut second = node("second");
        second.agent.auto_start = false;
        workflow.nodes = vec![first, second];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        match engine.poll() {
            EnginePollResult::AwaitInput { node_id, .. } => assert_eq!(node_id, "first"),
            other => panic!("expected AwaitInput, got {other:?}"),
        }
    }

    #[test]
    fn yolo_mode_skips_tool_approval() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "...".to_string(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })),
        );

        let pending = engine.poll();
        assert!(matches!(
            pending,
            EnginePollResult::RunTools { ref node_id, .. } if node_id == "idea"
        ));
    }

    #[test]
    fn denied_tool_call_resumes_with_error_result() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "...".to_string(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })),
        );
        let EnginePollResult::AwaitToolApproval { approval_id, .. } = engine.poll() else {
            panic!("expected approval");
        };

        engine.on_tool_decision(&approval_id, false).unwrap();

        let EnginePollResult::CallAi { request, .. } = engine.poll() else {
            panic!("expected resumed AI request");
        };
        assert!(matches!(
            request.transcript.last(),
            Some(AgentTranscriptItem::ToolResult { result })
                if result.is_error && result.content == "denied by user"
        ));
    }

    #[test]
    fn transient_failure_retries_then_succeeds() {
        let mut workflow = Workflow::new("retry");
        workflow.settings.retry_policy.max_attempts = 1;
        workflow.settings.retry_policy.backoff_ms = 25;
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete("idea", Err(AgentError::Transient("timeout".to_string())));

        let retry = engine.poll();
        assert!(matches!(
            retry,
            EnginePollResult::CallAi { ref node_id, .. } if node_id == "idea"
        ));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            })),
        );

        let EnginePollResult::Completed(report) = engine.poll() else {
            panic!("expected completed report");
        };
        assert!(report
            .events
            .iter()
            .any(|event| event.kind == RunEventKind::Retrying
                && event.message.contains("backoff_ms=25")));
        assert_eq!(report.outputs.len(), 1);
    }

    #[test]
    fn permanent_failure_does_not_retry() {
        let mut workflow = Workflow::new("retry");
        workflow.settings.retry_policy.max_attempts = 3;
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete("idea", Err(AgentError::Permanent("schema".to_string())));

        assert!(matches!(engine.poll(), EnginePollResult::Failed(_)));
    }

    #[test]
    fn malformed_submit_output_retries_then_succeeds() {
        let mut workflow = Workflow::new("submit-retry");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Err(AgentError::Failed(
                "OpenAI-compatible final output tool arguments were not valid JSON: missing field `output`"
                    .to_string(),
            )),
        );

        let EnginePollResult::CallAi { request, .. } = engine.poll() else {
            panic!("expected retry AI call");
        };
        assert!(matches!(
            request.transcript.last(),
            Some(AgentTranscriptItem::UserMessage { content })
                if content.contains("openflow_submit_node_output")
        ));

        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            })),
        );

        let EnginePollResult::Completed(report) = engine.poll() else {
            panic!("expected completed report");
        };
        assert!(report.events.iter().any(|event| {
            event.kind == RunEventKind::Retrying
                && event.message.contains("malformed submit-output")
        }));
    }

    #[test]
    fn tool_config_names_survive_into_request() {
        let mut workflow = Workflow::new("tools");
        let mut idea = node("idea");
        idea.agent.tools.catalog.tools = vec![crate::ToolRef {
            name: "search".to_string(),
            tier: Some(crate::ToolTier::Read),
        }];
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        let EnginePollResult::CallAi { request, .. } = engine.poll() else {
            panic!("expected AI request");
        };

        assert!(request.available_tools.is_empty());
        assert_eq!(request.tool_config.catalog.tools[0].name, "search");
    }

    #[test]
    fn tool_call_xml_echo_is_dropped_from_transcript() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "...".to_string(),
                assistant_message: Some(
                    "<tool_call><function=read></function></tool_call>".to_string(),
                ),
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })),
        );

        assert!(!engine.transcript("idea").iter().any(|item| matches!(
            item,
            AgentTranscriptItem::AssistantMessage { content }
                if content.contains("<tool_call>")
        )));
    }

    #[test]
    fn completion_tool_call_xml_echo_is_dropped_from_transcript() {
        let mut workflow = Workflow::new("completion");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: Some(
                    "<tool_call><function=read></function></tool_call>".to_string(),
                ),
            })),
        );

        assert!(engine.transcript("idea").is_empty());
    }

    #[test]
    fn misrouted_completion_is_rejected() {
        let mut workflow = Workflow::new("misroute");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "other",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "wrong"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            })),
        );

        let EnginePollResult::Failed(RunError::NodeFailed { node_id, message }) = engine.poll()
        else {
            panic!("expected failure");
        };
        assert_eq!(node_id, "other");
        assert_eq!(message, "expected model completion for idea, got other");
    }

    #[test]
    fn started_event_is_provider_neutral_and_emitted_once_per_poll_attempt() {
        let mut workflow = Workflow::new("events");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            })),
        );

        let EnginePollResult::Completed(report) = engine.poll() else {
            panic!("expected completion");
        };
        let started = report
            .events
            .iter()
            .filter(|event| event.kind == RunEventKind::Started)
            .collect::<Vec<_>>();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].message, "invoking model");
    }

    #[test]
    fn max_tool_rounds_failure_is_node_local() {
        let mut workflow = Workflow::new("tool rounds");
        let mut first = node("first");
        first.agent.tools.max_tool_rounds = 0;
        workflow.nodes = vec![first, node("second")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(
            engine.poll(),
            EnginePollResult::CallAi { ref node_id, .. } if node_id == "first"
        ));
        engine.on_ai_complete(
            "first",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "{}".to_string(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })),
        );

        assert!(matches!(
            engine.poll(),
            EnginePollResult::CallAi { ref node_id, .. } if node_id == "second"
        ));
    }

    #[test]
    fn inbound_ports_drive_engine_inputs() {
        use crate::ports::inbound::{
            HumanInput, HumanInputPort, ToolApprovalInput, ToolApprovalPort,
        };

        let mut workflow = Workflow::new("ports");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
        HumanInputPort::submit_human_input(
            &mut engine,
            HumanInput {
                node_id: NodeId("idea".to_string()),
                text: "Need context".to_string(),
            },
        )
        .unwrap();
        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
        engine.on_ai_complete(
            "idea",
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "{}".to_string(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })),
        );
        let EnginePollResult::AwaitToolApproval { approval_id, .. } = engine.poll() else {
            panic!("expected approval");
        };
        ToolApprovalPort::submit_tool_approval(
            &mut engine,
            ToolApprovalInput {
                approval_id,
                allow: false,
            },
        )
        .unwrap();

        assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    }
}
