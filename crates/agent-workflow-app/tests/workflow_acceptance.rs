#![allow(clippy::significant_drop_tightening)]

use agent_workflow_app::execution::{run_workflow_headless, ApprovalResponse, ManualInput};
use agent_workflow_app::state::TraceStatus;
use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use workflow_core::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ApprovalMode, Edge, Node, NodeId, ToolCall, ToolRef, Workflow,
};

#[derive(Clone, Default)]
struct ScriptedAi {
    requests: Arc<Mutex<Vec<AgentRequest>>>,
}

#[async_trait]
impl AiPort for ScriptedAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.requests.lock().push(request.clone());
        let output = match &*request.node_id {
            "idea" => {
                let entrypoint = request.input["entrypoint"]["text"]
                    .as_str()
                    .unwrap_or_default();
                json!({"summary": format!("idea keeps {entrypoint}")})
            }
            "plan" => {
                let upstream = request.input["upstream"][0]["output"]["summary"]
                    .as_str()
                    .unwrap_or_default();
                json!({"summary": format!("plan extends {upstream}")})
            }
            "risk" => {
                let upstream = request.input["upstream"][0]["output"]["summary"]
                    .as_str()
                    .unwrap_or_default();
                json!({"summary": format!("risk checks {upstream}")})
            }
            "join" => {
                let joined = request.input["upstream"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["output"]["summary"].as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(" | ");
                json!({"summary": joined})
            }
            other => json!({"summary": format!("output from {other}")}),
        };
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output,
            raw_text: "{}".to_string(),
            assistant_message: None,
        }))
    }
}

fn agent(id: &str, label: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = NodeId(id.to_string());
    node.agent.model = "test-model".to_string();
    node.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "summary": { "type": "string" }
        },
        "required": ["summary"]
    });
    node
}

fn branch_join_workflow() -> Workflow {
    let mut workflow = Workflow::new("Acceptance branch join");
    workflow.nodes = vec![
        agent("idea", "Idea"),
        agent("plan", "Plan"),
        agent("risk", "Risk"),
        agent("join", "Join"),
    ];
    workflow.edges = vec![
        Edge::new("idea", "plan"),
        Edge::new("idea", "risk"),
        Edge::new("plan", "join"),
        Edge::new("risk", "join"),
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
        vec![],
    )
    .await
    .unwrap();

    assert_eq!(
        snapshot.outputs[&NodeId("join".into())],
        json!({"summary": "plan extends idea keeps Plan project ORCHID-91 | risk checks idea keeps Plan project ORCHID-91"})
    );
    assert!(
        snapshot
            .run_trace
            .iter()
            .filter(|entry| entry.status == TraceStatus::Completed)
            .count()
            >= 4
    );
    let requests = ai.requests.lock();
    assert_eq!(
        requests[0].input["entrypoint"]["text"],
        "Plan project ORCHID-91"
    );
}

#[tokio::test]
async fn manual_node_pauses_accepts_input_and_feeds_downstream_node() {
    #[derive(Clone, Default)]
    struct ManualAi {
        requests: Arc<Mutex<Vec<AgentRequest>>>,
    }

    #[async_trait]
    impl AiPort for ManualAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.requests.lock().push(request.clone());
            match &*request.node_id {
                "human-review" => {
                    let asked_already = request.transcript.iter().any(|item| {
                        matches!(
                            item,
                            workflow_core::AgentTranscriptItem::AssistantMessage { .. }
                        )
                    });
                    if !asked_already {
                        return Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                            raw_text: "{}".to_string(),
                            assistant_message: "Which approval is mandatory?".to_string(),
                        }));
                    }
                    let answer = request
                        .transcript
                        .iter()
                        .rev()
                        .find_map(|item| match item {
                            workflow_core::AgentTranscriptItem::UserMessage { content } => {
                                Some(content.clone())
                            }
                            _ => None,
                        })
                        .unwrap();
                    Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                        output: json!({"summary": answer}),
                        raw_text: "{}".to_string(),
                        assistant_message: Some("Locked. Advancing.".to_string()),
                    }))
                }
                "final" => {
                    assert_eq!(request.input["upstream"][0]["node_id"], "human-review");
                    Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                        output: json!({
                            "summary": request.input["upstream"][0]["output"]["summary"]
                        }),
                        raw_text: "{}".to_string(),
                        assistant_message: None,
                    }))
                }
                _ => unreachable!(),
            }
        }
    }

    let mut workflow = Workflow::new("manual acceptance");
    let mut review = agent("human-review", "Human review");
    review.agent.auto_start = false;
    let final_node = agent("final", "Final");
    workflow.nodes = vec![review, final_node];
    workflow.edges = vec![Edge::new("human-review", "final")];

    let snapshot = run_workflow_headless(
        workflow,
        Some("Use project code ORCHID-91".to_string()),
        ManualAi::default(),
        vec![
            ManualInput {
                node_id: NodeId("human-review".into()),
                text: "Need the mandatory approval".to_string(),
            },
            ManualInput {
                node_id: NodeId("human-review".into()),
                text: "Legal sign-off keeps ORCHID-91".to_string(),
            },
        ],
        vec![],
    )
    .await
    .unwrap();

    assert!(snapshot
        .run_trace
        .iter()
        .any(|entry| entry.node_id == "human-review" && entry.status == TraceStatus::Paused));
    assert_eq!(
        snapshot.outputs[&NodeId("final".into())],
        json!({"summary": "Legal sign-off keeps ORCHID-91"})
    );
}

#[tokio::test]
async fn tool_approval_pause_and_result_round_trip_preserve_run_integrity() {
    #[derive(Clone, Default)]
    struct ToolAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for ToolAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let mut calls = self.calls.lock();
            *calls += 1;
            if *calls == 1 {
                assert_eq!(request.available_tools.len(), 1);
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: Some("Need repo context".to_string()),
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                        intent: Some("Reading repository overview".to_string()),
                    }],
                }));
            }
            let saw_tool_result = request
                .transcript
                .iter()
                .any(|item| matches!(item, workflow_core::AgentTranscriptItem::ToolResult { .. }));
            assert!(saw_tool_result);
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "tool verified ORCHID-91"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let mut workflow = Workflow::new("tool acceptance");
    let mut node = agent("tool-node", "Tool node");
    node.agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
    }];
    node.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![node];

    let first_attempt =
        run_workflow_headless(workflow.clone(), None, ToolAi::default(), vec![], vec![]).await;
    assert!(matches!(
        first_attempt,
        Err(agent_workflow_app::execution::WorkflowExecutionError::MissingApproval(_))
    ));

    let snapshot = run_workflow_headless(
        workflow,
        None,
        ToolAi::default(),
        vec![],
        vec![ApprovalResponse {
            approval_id: String::new(),
            allow: true,
        }],
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot.outputs[&NodeId("tool-node".into())],
        json!({"summary": "tool verified ORCHID-91"})
    );
    assert!(!snapshot.tool_calls_by_node[&NodeId("tool-node".into())].is_empty());
}
