use crate::{
    execution_layers, runner::build_node_input, AgentError, AgentRequest, AgentResponse,
    ChatMessage, ChatRole, ConversationAgentRequest, ConversationAgentResponse, NodeId,
    NodeRunOutput, RunEvent, RunEventKind, RunReport, Workflow, WorkflowValidationError,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum EnginePollResult {
    CallAi {
        node_id: NodeId,
        request: AgentRequest,
    },
    CallConversationAi {
        node_id: NodeId,
        request: ConversationAgentRequest,
    },
    AwaitInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    Completed(RunReport),
    Failed(crate::RunError),
}

pub struct InteractiveEngine {
    workflow: Workflow,
    upstream_map: HashMap<NodeId, Vec<NodeId>>,
    layers: Vec<Vec<NodeId>>,
    layer_idx: usize,
    node_idx: usize,
    outputs: BTreeMap<NodeId, Value>,
    conversations: BTreeMap<NodeId, Vec<ChatMessage>>,
    events: Vec<RunEvent>,
    awaiting_node: Option<NodeId>,
    entrypoint_text: Option<String>,
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
            conversations: BTreeMap::new(),
            events: Vec::new(),
            awaiting_node: None,
            entrypoint_text,
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
        if let Some(awaiting_id) = self.awaiting_node.clone() {
            let Some(node) = self.find_node(&awaiting_id) else {
                return EnginePollResult::Failed(crate::RunError::NodeFailed {
                    node_id: awaiting_id,
                    message: "awaiting node no longer exists".to_string(),
                });
            };
            return EnginePollResult::AwaitInput {
                node_id: node.id.clone(),
                label: node.label.clone(),
                context: self.assemble_context(&node.id),
                is_initial: self.conversation_history(&node.id).is_empty(),
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
            return EnginePollResult::Failed(crate::RunError::NodeFailed {
                node_id,
                message: "node id from layers not found in workflow".to_string(),
            });
        };
        let node_id = node.id.clone();
        let node_label = node.label.clone();
        let auto_start = node.agent.auto_start;

        if auto_start {
            self.events.push(RunEvent {
                node_id: node_id.clone(),
                kind: RunEventKind::Queued,
                message: "queued".to_string(),
                output: None,
            });
            return EnginePollResult::CallAi {
                node_id: node_id.clone(),
                request: self.build_request(&node_id),
            };
        }

        if self.conversation_history(&node_id).is_empty() {
            self.events.push(RunEvent {
                node_id: node_id.clone(),
                kind: RunEventKind::Queued,
                message: "queued".to_string(),
                output: None,
            });
            self.awaiting_node = Some(node_id.clone());
            self.conversations.entry(node_id.clone()).or_default();
            return EnginePollResult::AwaitInput {
                node_id: node_id.clone(),
                label: node_label,
                context: self.assemble_context(&node_id),
                is_initial: true,
            };
        }

        EnginePollResult::CallConversationAi {
            node_id: node_id.clone(),
            request: self.build_conversation_request(&node_id),
        }
    }

    pub fn on_ai_complete(&mut self, node_id: &str, result: Result<AgentResponse, AgentError>) {
        self.events.push(RunEvent {
            node_id: NodeId(node_id.to_string()),
            kind: RunEventKind::Started,
            message: "started OpenAI node call".to_string(),
            output: None,
        });

        match result {
            Ok(response) => {
                self.outputs
                    .insert(NodeId(node_id.to_string()), response.output.clone());
                self.events.push(RunEvent {
                    node_id: NodeId(node_id.to_string()),
                    kind: RunEventKind::Completed,
                    message: "completed".to_string(),
                    output: Some(response.output),
                });
                self.advance();
            }
            Err(error) => {
                self.events.push(RunEvent {
                    node_id: NodeId(node_id.to_string()),
                    kind: RunEventKind::Failed,
                    message: error.to_string(),
                    output: None,
                });
            }
        }
    }

    pub fn on_conversation_ai_complete(
        &mut self,
        node_id: &str,
        result: Result<ConversationAgentResponse, AgentError>,
    ) {
        self.events.push(RunEvent {
            node_id: NodeId(node_id.to_string()),
            kind: RunEventKind::Started,
            message: "started OpenAI node call".to_string(),
            output: None,
        });

        match result {
            Ok(response) => {
                if let Some(message) = response.assistant_message {
                    self.conversations
                        .entry(NodeId(node_id.to_string()))
                        .or_default()
                        .push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: message,
                        });
                }

                if response.ready_to_advance {
                    let Some(output) = response.output else {
                        self.events.push(RunEvent {
                            node_id: NodeId(node_id.to_string()),
                            kind: RunEventKind::Failed,
                            message: "conversation response missing final output".to_string(),
                            output: None,
                        });
                        return;
                    };
                    self.outputs
                        .insert(NodeId(node_id.to_string()), output.clone());
                    self.events.push(RunEvent {
                        node_id: NodeId(node_id.to_string()),
                        kind: RunEventKind::Completed,
                        message: "completed".to_string(),
                        output: Some(output),
                    });
                    self.advance();
                } else {
                    self.awaiting_node = Some(NodeId(node_id.to_string()));
                }
            }
            Err(error) => {
                self.events.push(RunEvent {
                    node_id: NodeId(node_id.to_string()),
                    kind: RunEventKind::Failed,
                    message: error.to_string(),
                    output: None,
                });
            }
        }
    }

    #[must_use]
    pub fn node_output(&self, node_id: &str) -> Option<Value> {
        self.outputs.get(node_id).cloned()
    }

    #[must_use]
    pub fn conversation_history(&self, node_id: &str) -> &[ChatMessage] {
        self.conversations.get(node_id).map_or(&[], Vec::as_slice)
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
        self.conversations
            .entry(NodeId(node_id.to_string()))
            .or_default()
            .push(ChatMessage {
                role: ChatRole::User,
                content: text.to_string(),
            });
        Ok(())
    }

    fn build_request(&self, node_id: &str) -> AgentRequest {
        let node = self.find_node(node_id).expect("node must exist");
        AgentRequest {
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
        }
    }

    fn build_conversation_request(&self, node_id: &str) -> ConversationAgentRequest {
        let node = self.find_node(node_id).expect("node must exist");
        ConversationAgentRequest {
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
            conversation: self.conversation_history(node_id).to_vec(),
        }
    }

    fn assemble_context(&self, node_id: &str) -> String {
        let upstream = self.upstream_map.get(node_id).cloned().unwrap_or_default();
        let mut context = String::new();
        for upstream_id in &upstream {
            if let Some(output) = self.outputs.get(upstream_id) {
                writeln!(context, "{upstream_id}: {output}").expect("write context");
            }
        }
        if context.is_empty() {
            if let Some(text) = self.entrypoint_text.as_deref() {
                writeln!(context, "Entrypoint: {text}").expect("write context");
            }
        }
        if let Some(node) = self.find_node(node_id) {
            write!(context, "\nTask: {}", node.agent.task_prompt).expect("write context");
        }
        context
    }
}

