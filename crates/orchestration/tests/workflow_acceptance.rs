use async_trait::async_trait;
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ApprovalMode, Edge, Node, NodeId, ToolCall, ToolRef, Workflow,
};
use orchestration::run::execution::{run_workflow_headless, ApprovalResponse, ManualInput};
use orchestration::run::state::TraceStatus;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
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
        BTreeMap::new(),
        None,
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
    let entrypoint_text = {
        let requests = ai.requests.lock();
        requests[0].input["entrypoint"]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(entrypoint_text, "Plan project ORCHID-91");
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
                        matches!(item, engine::AgentTranscriptItem::AssistantMessage { .. })
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
                            engine::AgentTranscriptItem::UserMessage { content } => {
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
        BTreeMap::new(),
        None,
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
            let call_number = {
                let mut calls = self.calls.lock();
                *calls += 1;
                *calls
            };
            if call_number == 1 {
                assert_eq!(request.available_tools.len(), 3);
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: Some("Need repo context".to_string()),
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                    }],
                }));
            }
            let saw_tool_result = request
                .transcript
                .iter()
                .any(|item| matches!(item, engine::AgentTranscriptItem::ToolResult { .. }));
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
        tier: Some(engine::ToolTier::Read),
    }];
    node.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![node];

    let first_attempt = run_workflow_headless(
        workflow.clone(),
        None,
        ToolAi::default(),
        vec![],
        vec![],
        BTreeMap::new(),
        None,
    )
    .await;
    assert!(matches!(
        first_attempt,
        Err(orchestration::run::execution::WorkflowExecutionError::MissingApproval(_))
    ));

    let snapshot = run_workflow_headless(
        workflow,
        None,
        ToolAi::default(),
        vec![],
        vec![ApprovalResponse {
            approval_id: String::new(),
            allow: true,
            reason: None,
        }],
        BTreeMap::new(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot.outputs[&NodeId("tool-node".into())],
        json!({"summary": "tool verified ORCHID-91"})
    );
    assert!(!snapshot.tool_calls_by_node[&NodeId("tool-node".into())].is_empty());
}

#[tokio::test]
async fn write_tool_requires_approval_and_mutates_file_after_allow() {
    #[derive(Clone, Default)]
    struct WriteAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for WriteAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let call_number = {
                let mut calls = self.calls.lock();
                *calls += 1;
                *calls
            };
            if call_number == 1 {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: Some("Saving draft".to_string()),
                    tool_calls: vec![ToolCall {
                        id: "call-write".to_string(),
                        name: "write".to_string(),
                        arguments: json!({"path": "draft.txt", "content": "saved ORCHID-91\n"}),
                    }],
                }));
            }
            let saw_tool_result = request
                .transcript
                .iter()
                .any(|item| matches!(item, engine::AgentTranscriptItem::ToolResult { .. }));
            assert!(saw_tool_result);
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "draft saved ORCHID-91"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("draft.txt");
    let execution_cwd = Some(dir.path().to_path_buf());

    let mut workflow = Workflow::new("write tool acceptance");
    let mut node = agent("write-node", "Write node");
    node.agent.tools.catalog.tools = vec![ToolRef {
        name: "write".to_string(),
        tier: Some(engine::ToolTier::Write),
    }];
    node.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![node];

    let first_attempt = run_workflow_headless(
        workflow.clone(),
        None,
        WriteAi::default(),
        vec![],
        vec![],
        BTreeMap::new(),
        execution_cwd.clone(),
    )
    .await;
    assert!(matches!(
        first_attempt,
        Err(orchestration::run::execution::WorkflowExecutionError::MissingApproval(_))
    ));
    assert!(!target.exists());

    let snapshot = run_workflow_headless(
        workflow,
        None,
        WriteAi::default(),
        vec![],
        vec![ApprovalResponse {
            approval_id: String::new(),
            allow: true,
            reason: None,
        }],
        BTreeMap::new(),
        execution_cwd,
    )
    .await
    .unwrap();

    assert_eq!(
        snapshot.outputs[&NodeId("write-node".into())],
        json!({"summary": "draft saved ORCHID-91"})
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "saved ORCHID-91\n"
    );
}
