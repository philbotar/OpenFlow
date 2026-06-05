use crate::{
    execution_layers, AgentError, AgentRequest, AgentResponse, AiPort, NodeId, NodeRunOutput,
    RunEvent, RunEventKind, RunReport, Workflow, WorkflowValidationError,
};
use futures::future::join_all;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RunError {
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error("node {node_id} failed: {message}")]
    NodeFailed { node_id: NodeId, message: String },
}

pub struct WorkflowRunner<A> {
    ai: A,
}

impl<A> WorkflowRunner<A>
where
    A: AiPort,
{
    pub const fn new(ai: A) -> Self {
        Self { ai }
    }

    /// # Errors
    /// Returns an error if the workflow is invalid or a node call fails.
    pub async fn run(&self, workflow: &Workflow) -> Result<RunReport, RunError> {
        self.run_with_entrypoint(workflow, None).await
    }

    /// # Errors
    /// Returns an error if the workflow is invalid or a node call fails.
    ///
    /// # Panics
    /// Panics if a layer references a node id that was not found in the validated node map.
    pub async fn run_with_entrypoint(
        &self,
        workflow: &Workflow,
        entrypoint_text: Option<&str>,
    ) -> Result<RunReport, RunError> {
        let layers = execution_layers(workflow)?;
        let nodes_by_id: HashMap<NodeId, _> = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect();
        let upstream_by_node = upstream_map(workflow);
        let mut events = Vec::new();
        let mut outputs_by_node: BTreeMap<NodeId, Value> = BTreeMap::new();

        for layer in layers {
            for node_id in &layer {
                events.push(RunEvent {
                    node_id: node_id.clone(),
                    kind: RunEventKind::Queued,
                    message: "queued after upstream dependencies completed".to_string(),
                    output: None,
                });
            }

            let calls = layer.iter().map(|node_id| {
                let node = nodes_by_id
                    .get(node_id)
                    .expect("layer contains validated node id");
                let request = AgentRequest {
                    workflow_id: workflow.id.clone(),
                    node_id: node.id.clone(),
                    node_label: node.label.clone(),
                    model: node.agent.model.clone(),
                    system_prompt: node.agent.system_prompt.clone(),
                    task_prompt: node.agent.task_prompt.clone(),
                    input: build_node_input(
                        node_id,
                        &upstream_by_node,
                        &outputs_by_node,
                        entrypoint_text,
                    ),
                    output_schema: node.agent.output_schema.clone(),
                };

                async move { (node_id.clone(), self.ai.invoke(request).await) }
            });

            for node_id in &layer {
                events.push(RunEvent {
                    node_id: node_id.clone(),
                    kind: RunEventKind::Started,
                    message: "started OpenAI node call".to_string(),
                    output: None,
                });
            }

            let responses: Vec<(NodeId, Result<AgentResponse, AgentError>)> = join_all(calls).await;

            for (node_id, response) in responses {
                match response {
                    Ok(response) => {
                        outputs_by_node.insert(node_id.clone(), response.output.clone());
                        events.push(RunEvent {
                            node_id,
                            kind: RunEventKind::Completed,
                            message: "completed OpenAI node call".to_string(),
                            output: Some(response.output),
                        });
                    }
                    Err(error) => {
                        let message = error.to_string();
                        events.push(RunEvent {
                            node_id: node_id.clone(),
                            kind: RunEventKind::Failed,
                            message: message.clone(),
                            output: None,
                        });
                        return Err(RunError::NodeFailed { node_id, message });
                    }
                }
            }
        }

        Ok(RunReport {
            workflow_id: workflow.id.clone(),
            events,
            outputs: outputs_by_node
                .into_iter()
                .map(|(node_id, output)| NodeRunOutput { node_id, output })
                .collect(),
        })
    }
}

fn upstream_map(workflow: &Workflow) -> HashMap<NodeId, Vec<NodeId>> {
    let mut upstream: HashMap<NodeId, Vec<NodeId>> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), Vec::new()))
        .collect();

    for edge in &workflow.edges {
        upstream
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }

    for ids in upstream.values_mut() {
        ids.sort();
    }

    upstream
}