#[cfg(test)]
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
            Ok(AgentResponse {
                output: json!({"summary": "ok"}),
                raw_text: "...".to_string(),
            }),
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
    async fn manual_node_user_input_starts_conversation_request() {
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
        let EnginePollResult::CallConversationAi { request, .. } = result else {
            panic!("expected conversation request");
        };
        assert_eq!(request.node_id, "idea");
        assert_eq!(
            request.conversation,
            vec![ChatMessage {
                role: ChatRole::User,
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
        engine.on_conversation_ai_complete(
            "idea",
            Ok(ConversationAgentResponse {
                ready_to_advance: false,
                assistant_message: Some("Which approval step is mandatory?".to_string()),
                output: None,
                raw_text: "...".to_string(),
            }),
        );

        let result = engine.poll();
        assert!(matches!(
            result,
            EnginePollResult::AwaitInput { ref node_id, is_initial: false, .. } if node_id == "idea"
        ));
        assert_eq!(
            engine.conversation_history("idea"),
            &[
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
        engine.on_conversation_ai_complete(
            "idea",
            Ok(ConversationAgentResponse {
                ready_to_advance: true,
                assistant_message: Some("Locked. Advancing.".to_string()),
                output: Some(json!({"summary": "Workflow execution with approvals"})),
                raw_text: "...".to_string(),
            }),
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
