use crate::{
    execution_layers, runner::build_node_input, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTranscriptItem, AgentTurnOutcome, AgentTurnSuccess, ChatMessage,
    ChatRole, NodeId, NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport, ToolCall,
    ToolResult, Workflow, WorkflowValidationError,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum EnginePollResult {
    CallAi {
        node_id: NodeId,
        request: Box<AgentRequest>,
    },
    AwaitInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    AwaitToolApproval {
        node_id: NodeId,
        label: String,
        tool_calls: Vec<ToolCall>,
    },
    Completed(RunReport),
    Failed(RunError),
}

#[derive(Debug, Clone)]
struct PendingToolBatch {
    node_id: NodeId,
    tool_calls: Vec<ToolCall>,
}

pub struct InteractiveEngine {
    workflow: Workflow,
    upstream_map: HashMap<NodeId, Vec<NodeId>>,
    layers: Vec<Vec<NodeId>>,
    layer_idx: usize,
    node_idx: usize,
    outputs: BTreeMap<NodeId, Value>,
    transcripts: BTreeMap<NodeId, Vec<AgentTranscriptItem>>,
    events: Vec<RunEvent>,
    awaiting_node: Option<NodeId>,
    pending_tool_batch: Option<PendingToolBatch>,
    tool_rounds_by_node: BTreeMap<NodeId, u8>,
    entrypoint_text: Option<String>,
    terminal_error: Option<RunError>,
}

impl InteractiveEngine {
    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn new(
        workflow: Workflow,
        entrypoint_text: Option<String>,
    ) -> Result<Self, WorkflowValidationError> {
        let layers = execution_layers(&workflow)?;
        let mut upstream_map: HashMap<NodeId, Vec<NodeId>> = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), Vec::new()))
            .collect();
        for edge in &workflow.edges {
            upstream_map
                .entry(edge.to.clone())
                .or_default()
                .push(edge.from.clone());
        }
        for ids in upstream_map.values_mut() {
            ids.sort();
        }
        Ok(Self {
            workflow,
            upstream_map,
            layers,
            layer_idx: 0,
            node_idx: 0,
            outputs: BTreeMap::new(),
            transcripts: BTreeMap::new(),
            events: Vec::new(),
            awaiting_node: None,
            pending_tool_batch: None,
            tool_rounds_by_node: BTreeMap::new(),
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

    fn find_node(&self, node_id: &str) -> Option<&crate::Node> {
        self.workflow.nodes.iter().find(|node| node.id == node_id)
    }

    pub fn poll(&mut self) -> EnginePollResult {
        if let Some(error) = self.terminal_error.clone() {
            return EnginePollResult::Failed(error);
        }

        if let Some(awaiting_id) = self.awaiting_node.clone() {
            let Some(node) = self.find_node(&awaiting_id) else {
                return self.fail_internal(&awaiting_id, "awaiting node no longer exists");
            };
            return EnginePollResult::AwaitInput {
                node_id: node.id.clone(),
                label: node.label.clone(),
                context: self.assemble_context(&node.id),
                is_initial: self.conversation_history(&node.id).is_empty(),
            };
        }

        if let Some(pending) = self.pending_tool_batch.clone() {
            let Some(node) = self.find_node(&pending.node_id) else {
                return self.fail_internal(&pending.node_id, "pending tool node no longer exists");
            };
            return EnginePollResult::AwaitToolApproval {
                node_id: node.id.clone(),
                label: node.label.clone(),
                tool_calls: pending.tool_calls,
            };
        }

        let Some(node_id) = self.current_node_id() else {
            return EnginePollResult::Completed(RunReport {
                workflow_id: self.workflow.id.clone(),
                events: self.events.clone(),
                outputs: self
                    .outputs
                    .iter()
                    .map(|(id, output)| NodeRunOutput {
                        node_id: id.clone(),
                        output: output.clone(),
                    })
                    .collect(),
            });
        };

        let Some(node) = self.find_node(&node_id) else {
            return self.fail_internal(&node_id, "node id from layers not found in workflow");
        };
        let node_id = node.id.clone();
        let node_label = node.label.clone();

        if node.agent.auto_start || !self.transcript(&node_id).is_empty() {
            if self.transcript(&node_id).is_empty() {
                self.events.push(RunEvent {
                    node_id: node_id.clone(),
                    kind: RunEventKind::Queued,
                    message: "queued".to_string(),
                    output: None,
                });
            }
            return EnginePollResult::CallAi {
                node_id: node_id.clone(),
                request: Box::new(match self.build_request(&node_id) {
                    Ok(r) => r,
                    Err(e) => return EnginePollResult::Failed(e),
                }),
            };
        }

        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Queued,
            message: "queued".to_string(),
            output: None,
        });
        self.awaiting_node = Some(node_id.clone());
        self.transcripts.entry(node_id.clone()).or_default();
        EnginePollResult::AwaitInput {
            node_id: node_id.clone(),
            label: node_label,
            context: self.assemble_context(&node_id),
            is_initial: true,
        }
    }

    pub fn on_ai_complete(&mut self, node_id: &str, result: Result<AgentTurnOutcome, AgentError>) {
        self.events.push(RunEvent {
            node_id: NodeId(node_id.to_string()),
            kind: RunEventKind::Started,
            message: "started OpenAI node call".to_string(),
            output: None,
        });

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
                let run_error = RunError::NodeFailed {
                    node_id: NodeId(node_id.to_string()),
                    message: error.to_string(),
                };
                self.events.push(RunEvent {
                    node_id: NodeId(node_id.to_string()),
                    kind: RunEventKind::Failed,
                    message: error.to_string(),
                    output: None,
                });
                self.terminal_error = Some(run_error);
            }
        }
    }

    fn apply_completion(&mut self, node_id: &str, success: AgentTurnSuccess) {
        if let Some(message) = success
            .assistant_message
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
                message: message.clone(),
                output: None,
            });
            self.terminal_error = Some(RunError::NodeFailed {
                node_id: NodeId(node_id.to_string()),
                message,
            });
            return;
        }
        *round_count += 1;

        let transcript = self
            .transcripts
            .entry(NodeId(node_id.to_string()))
            .or_default();
        if let Some(message) = batch
            .assistant_message
            .filter(|message| !message.trim().is_empty())
        {
            transcript.push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        for call in &batch.tool_calls {
            transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
        }
        self.pending_tool_batch = Some(PendingToolBatch {
            node_id: NodeId(node_id.to_string()),
            tool_calls: batch.tool_calls,
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
    pub fn on_human_input(&mut self, node_id: &str, text: &str) -> Result<(), String> {
        let expected = self
            .awaiting_node
            .as_ref()
            .ok_or("no node awaiting input")?;
        if expected != node_id {
            return Err(format!("expected input for {expected}, got {node_id}"));
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

    /// # Errors
    /// Returns an error if no tool calls are pending or the wrong node id is provided.
    pub fn on_tool_results(
        &mut self,
        node_id: &str,
        results: Vec<ToolResult>,
    ) -> Result<(), String> {
        let pending = self
            .pending_tool_batch
            .as_ref()
            .ok_or("no node awaiting tool results")?;
        if pending.node_id != node_id {
            return Err(format!(
                "expected tool results for {}, got {node_id}",
                pending.node_id
            ));
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

    #[must_use]
    pub fn node_output(&self, node_id: &str) -> Option<Value> {
        self.outputs.get(node_id).cloned()
    }

    #[must_use]
    pub fn conversation_history(&self, node_id: &str) -> Vec<ChatMessage> {
        self.transcript(node_id)
            .iter()
            .filter_map(|item| match item {
                AgentTranscriptItem::AssistantMessage { content } => Some(ChatMessage {
                    role: ChatRole::Assistant,
                    content: content.clone(),
                }),
                AgentTranscriptItem::UserMessage { content } => Some(ChatMessage {
                    role: ChatRole::User,
                    content: content.clone(),
                }),
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
        Ok(AgentRequest {
            workflow_id: self.workflow.id.clone(),
            node_id: node.id.clone(),
            node_label: node.label.clone(),
            model: node.agent.model.clone(),
            system_prompt: node.agent.system_prompt.clone(),
            task_prompt: node.agent.task_prompt.clone(),
            input: build_node_input(
                &node.id,
                &self.upstream_map,
                &self.outputs,
                self.entrypoint_text.as_deref(),
            ),
            output_schema: node.agent.output_schema.clone(),
            tool_config: node.agent.tools.clone(),
            available_tools: Vec::new(),
            transcript: self.transcript(&node.id).to_vec(),
        })
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::Node;
    use serde_json::json;

    fn node(id: &str) -> Node {
        let mut node = Node::agent(id, 0.0, 0.0);
        node.id = NodeId(id.to_string());
        node
    }

    #[tokio::test]
    async fn auto_start_node_runs_ai_and_completes() {
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

    #[tokio::test]
    async fn non_auto_start_node_pauses_awaiting_input() {
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

    #[tokio::test]
    async fn awaiting_manual_node_repeats_context_until_input_arrives() {
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

    #[tokio::test]
    async fn wrong_node_human_input_is_rejected_without_advancing() {
        let mut workflow = Workflow::new("manual");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();
        assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));

        let error = engine.on_human_input("other", "Wrong node").unwrap_err();
        let result = engine.poll();

        assert_eq!(error, "expected input for idea, got other");
        assert!(matches!(
            result,
            EnginePollResult::AwaitInput { ref node_id, .. } if node_id == "idea"
        ));
        assert!(engine.node_output("idea").is_none());
    }

    #[tokio::test]
    async fn manual_node_user_input_starts_ai_request() {
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

    #[tokio::test]
    async fn conversation_follow_up_repauses_same_node() {
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
                ChatMessage {
                    role: ChatRole::User,
                    content: "Need a smaller launch scope".to_string(),
                },
                ChatMessage {
                    role: ChatRole::Assistant,
                    content: "Which approval step is mandatory?".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn tool_calls_pause_for_approval_and_resume_after_results() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.catalog.tools = vec![crate::ToolRef {
            name: "read".to_string(),
        }];
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
        assert!(matches!(
            pending,
            EnginePollResult::AwaitToolApproval { ref node_id, .. } if node_id == "idea"
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

    #[tokio::test]
    async fn conversation_completion_sets_output_and_advances() {
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
}
