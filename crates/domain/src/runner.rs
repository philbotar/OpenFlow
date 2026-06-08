use crate::{
    execution_layers, AgentRequest, AgentTranscriptItem, AgentTurnOutcome, AiPort, Node, NodeId,
    NodeRunOutput, RunEvent, RunEventKind, RunReport, Workflow, WorkflowValidationError,
};
use futures::future::try_join_all;
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
    pub async fn run_with_entrypoint(
        &self,
        workflow: &Workflow,
        entrypoint_text: Option<&str>,
    ) -> Result<RunReport, RunError> {
        let layers = execution_layers(workflow)?;
        let upstream_map = build_upstream_map(workflow);
        let mut outputs = BTreeMap::new();
        let mut events = Vec::new();

        for layer in layers {
            let layer_results = try_join_all(layer.iter().map(|node_id| {
                self.invoke_headless_node(
                    workflow,
                    node_id,
                    &upstream_map,
                    &outputs,
                    entrypoint_text,
                )
            }))
            .await?;
            for result in layer_results {
                events.extend(result.events);
                outputs.insert(result.node_id, result.output);
            }
        }

        Ok(RunReport {
            workflow_id: workflow.id.clone(),
            events,
            outputs: outputs
                .into_iter()
                .map(|(node_id, output)| NodeRunOutput { node_id, output })
                .collect(),
        })
    }

    async fn invoke_headless_node(
        &self,
        workflow: &Workflow,
        node_id: &NodeId,
        upstream_map: &HashMap<NodeId, Vec<NodeId>>,
        outputs: &BTreeMap<NodeId, Value>,
        entrypoint_text: Option<&str>,
    ) -> Result<HeadlessNodeResult, RunError> {
        let node = workflow
            .nodes
            .iter()
            .find(|node| node.id == *node_id)
            .ok_or_else(|| RunError::NodeFailed {
                node_id: node_id.clone(),
                message: "node id from layers not found in workflow".to_string(),
            })?;
        if !node.agent.auto_start {
            return Err(RunError::NodeFailed {
                node_id: node_id.clone(),
                message: "headless runner cannot satisfy human input".to_string(),
            });
        }

        let mut events = vec![RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Queued,
            message: "queued".to_string(),
            output: None,
        }];
        let mut retries = 0;
        loop {
            events.push(RunEvent {
                node_id: node_id.clone(),
                kind: RunEventKind::Started,
                message: "invoking model".to_string(),
                output: None,
            });
            let request =
                build_agent_request(workflow, node, upstream_map, outputs, entrypoint_text);
            match self.ai.invoke(request).await {
                Ok(AgentTurnOutcome::Completed(success)) => {
                    events.push(RunEvent {
                        node_id: node_id.clone(),
                        kind: RunEventKind::Completed,
                        message: "completed".to_string(),
                        output: Some(success.output.clone()),
                    });
                    return Ok(HeadlessNodeResult {
                        node_id: node_id.clone(),
                        output: success.output,
                        events,
                    });
                }
                Ok(AgentTurnOutcome::ToolCalls(_)) => {
                    return Err(RunError::NodeFailed {
                        node_id: node_id.clone(),
                        message: "headless runner cannot satisfy tool execution".to_string(),
                    });
                }
                Ok(AgentTurnOutcome::NeedsUserInput(_)) => {
                    return Err(RunError::NodeFailed {
                        node_id: node_id.clone(),
                        message: "headless runner cannot satisfy human input".to_string(),
                    });
                }
                Err(error) => {
                    if error.is_retryable() && retries < workflow.settings.retry_policy.max_attempts
                    {
                        retries += 1;
                        events.push(RunEvent {
                            node_id: node_id.clone(),
                            kind: RunEventKind::Retrying,
                            message: format!(
                                "retrying after transient failure; backoff_ms={}",
                                workflow.settings.retry_policy.backoff_ms
                            ),
                            output: None,
                        });
                        continue;
                    }
                    events.push(RunEvent {
                        node_id: node_id.clone(),
                        kind: RunEventKind::Failed,
                        message: error.to_string(),
                        output: None,
                    });
                    return Err(RunError::NodeFailed {
                        node_id: node_id.clone(),
                        message: error.to_string(),
                    });
                }
            }
        }
    }
}

struct HeadlessNodeResult {
    node_id: NodeId,
    output: Value,
    events: Vec<RunEvent>,
}

pub(crate) fn build_upstream_map(workflow: &Workflow) -> HashMap<NodeId, Vec<NodeId>> {
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
    upstream_map
}

fn build_agent_request(
    workflow: &Workflow,
    node: &Node,
    upstream_map: &HashMap<NodeId, Vec<NodeId>>,
    outputs: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
) -> AgentRequest {
    AgentRequest {
        workflow_id: workflow.id.clone(),
        node_id: node.id.clone(),
        node_label: node.label.clone(),
        model: node.agent.model.clone(),
        system_prompt: workflow_system_prompt(workflow, node),
        task_prompt: node.agent.task_prompt.clone(),
        input: build_node_input(&node.id, upstream_map, outputs, entrypoint_text),
        output_schema: node.agent.output_schema.clone(),
        tool_config: node.agent.tools.clone(),
        available_tools: Vec::new(),
        transcript: Vec::<AgentTranscriptItem>::new(),
    }
}

