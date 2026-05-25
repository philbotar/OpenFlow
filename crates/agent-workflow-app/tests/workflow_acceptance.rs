use agent_workflow_app::execution::{run_workflow_headless, ManualInput};
use agent_workflow_app::state::TraceStatus;
use async_trait::async_trait;
use serde_json::json;
use std::sync::{Arc, Mutex};
use workflow_core::{AgentError, AgentRequest, AgentResponse, AiPort, Edge, Node, Workflow};

#[derive(Clone, Default)]
struct ScriptedAi {
    requests: Arc<Mutex<Vec<AgentRequest>>>,
}

#[async_trait]
impl AiPort for ScriptedAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        self.requests.lock().unwrap().push(request.clone());
        let output = match request.node_id.as_str() {
            "idea" => {
                let entrypoint = request.input["entrypoint"]["text"]
                    .as_str()
                    .unwrap_or_default();
                assert!(entrypoint.contains("ORCHID-91"));
                json!({
                    "project_code": "ORCHID-91",
                    "summary": "clarified"
                })
            }
            "plan" => {
                assert_eq!(request.input["upstream"][0]["node_id"], "idea");
                assert_eq!(
                    request.input["upstream"][0]["output"]["project_code"],
                    "ORCHID-91"
                );
                json!({
                    "project_code": "ORCHID-91",
                    "slices": ["extract execution driver", "add acceptance tests"]
                })
            }
            "risk" => {
                assert_eq!(request.input["upstream"][0]["node_id"], "idea");
                json!({
                    "project_code": "ORCHID-91",
                    "risks": ["live model output may vary"]
                })
            }
            "brief" => {
                let upstream_ids = request.input["upstream"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["node_id"].as_str().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(upstream_ids, vec!["plan", "risk"]);
                json!({
                    "project_code": "ORCHID-91",
                    "brief": "acceptance lane ready",
                    "next_action": "run cargo test"
                })
            }
            other => panic!("unexpected node id: {other}"),
        };
        Ok(AgentResponse {
            raw_text: output.to_string(),
            output,
        })
    }
}

fn agent(id: &str, label: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = id.to_string();
    node.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": true,
        "properties": {
            "project_code": { "type": "string" }
        },
        "required": ["project_code"]
    });
    node
}

fn branch_join_workflow() -> Workflow {
    let mut workflow = Workflow::new("Acceptance branch join");
    workflow.nodes = vec![
        agent("idea", "Clarify idea"),
        agent("plan", "Create plan"),
        agent("risk", "Find risks"),
        agent("brief", "Final brief"),
    ];
    workflow.edges = vec![
        Edge::new("idea", "plan"),
        Edge::new("idea", "risk"),
        Edge::new("plan", "brief"),
        Edge::new("risk", "brief"),
    ];
    workflow
}

#[tokio::test]
async fn branch_join_workflow_preserves_sentinel_and_trace_contract() {
    let ai = ScriptedAi::default();
    let snapshot = run_workflow_headless(
        branch_join_workflow(),
        Some("Plan project ORCHID-91".to_string()),
        ai.clone(),
        vec![],
    )
    .await
    .unwrap();

    assert_eq!(snapshot.outputs["brief"]["project_code"], "ORCHID-91");
    assert_eq!(snapshot.report.outputs.len(), 4);

    let completed = snapshot
        .run_trace
        .iter()
        .filter(|entry| entry.status == TraceStatus::Completed)
        .map(|entry| entry.node_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(completed, vec!["idea", "plan", "risk", "brief"]);

    let requests = ai.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["idea", "plan", "risk", "brief"]
    );
}

#[tokio::test]
async fn manual_node_pauses_accepts_input_and_feeds_downstream_node() {
    #[derive(Clone, Default)]
    struct DownstreamAi {
        requests: Arc<Mutex<Vec<AgentRequest>>>,
    }

    #[async_trait]
    impl AiPort for DownstreamAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
            self.requests.lock().unwrap().push(request.clone());
            assert_eq!(request.node_id, "final");
            assert_eq!(request.input["upstream"][0]["node_id"], "human-review");
            assert_eq!(
                request.input["upstream"][0]["output"],
                "human approved ORCHID-91"
            );
            let output = json!({
                "project_code": "ORCHID-91",
                "brief": "manual input accepted"
            });
            Ok(AgentResponse {
                raw_text: output.to_string(),
                output,
            })
        }
    }

    let mut manual = agent("human-review", "Human review");
    manual.agent.auto_start = false;
    let final_node = agent("final", "Final brief");
    let mut workflow = Workflow::new("Manual acceptance");
    workflow.nodes = vec![manual, final_node];
    workflow.edges = vec![Edge::new("human-review", "final")];

    let ai = DownstreamAi::default();
    let snapshot = run_workflow_headless(
        workflow,
        Some("Use project code ORCHID-91".to_string()),
        ai.clone(),
        vec![ManualInput {
            node_id: "human-review".to_string(),
            text: "human approved ORCHID-91".to_string(),
        }],
    )
    .await
    .unwrap();

    assert_eq!(snapshot.outputs["final"]["project_code"], "ORCHID-91");
    assert!(snapshot
        .run_trace
        .iter()
        .any(|entry| entry.node_id == "human-review" && entry.status == TraceStatus::Paused));
    assert!(snapshot
        .chat_logs
        .get("human-review")
        .unwrap()
        .iter()
        .any(|message| message.content == "human approved ORCHID-91"));
    assert_eq!(ai.requests.lock().unwrap().len(), 1);
}
