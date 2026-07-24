mod support;

use async_trait::async_trait;
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ApprovalMode, Edge, NodeId, ToolCall, Workflow,
};
use orchestration::run::execution::{
    new_artifact_root, new_in_memory_snapshot_store, run_workflow_headless,
    spawn_interactive_workflow_run, ApprovalResponse, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, ManualInput,
};
use orchestration::run::state::TraceStatus;
use orchestration::settings::model::McpSettings;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::{agent_node as agent, branch_join_workflow, run_headless_script, HeadlessRunOpts};
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
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn branch_join_workflow_preserves_sentinel_and_trace_contract() {
    let ai = ScriptedAi::default();
    let snapshot = run_headless_script(
        branch_join_workflow(),
        ai.clone(),
        HeadlessRunOpts {
            entrypoint: Some("Plan project ORCHID-91".to_string()),
            ..HeadlessRunOpts::default()
        },
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
    assert!(
        snapshot
            .run_trace
            .iter()
            .any(|entry| entry.status == TraceStatus::Completed),
        "durable replay depends on completed trace entries being projected"
    );
    assert!(
        snapshot
            .chat_logs
            .values()
            .any(|messages| !messages.is_empty()),
        "durable replay depends on chat logs being projected"
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

#[cfg_attr(miri, ignore)]
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
                            reasoning: vec![],
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
                        reasoning: Vec::new(),
                        usage: None,
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
                        reasoning: Vec::new(),
                        usage: None,
                    }))
                }
                _ => unreachable!(),
            }
        }
    }

    let mut workflow = Workflow::new("manual acceptance");
    let mut review = agent("human-review", "Human review");
    review.agent.auto_start = false;
    review.agent.request_user_input = true;
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

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn conversational_node_can_pause_on_consecutive_explicit_input_requests() {
    #[derive(Clone, Default)]
    struct ConversationalAi;

    #[async_trait]
    impl AiPort for ConversationalAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let assistant_turns = request
                .transcript
                .iter()
                .filter(|item| matches!(item, engine::AgentTranscriptItem::AssistantMessage { .. }))
                .count();
            match assistant_turns {
                0 => Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    raw_text: "Which Supabase products should this include?".to_string(),
                    assistant_message: "Which Supabase products should this include?".to_string(),
                    reasoning: Vec::new(),
                })),
                1 => Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    raw_text: "Should I add Storage too?".to_string(),
                    assistant_message: "Should I add Storage too?".to_string(),
                    reasoning: Vec::new(),
                })),
                _ => {
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
                        .expect("second answer");
                    Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                        output: json!({"summary": answer}),
                        raw_text: "{}".to_string(),
                        assistant_message: None,
                        reasoning: Vec::new(),
                        usage: None,
                    }))
                }
            }
        }
    }

    let mut workflow = Workflow::new("conversational acceptance");
    let mut brief = agent("brief", "Feature brief");
    brief.agent.request_user_input = true;
    workflow.nodes = vec![brief];

    let snapshot = run_workflow_headless(
        workflow,
        None,
        ConversationalAi,
        vec![
            ManualInput {
                node_id: NodeId("brief".into()),
                text: "Auth and Postgres. What else?".to_string(),
            },
            ManualInput {
                node_id: NodeId("brief".into()),
                text: "Add Storage too.".to_string(),
            },
        ],
        Vec::new(),
        BTreeMap::new(),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        snapshot.outputs[&NodeId("brief".into())],
        json!({"summary": "Add Storage too."})
    );
    assert_eq!(
        snapshot
            .run_trace
            .iter()
            .filter(|entry| entry.node_id == "brief" && entry.status == TraceStatus::Paused)
            .count(),
        2
    );
}

#[cfg_attr(miri, ignore)]
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
                assert_eq!(request.available_tools.len(), 10);
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: Some("Need repo context".to_string()),
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                    }],
                    reasoning: vec![],
                    usage: None,
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
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    let mut workflow = Workflow::new("tool acceptance");
    let mut node = agent("tool-node", "Tool node");
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

#[cfg_attr(miri, ignore)]
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
                    reasoning: vec![],
                    usage: None,
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
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("draft.txt");
    let execution_cwd = Some(dir.path().to_path_buf());

    let mut workflow = Workflow::new("write tool acceptance");
    let mut node = agent("write-node", "Write node");
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
        None,
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
        None,
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