pub(crate) fn workflow_system_prompt(workflow: &Workflow, node: &Node) -> String {
    let base = node.agent.system_prompt.clone();
    let shared = workflow.settings.shared_context.trim();
    if shared.is_empty() {
        return base;
    }
    format!("{base}\n\n--- Workflow context ---\n{shared}")
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
#[allow(
    clippy::items_after_statements,
    clippy::significant_drop_tightening,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
mod tests {
    use super::*;
    use crate::{
        AgentError, AgentRequest, AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, Edge,
        Node, ToolCall, ToolRef,
    };
    use async_trait::async_trait;
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct RecordingAi {
        requests: Arc<Mutex<Vec<AgentRequest>>>,
    }

    #[async_trait]
    impl AiPort for RecordingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.requests.lock().push(request.clone());
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({
                    "summary": format!("output from {}", request.node_id)
                }),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    struct FailingAi;

    #[async_trait]
    impl AiPort for FailingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Failed(format!(
                "synthetic failure for {}",
                request.node_id
            )))
        }
    }

    fn node(id: &str) -> Node {
        let mut node = Node::agent(id, 0.0, 0.0);
        node.id = NodeId(id.to_string());
        node.agent.model = "test-model".to_string();
        node
    }

    #[tokio::test]
    async fn waits_for_upstream_outputs_before_downstream_calls() {
        let mut workflow = Workflow::new("runner");
        workflow.nodes = vec![node("idea"), node("plan")];
        workflow.edges = vec![Edge::new("idea", "plan")];
        let ai = RecordingAi::default();
        let requests_handle = ai.requests.clone();
        let runner = WorkflowRunner::new(ai);

        let report = runner.run(&workflow).await.unwrap();
        let requests = requests_handle.lock();

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].node_id, "idea");
        assert_eq!(requests[1].node_id, "plan");
        assert_eq!(requests[1].input["upstream"][0]["node_id"], "idea");
        assert_eq!(report.outputs.len(), 2);
    }

    #[tokio::test]
    async fn rejects_invalid_workflow_before_model_call() {
        let workflow = Workflow::new("empty");
        let runner = WorkflowRunner::new(RecordingAi::default());

        let error = runner.run(&workflow).await.unwrap_err();
        assert!(matches!(error, RunError::Validation(_)));
    }

    #[tokio::test]
    async fn tool_enabled_node_requires_tool_execution_in_headless_runner() {
        let mut workflow = Workflow::new("tooling");
        let mut idea = node("idea");
        idea.agent.tools.catalog.tools = vec![ToolRef {
            name: "read".to_string(),
            tier: Some(crate::ToolTier::Read),
        }];
        workflow.nodes = vec![idea];

        struct ToolCallingAi;

        #[async_trait]
        impl AiPort for ToolCallingAi {
            async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
                Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: "{}".to_string(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                        intent: None,
                    }],
                }))
            }
        }

        let runner = WorkflowRunner::new(ToolCallingAi);
        let error = runner.run(&workflow).await.unwrap_err();
        assert!(matches!(
            error,
            RunError::NodeFailed { node_id, message }
                if node_id == "idea" && message == "headless runner cannot satisfy tool execution"
        ));
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
            .run_with_entrypoint(&workflow, Some("ORCHID-91 kickoff"))
            .await
            .unwrap();

        let (idea_input, plan_input) = {
            let requests = requests_handle.lock();
            let idea = requests.iter().find(|req| req.node_id == "idea").unwrap();
            let plan = requests.iter().find(|req| req.node_id == "plan").unwrap();
            (idea.input.clone(), plan.input.clone())
        };

        assert_eq!(
            idea_input,
            json!({
                "entrypoint": { "text": "ORCHID-91 kickoff" },
                "upstream": []
            })
        );
        assert_eq!(
            plan_input,
            json!({
                "upstream": [
                    {
                        "node_id": "idea",
                        "output": { "summary": "output from idea" }
                    }
                ]
            })
        );
    }

    #[tokio::test]
    async fn run_without_entrypoint_preserves_existing_input_shape() {
        let mut workflow = Workflow::new("default");
        workflow.nodes = vec![node("idea")];

        let ai = RecordingAi::default();
        let requests_handle = ai.requests.clone();
        let runner = WorkflowRunner::new(ai);

        runner.run(&workflow).await.unwrap();

        let input = { requests_handle.lock()[0].input.clone() };
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
            let requests = requests_handle.lock();
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
        assert!(matches!(
            error,
            RunError::NodeFailed { ref node_id, ref message }
                if node_id == "idea" && message == "synthetic failure for idea"
        ));
    }

    #[tokio::test]
    async fn independent_siblings_invoked_concurrently() {
        #[derive(Clone, Default)]
        struct ConcurrentAi {
            current: Arc<AtomicUsize>,
            max_seen: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl AiPort for ConcurrentAi {
            async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
                let current = self.current.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_seen.fetch_max(current, Ordering::SeqCst);
                for _ in 0..10 {
                    tokio::task::yield_now().await;
                }
                self.current.fetch_sub(1, Ordering::SeqCst);
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output: json!({
                        "summary": format!("output from {}", request.node_id)
                    }),
                    raw_text: "{}".to_string(),
                    assistant_message: None,
                }))
            }
        }

        let mut workflow = Workflow::new("parallel");
        workflow.nodes = vec![node("alpha"), node("beta")];
        let ai = ConcurrentAi::default();
        let max_seen = ai.max_seen.clone();
        let runner = WorkflowRunner::new(ai);

        runner.run(&workflow).await.unwrap();

        assert_eq!(max_seen.load(Ordering::SeqCst), 2);
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
