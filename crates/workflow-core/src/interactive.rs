use crate::{
    execution_layers, runner::build_node_input, AgentError, AgentRequest, AgentResponse, NodeId,
    NodeRunOutput, RunEvent, RunEventKind, RunReport, Workflow, WorkflowValidationError,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum EnginePollResult {
    CallAi {
        node_id: NodeId,
        request: AgentRequest,
    },
    AwaitInput {
        node_id: NodeId,
        label: String,
        context: String,
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
            .map(|n| (n.id.clone(), Vec::new()))
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

    fn find_node(&self, node_id: &str) -> Option<&crate::Node> {
        self.workflow.nodes.iter().find(|n| n.id == node_id)
    }

    pub fn poll(&mut self) -> EnginePollResult {
        // If a node is awaiting input, return its context again
        if let Some(ref awaiting_id) = self.awaiting_node {
            if let Some(node) = self.find_node(awaiting_id) {
                return EnginePollResult::AwaitInput {
                    node_id: awaiting_id.clone(),
                    label: node.label.clone(),
                    context: self.assemble_context(awaiting_id),
                };
            }
        }

        // Determine the current node id from layers
        let Some(current_node_id) = self
            .layers
            .get(self.layer_idx)
            .and_then(|l| l.get(self.node_idx))
            .cloned()
        else {
            return EnginePollResult::Completed(RunReport {
                workflow_id: self.workflow.id.clone(),
                events: self.events.clone(),
                outputs: self
                    .outputs
                    .iter()
                    .map(|(id, out)| NodeRunOutput {
                        node_id: id.clone(),
                        output: out.clone(),
                    })
                    .collect(),
            });
        };

        // Look up the node (immutable borrow of workflow nodes)
        let Some(node) = self.find_node(&current_node_id) else {
            return EnginePollResult::Failed(crate::RunError::NodeFailed {
                node_id: current_node_id,
                message: "node id from layers not found in workflow".to_string(),
            });
        };

        // Clone everything we need from the node before any mutable operations
        let node_id = node.id.clone();
        let node_label = node.label.clone();
        let auto_start = node.agent.auto_start;

        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Queued,
            message: "queued".to_string(),
            output: None,
        });

        if !auto_start {
            self.awaiting_node = Some(node_id.clone());
            return EnginePollResult::AwaitInput {
                node_id,
                label: node_label,
                context: self.assemble_context(&current_node_id),
            };
        }

        // Build the request — need a fresh immutable borrow of workflow nodes via build_request
        let request = self.build_request(&current_node_id);
        EnginePollResult::CallAi { node_id, request }
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
            }
            Err(error) => {
                let msg = error.to_string();
                self.events.push(RunEvent {
                    node_id: NodeId(node_id.to_string()),
                    kind: RunEventKind::Failed,
                    message: msg,
                    output: None,
                });
            }
        }
        self.advance();
    }

    #[must_use]
    pub fn node_output(&self, node_id: &str) -> Option<Value> {
        self.outputs.get(node_id).cloned()
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
        let value = json!(text);
        self.outputs
            .insert(NodeId(node_id.to_string()), value.clone());
        self.events.push(RunEvent {
            node_id: NodeId(node_id.to_string()),
            kind: RunEventKind::Completed,
            message: "completed via human input".to_string(),
            output: Some(value),
        });
        self.advance();
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

    fn assemble_context(&self, node_id: &str) -> String {
        let upstream = self.upstream_map.get(node_id).cloned().unwrap_or_default();
        let mut ctx = String::new();
        for up_id in &upstream {
            if let Some(output) = self.outputs.get(up_id) {
                writeln!(ctx, "{up_id}: {output}").unwrap();
            }
        }
        if ctx.is_empty() {
            if let Some(text) = self.entrypoint_text.as_deref() {
                writeln!(ctx, "Entrypoint: {text}").unwrap();
            }
        }
        let node = self.workflow.nodes.iter().find(|n| n.id == node_id);
        if let Some(node) = node {
            write!(ctx, "\nTask: {}", node.agent.task_prompt).unwrap();
        }
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Node;

    fn node(id: &str) -> Node {
        let mut node = Node::agent(id, 0.0, 0.0);
        node.id = NodeId(id.to_string());
        node
    }

    #[tokio::test]
    async fn auto_start_node_runs_ai_and_completes() {
        use crate::AgentResponse;
        let mut workflow = Workflow::new("test");
        workflow.nodes = vec![node("idea")];
        let mut engine = InteractiveEngine::new(workflow, None).unwrap();

        let result = engine.poll();
        assert!(
            matches!(result, EnginePollResult::CallAi { ref node_id, .. } if node_id == "idea")
        );

        let EnginePollResult::CallAi { request, .. } = result else {
            panic!("expected CallAi")
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
        assert!(
            matches!(result, EnginePollResult::AwaitInput { ref node_id, .. } if node_id == "idea")
        );
    }

    #[tokio::test]
    async fn human_input_becomes_node_output() {
        let mut workflow = Workflow::new("test");
        let mut idea = node("idea");
        idea.agent.auto_start = false;
        workflow.nodes = vec![idea];

        let mut engine = InteractiveEngine::new(workflow, None).unwrap();
        engine.poll(); // AwaitInput
        engine.on_human_input("idea", "User decision").unwrap();

        let result = engine.poll();
        assert!(
            matches!(result, EnginePollResult::Completed(ref report) if report.outputs[0].output == json!("User decision"))
        );
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
        assert!(
            matches!(result, EnginePollResult::AwaitInput { ref node_id, .. } if node_id == "idea")
        );
        assert!(engine.node_output("idea").is_none());
    }
}