#[derive(Clone)]
struct CheckpointWriteToolAi {
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl AiPort for CheckpointWriteToolAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        use std::sync::atomic::Ordering;
        let call_number = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call_number == 1 {
            return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: Some("Saving draft".to_string()),
                tool_calls: vec![ToolCall {
                    id: "call-write".to_string(),
                    name: "write".to_string(),
                    arguments: json!({"path": "draft.txt", "content": "checkpoint ORCHID-91\n"}),
                }],
                reasoning: vec![],
                usage: None,
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
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

fn write_approval_workflow() -> Workflow {
    let mut workflow = Workflow::new("checkpoint mid approval");
    let mut node = agent("write-node", "Write node");
    node.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![node];
    workflow
}

fn checkpoint_interactive_params<A: AiPort + Send + Sync + 'static>(
    workflow: Workflow,
    execution_cwd: std::path::PathBuf,
    ai: A,
    resume_checkpoint: Option<engine::InteractiveEngineCheckpoint>,
    checkpoint_sink: Arc<
        parking_lot::Mutex<Option<orchestration::run::persistence::PendingRunCheckpoint>>,
    >,
) -> InteractiveWorkflowRunParams<A> {
    InteractiveWorkflowRunParams {
        workflow,
        entrypoint: None,
        execution_cwd,
        project_repository_root: None,
        artifact_root: new_artifact_root(),
        resume_checkpoint,
        checkpoint_sink,
        ai,
        agent_snapshots: BTreeMap::new(),
        snapshot_store: new_in_memory_snapshot_store(),
        lsp: orchestration::lsp::LspSettings::from_env(),
        pending_engine_reverts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        node_interrupts: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
        context_window_sizes: BTreeMap::new(),
        mcp: McpSettings {
            discover_external: false,
            ..McpSettings::default()
        },
        search: orchestration::settings::model::SearchSettings::default(),
        runtime_config_store: engine::new_runtime_config_store(),
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn checkpoint_resume_mid_approval_replays_batch() {
    use parking_lot::Mutex as ParkingMutex;
    use std::time::Duration;
    use tokio::time::timeout;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("draft.txt");
    let workflow = write_approval_workflow();
    let ai = CheckpointWriteToolAi {
        calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let checkpoint_sink = Arc::new(ParkingMutex::new(None));
    let params = checkpoint_interactive_params(
        workflow.clone(),
        dir.path().to_path_buf(),
        ai.clone(),
        None,
        checkpoint_sink.clone(),
    );

    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    let mut captured_approval_id = None;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if let ExecutionEvent::ToolApprovalRequested { request } = &event {
            captured_approval_id = Some(request.approval_id.clone());
            action_tx.send(ExecutionAction::Stop).expect("stop");
        }
        if matches!(event, ExecutionEvent::Aborted) {
            break;
        }
    }
    handle.await.expect("drive task");

    let approval_id = captured_approval_id.expect("expected tool approval before stop");
    let checkpoint = checkpoint_sink
        .lock()
        .clone()
        .expect("checkpoint after stop")
        .engine;
    assert!(
        checkpoint.pending_tool_batches.contains_key(&approval_id),
        "checkpoint must retain pending approval batch"
    );

    let resume_params = checkpoint_interactive_params(
        workflow,
        dir.path().to_path_buf(),
        ai,
        Some(checkpoint),
        Arc::new(ParkingMutex::new(None)),
    );

    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), resume_params);

    let mut replayed_approval_id = None;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if let ExecutionEvent::ToolApprovalRequested { request } = &event {
            replayed_approval_id = Some(request.approval_id.clone());
            assert_eq!(request.tool_call.id, "call-write");
            action_tx
                .send(ExecutionAction::ResolveApproval {
                    approval_id: request.approval_id.clone(),
                    allow: true,
                    reason: None,
                })
                .expect("approve");
        }
        if matches!(
            event,
            ExecutionEvent::NodeCompleted { ref node_id, .. } if node_id.0 == "write-node"
        ) {
            break;
        }
    }
    handle.await.expect("resume drive task");

    assert_eq!(replayed_approval_id.as_deref(), Some(approval_id.as_str()));
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "checkpoint ORCHID-91\n"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn failed_read_tool_feeds_error_and_node_completes() {
    #[derive(Clone, Default)]
    struct RecoverAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for RecoverAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let n = {
                let mut calls = self.calls.lock();
                *calls += 1;
                *calls
            };
            if n == 1 {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "missing-acceptance-file.txt"}),
                    }],
                    reasoning: vec![],
                    usage: None,
                }));
            }
            assert!(request.transcript.iter().any(|item| matches!(
                item,
                engine::AgentTranscriptItem::ToolResult { result } if result.is_error
            )));
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok after tool error"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let mut workflow = Workflow::new("tool resilience");
    let mut node = agent("worker", "Worker");
    node.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![node];

    let snapshot = run_workflow_headless(
        workflow,
        None,
        RecoverAi::default(),
        vec![],
        vec![],
        BTreeMap::new(),
        Some(temp.path().to_path_buf()),
        None,
    )
    .await
    .expect("acceptance run completes");

    assert_eq!(
        snapshot.report.outputs.len(),
        1,
        "node should complete after tool error"
    );
}