pub(crate) fn build_node_input(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
    outputs_by_node: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
) -> Value {
    let upstream = upstream_by_node
        .get(node_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .filter_map(|id| {
            outputs_by_node.get(id).map(|output| {
                json!({
                    "node_id": id,
                    "output": output
                })
            })
        })
        .collect::<Vec<_>>();

    if upstream.is_empty() {
        if let Some(text) = entrypoint_text.filter(|text| !text.trim().is_empty()) {
            return json!({
                "entrypoint": { "text": text },
                "upstream": []
            });
        }
    }

    json!({
        "upstream": upstream
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Node};
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingAi {
        requests: Arc<Mutex<Vec<AgentRequest>>>,
    }

    #[async_trait]
    impl AiPort for RecordingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
            self.requests.lock().unwrap().push(request.clone());
            Ok(AgentResponse {
                output: json!({
                    "summary": format!("output from {}", request.node_id)
                }),
                raw_text: "{\"summary\":\"ok\"}".to_string(),
            })
        }

        async fn invoke_conversation(
            &self,
            _request: crate::ConversationAgentRequest,
        ) -> Result<crate::ConversationAgentResponse, AgentError> {
            Err(AgentError::Failed(
                "conversation path not used in runner tests".to_string(),
            ))
        }
    }

    struct FailingAi;

    #[async_trait]
    impl AiPort for FailingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
            Err(AgentError::Failed(format!(
                "synthetic failure for {}",
                request.node_id
            )))
        }

        async fn invoke_conversation(
            &self,
            _request: crate::ConversationAgentRequest,
        ) -> Result<crate::ConversationAgentResponse, AgentError> {
            Err(AgentError::Failed(
                "conversation path not used in runner tests".to_string(),
            ))
        }
    }

    fn node(id: &str) -> Node {
        let mut node = Node::agent(id, 0.0, 0.0);
        node.id = NodeId(id.to_string());
        node
    }

    #[tokio::test]
    async fn waits_for_upstream_outputs_before_downstream_calls() {
        let mut workflow = Workflow::new("runner");
        workflow.nodes = vec![node("idea"), node("plan")];
        workflow.edges = vec![Edge::new("idea", "plan")];
        let ai = RecordingAi::default();
        let runner = WorkflowRunner::new(ai);

        let report = runner.run(&workflow).await.unwrap();

        assert_eq!(report.outputs.len(), 2);
        let completed_events = report
            .events
            .iter()
            .filter(|event| event.kind == RunEventKind::Completed)
            .count();
        assert_eq!(completed_events, 2);
    }

    #[tokio::test]
    async fn rejects_invalid_workflow_before_openai_call() {
        let workflow = Workflow::new("empty");
        let runner = WorkflowRunner::new(RecordingAi::default());

        let error = runner.run(&workflow).await.unwrap_err();

        assert!(matches!(
            error,
            RunError::Validation(WorkflowValidationError::EmptyWorkflow)
        ));
    }

    #[tokio::test]
    async fn fan_out_nodes_run_in_same_layer() {
        let mut workflow = Workflow::new("fan out");
        workflow.nodes = vec![node("idea"), node("plan"), node("risk")];
        workflow.edges = vec![Edge::new("idea", "plan"), Edge::new("idea", "risk")];
        let runner = WorkflowRunner::new(RecordingAi::default());

        let report = runner.run(&workflow).await.unwrap();

        assert_eq!(report.outputs.len(), 3);
        assert_eq!(report.outputs[0].node_id, "idea");
        assert_eq!(report.outputs[1].node_id, "plan");
        assert_eq!(report.outputs[2].node_id, "risk");
    }

    #[tokio::test]
    async fn injects_entrypoint_into_root_node_input_only() {
        let mut workflow = Workflow::new("entrypoint");
        workflow.nodes = vec![node("idea"), node("plan")];
        workflow.edges = vec![Edge::new("idea", "plan")];

        let ai = RecordingAi::default();
        let requests_handle = ai.requests.clone();
        let runner = WorkflowRunner::new(ai);

        runner
            .run_with_entrypoint(&workflow, Some("Draft a launch plan"))
            .await
            .unwrap();

        let (idea_input, plan_input) = {
            let requests = requests_handle.lock().unwrap();
            let idea = requests
                .iter()
                .find(|req| req.node_id == "idea")
                .unwrap()
                .input
                .clone();
            let plan = requests
                .iter()
                .find(|req| req.node_id == "plan")
                .unwrap()
                .input
                .clone();
            drop(requests);
            (idea, plan)
        };

        assert_eq!(
            idea_input["entrypoint"]["text"],
            json!("Draft a launch plan")
        );
        assert!(plan_input.get("entrypoint").is_none());
    }

    #[tokio::test]
    async fn run_without_entrypoint_preserves_existing_input_shape() {
        let mut workflow = Workflow::new("default");
        workflow.nodes = vec![node("idea")];

        let ai = RecordingAi::default();
        let requests_handle = ai.requests.clone();
        let runner = WorkflowRunner::new(ai);

        runner.run(&workflow).await.unwrap();

        let input = { requests_handle.lock().unwrap()[0].input.clone() };
        assert_eq!(input, json!({"upstream": []}));
    }

    #[tokio::test]
    async fn downstream_request_receives_sorted_upstream_outputs() {
        let mut workflow = Workflow::new("join");
        workflow.nodes = vec![node("root"), node("alpha"), node("beta"), node("join")];
        workflow.edges = vec![
            Edge::new("root", "beta"),
            Edge::new("root", "alpha"),
            Edge::new("beta", "join"),
            Edge::new("alpha", "join"),
        ];
        let ai = RecordingAi::default();
        let requests_handle = ai.requests.clone();
        let runner = WorkflowRunner::new(ai);

        runner.run(&workflow).await.unwrap();

        let join_input = {
            let requests = requests_handle.lock().unwrap();
            requests
                .iter()
                .find(|req| req.node_id == "join")
                .unwrap()
                .input
                .clone()
        };
        assert_eq!(
            join_input,
            json!({
                "upstream": [
                    {
                        "node_id": "alpha",
                        "output": { "summary": "output from alpha" }
                    },
                    {
                        "node_id": "beta",
                        "output": { "summary": "output from beta" }
                    }
                ]
            })
        );
    }

    #[tokio::test]
    async fn node_failure_returns_node_failed_error() {
        let mut workflow = Workflow::new("failure");
        workflow.nodes = vec![node("idea")];
        let runner = WorkflowRunner::new(FailingAi);

        let error = runner.run(&workflow).await.unwrap_err();

        match error {
            RunError::NodeFailed { node_id, message } => {
                assert_eq!(node_id, "idea");
                assert_eq!(message, "synthetic failure for idea");
            }
            other @ RunError::Validation(_) => panic!("expected node failure, got {other:?}"),
        }
    }

    #[test]
    fn blank_entrypoint_is_not_injected_into_root_input() {
        let input = build_node_input(
            "idea",
            &HashMap::from([(NodeId("idea".to_string()), Vec::new())]),
            &BTreeMap::new(),
            Some("   "),
        );

        assert_eq!(input, json!({"upstream": []}));
    }
}
