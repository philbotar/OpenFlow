use crate::{
    AiPort, EnginePollResult, InteractiveEngine, NodeId, RunReport, Workflow,
    WorkflowValidationError,
};
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
        let mut engine =
            InteractiveEngine::new(workflow.clone(), entrypoint_text.map(ToString::to_string))?;

        loop {
            match engine.poll() {
                EnginePollResult::CallAi { node_id, request } => {
                    let result = self.ai.invoke((*request).clone()).await;
                    engine.on_ai_complete(&node_id, result);
                }
                EnginePollResult::AwaitInput { node_id, .. } => {
                    return Err(RunError::NodeFailed {
                        node_id,
                        message: "headless runner cannot satisfy human input".to_string(),
                    });
                }
                EnginePollResult::AwaitToolApproval { node_id, .. } => {
                    return Err(RunError::NodeFailed {
                        node_id,
                        message: "headless runner cannot satisfy tool execution".to_string(),
                    });
                }
                EnginePollResult::Completed(report) => return Ok(report),
                EnginePollResult::Failed(error) => return Err(error),
            }
        }
    }
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
#[allow(clippy::items_after_statements, clippy::significant_drop_tightening)]
mod tests {
    use super::*;
    use crate::{
        AgentError, AgentRequest, AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, Edge,
        Node, ToolCall, ToolRef,
    };
    use async_trait::async_trait;
    use parking_lot::Mutex;
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
    async fn rejects_invalid_workflow_before_openai_call() {
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