/// Regression probe: `read` on a missing file yields `[not_found]`; `search` on a
/// missing path should not silently succeed with "No matches found".
#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn search_missing_path_surfaces_not_found_not_empty_success() {
    #[derive(Clone, Default)]
    struct SearchMissingAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for SearchMissingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let n = {
                let mut calls = self.calls.lock();
                *calls += 1;
                *calls
            };
            if n == 1 {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-search".to_string(),
                        name: "search".to_string(),
                        arguments: json!({
                            "pattern": "ORCHID-91",
                            "paths": "missing-acceptance-target.txt"
                        }),
                    }],
                    reasoning: vec![],
                    usage: None,
                }));
            }
            let tool_result = request.transcript.iter().find_map(|item| match item {
                engine::AgentTranscriptItem::ToolResult { result } => Some(result),
                _ => None,
            });
            let Some(result) = tool_result else {
                panic!("model should receive a search tool result on turn two");
            };
            assert!(
                result.is_error,
                "missing search path should surface a tool error, got success: {}",
                result.content
            );
            assert!(
                result.content.contains("[not_found]"),
                "missing search path should use [not_found], got: {}",
                result.content
            );
            assert!(
                !result.content.contains("No matches found"),
                "missing path must not masquerade as zero-match success: {}",
                result.content
            );
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok after search path error"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let mut workflow = Workflow::new("search missing path");
    let mut node = agent("worker", "Worker");
    node.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![node];

    let snapshot = run_workflow_headless(
        workflow,
        None,
        SearchMissingAi::default(),
        vec![],
        vec![],
        BTreeMap::new(),
        Some(temp.path().to_path_buf()),
        None,
    )
    .await
    .expect("run should complete after surfacing search path error");

    assert_eq!(
        snapshot.outputs[&NodeId("worker".into())],
        json!({"summary": "ok after search path error"})
    );
}

fn summary_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": { "summary": { "type": "string" } },
        "required": ["summary"]
    })
}

/// Node-aware AI for output-repair acceptance: distinguishes worker, repair, and downstream.
#[derive(Clone)]
struct OutputRepairAcceptanceAi {
    requests: Arc<Mutex<Vec<AgentRequest>>>,
    /// When true, overseer returns an invalid completed envelope instead of `repaired_arguments`.
    invalid_repair: bool,
    root_malformed_remaining: Arc<Mutex<u32>>,
}

impl OutputRepairAcceptanceAi {
    fn success() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            invalid_repair: false,
            root_malformed_remaining: Arc::new(Mutex::new(1)),
        }
    }

    fn invalid_repair() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            invalid_repair: true,
            root_malformed_remaining: Arc::new(Mutex::new(8)),
        }
    }
}

#[async_trait]
impl AiPort for OutputRepairAcceptanceAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.requests.lock().push(request.clone());
        let node_id: &str = &request.node_id;

        if node_id.ends_with("__output_repair") {
            if self.invalid_repair {
                return Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output: json!({"not_repaired": true}),
                    raw_text: "{}".into(),
                    assistant_message: Some("overseer prose".into()),
                    reasoning: Vec::new(),
                    usage: None,
                }));
            }
            return Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({
                    "repaired_arguments": {
                        "output": { "summary": "FIXED-REPAIR" }
                    }
                }),
                raw_text: "{}".into(),
                assistant_message: Some("clear me".into()),
                reasoning: Vec::new(),
                usage: None,
            }));
        }

        match node_id {
            "root" => {
                let should_fail = {
                    let mut remaining = self.root_malformed_remaining.lock();
                    if *remaining > 0 {
                        *remaining -= 1;
                        true
                    } else {
                        false
                    }
                };
                if should_fail {
                    let secret = "SECRET_SENTINEL_must_not_leak";
                    let raw = format!(r#"{{"output":{{"bad":true}},"leak":"{secret}"}}"#);
                    return Err(engine::malformed_submit_invalid_json(
                        "acceptance",
                        &raw,
                        "schema violation",
                        Some(&summary_schema()),
                        Some("call_root".into()),
                        None,
                        None,
                    ));
                }
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output: json!({"summary": "should-not-reach-without-repair"}),
                    raw_text: "{}".into(),
                    assistant_message: None,
                    reasoning: Vec::new(),
                    usage: None,
                }))
            }
            "down" => {
                let upstream = &request.input["upstream"][0]["output"];
                assert_eq!(
                    upstream,
                    &json!({"summary": "FIXED-REPAIR"}),
                    "downstream must receive repaired root output"
                );
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output: json!({
                        "summary": format!(
                            "down saw {}",
                            upstream["summary"].as_str().unwrap_or_default()
                        )
                    }),
                    raw_text: "{}".into(),
                    assistant_message: None,
                    reasoning: Vec::new(),
                    usage: None,
                }))
            }
            other => Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": format!("unexpected {other}")}),
                raw_text: "{}".into(),
                assistant_message: None,
                reasoning: Vec::new(),
                usage: None,
            })),
        }
    }
}

fn root_down_workflow(overseer_model: Option<&str>) -> Workflow {
    let mut workflow = Workflow::new("output repair acceptance");
    let mut root = agent("root", "Root");
    root.agent.model = "worker-m".into();
    root.agent.output_schema = summary_schema();
    let mut down = agent("down", "Down");
    down.agent.model = "down-m".into();
    down.agent.output_schema = summary_schema();
    workflow.nodes = vec![root, down];
    workflow.edges = vec![Edge::new("root", "down")];
    workflow.settings.output_repair_model = overseer_model.map(str::to_string);
    workflow.settings.retry_policy.max_attempts = 0;
    workflow
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn output_repair_propagates_repaired_output_downstream() {
    let ai = OutputRepairAcceptanceAi::success();
    let requests = ai.requests.clone();
    let snapshot = run_headless_script(
        root_down_workflow(Some("overseer-m")),
        ai,
        HeadlessRunOpts::default(),
    )
    .await
    .expect("repaired run completes");

    assert_eq!(
        snapshot.outputs[&NodeId("root".into())],
        json!({"summary": "FIXED-REPAIR"})
    );
    assert_eq!(
        snapshot.outputs[&NodeId("down".into())],
        json!({"summary": "down saw FIXED-REPAIR"})
    );

    let repair_trace: Vec<_> = snapshot
        .run_trace
        .iter()
        .filter(|entry| entry.node_label == "output repair")
        .collect();
    assert!(
        repair_trace.iter().any(|e| e.message.contains("repairing")),
        "expected repair started: {repair_trace:?}"
    );
    assert!(
        repair_trace.iter().any(|e| e.message.contains("repaired")),
        "expected repair succeeded: {repair_trace:?}"
    );

    let secret = "SECRET_SENTINEL_must_not_leak";
    for entry in &snapshot.run_trace {
        assert!(
            !entry.message.contains(secret),
            "secret in trace: {}",
            entry.message
        );
    }
    for messages in snapshot.chat_logs.values() {
        for message in messages {
            assert!(
                !message.content.contains(secret),
                "secret in chat: {}",
                message.content
            );
        }
    }

    let recorded = requests.lock().clone();
    let repair = recorded
        .iter()
        .find(|r| r.node_id.ends_with("__output_repair"))
        .expect("repair request");
    assert_eq!(repair.model, "overseer-m");
    assert!(repair.transcript.is_empty());
    assert!(repair.available_tools.is_empty());
    assert!(!repair.allow_user_input);

    let root = recorded
        .iter()
        .find(|r| &*r.node_id == "root")
        .expect("root request");
    assert_eq!(root.model, "worker-m");
    let down = recorded
        .iter()
        .find(|r| &*r.node_id == "down")
        .expect("down request");
    assert_eq!(down.model, "down-m");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn output_repair_inherits_worker_model_when_unset() {
    let ai = OutputRepairAcceptanceAi::success();
    let requests = ai.requests.clone();
    run_headless_script(root_down_workflow(None), ai, HeadlessRunOpts::default())
        .await
        .expect("repaired run completes");

    let repair = requests
        .lock()
        .iter()
        .find(|r| r.node_id.ends_with("__output_repair"))
        .expect("repair request")
        .clone();
    assert_eq!(repair.model, "worker-m");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn output_repair_invalid_overseer_keeps_retryable_path_and_redacts_secret() {
    let ai = OutputRepairAcceptanceAi::invalid_repair();
    let error = run_headless_script(
        root_down_workflow(Some("overseer-m")),
        ai,
        HeadlessRunOpts::default(),
    )
    .await
    .expect_err("invalid repair should exhaust retries");

    assert!(
        matches!(
            &error,
            orchestration::run::execution::WorkflowExecutionError::MissingRetry(node_id)
                if node_id.0 == "root"
        ),
        "expected MissingRetry on root, got: {error}"
    );
    let message = error.to_string();
    assert!(
        !message.contains("SECRET_SENTINEL_must_not_leak"),
        "secret leaked in error: {message}"
    );
}
