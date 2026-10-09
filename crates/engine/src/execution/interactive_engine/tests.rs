#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::type_complexity,
    reason = "engine tests use unwrap/expect and panic for concise failure messages"
)]

use super::recovery::RecoveryCounters;
use super::{
    checkpoint::CheckpointError, EngineInputError, EngineRunResult, EngineUserMessage,
    InteractiveEngine, PendingToolBatch, RunError, MAX_MALFORMED_REQUEST_INPUT_RETRIES,
    MAX_MIXED_TOOL_TURN_RETRIES, MAX_OUTPUT_TRUNCATION_RETRIES,
};
use crate::conversation::{AgentTranscriptItem, ChatAttachmentKind, ChatAttachmentRef, ChatRole};
use crate::execution::{HandoffArtifact, HandoffFormat, NodeFailureKind};
use crate::graph::{
    new_runtime_config_store, upsert_runtime_patch, Node, NodeId, NodeRuntimeConfigPatch,
    PlanModeConfig, Workflow,
};
use crate::ports::{
    AgentError, AgentMessageTurn, AgentNeedUserInput, AgentRequest, AgentToolCallBatch,
    AgentTurnOutcome, AgentTurnSuccess, AiPort, PartialToolCall, StructuredUserInput,
    ToolAccessPolicy, ToolBatchEffects, ToolBatchOutput, ToolPort, UserInputOption,
    UserInputQuestion, UserInputSelectionMode,
};
use crate::tools::{ApprovalMode, FileChangeRecord, ToolCall, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn node(id: &str) -> Node {
    let mut node = Node::agent(id, 0.0, 0.0);
    node.id = NodeId(id.to_string());
    node.agent.model = "test-model".to_string();
    node
}

fn image_attachment() -> ChatAttachmentRef {
    ChatAttachmentRef {
        id: "attachment-1".to_string(),
        file_name: "diagram.png".to_string(),
        media_type: "image/png".to_string(),
        size_bytes: 4,
        sha256: "abcd".to_string(),
        kind: ChatAttachmentKind::Image,
    }
}

fn mark_plan_sealed(engine: &mut InteractiveEngine, node_id: &NodeId) {
    engine
        .transcripts
        .entry(node_id.clone())
        .or_default()
        .push(AgentTranscriptItem::ToolResult {
            result: ToolResult {
                tool_call_id: "seal".to_string(),
                tool_name: crate::tools::WRITE_PLAN_ARTIFACT_TOOL.to_string(),
                content: "sealed".to_string(),
                is_error: false,
                artifact_ids: vec!["sealed-plan".to_string()],
                output_meta: None,
            },
        });
}

struct NoopToolPort;

#[async_trait]
impl ToolPort for NoopToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
        _policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        ToolBatchOutput {
            results: calls
                .into_iter()
                .map(|call| ToolResult {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: "noop".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                })
                .collect(),
            effects: ToolBatchEffects::default(),
        }
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
}

struct PlanningRecordingToolPort {
    batches: Arc<Mutex<Vec<(Vec<String>, ToolAccessPolicy)>>>,
}

#[async_trait]
impl ToolPort for PlanningRecordingToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
        policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        self.batches
            .lock()
            .expect("batches lock")
            .push((calls.iter().map(|call| call.name.clone()).collect(), policy));
        ToolBatchOutput {
            results: calls
                .into_iter()
                .map(|call| ToolResult {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: "ok".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                })
                .collect(),
            effects: ToolBatchEffects::default(),
        }
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
}

fn engine_with(nodes: Vec<Node>) -> InteractiveEngine {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = nodes;
    InteractiveEngine::new(workflow, None, None).unwrap()
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn mark_node_interrupted_closes_dangling_tool_calls() {
    let mut engine = engine_with(vec![node("a")]);
    let call = ToolCall::new("call-1", "bash", json!({}));
    engine
        .transcripts
        .entry(NodeId("a".to_string()))
        .or_default()
        .push(AgentTranscriptItem::ToolCall { call: call.clone() });
    engine.test_insert_pending_batch(PendingToolBatch {
        approval_id: "ap-1".to_string(),
        node_id: NodeId("a".to_string()),
        tool_calls: vec![call],
        requires_approval: false,
        tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
    });

    engine.mark_node_interrupted(&NodeId("a".to_string()));

    let transcript = engine.transcript(&NodeId("a".to_string()));
    assert_eq!(transcript.len(), 2);
    match &transcript[1] {
        AgentTranscriptItem::ToolResult { result } => {
            assert_eq!(result.tool_call_id, "call-1");
            assert!(result.is_error);
        }
        other => panic!("expected tool result, got {other:?}"),
    }
}

#[test]
fn retry_node_saturates_retry_counter() {
    let mut engine = engine_with(vec![node("a")]);
    for _ in 0..300 {
        engine
            .failed_nodes
            .insert(NodeId("a".to_string()), "boom".to_string());
        engine.retry_node(&NodeId("a".to_string())).unwrap();
    }
    assert_eq!(
        engine.model_attempt_for_node(&NodeId("a".to_string())),
        u8::MAX
    );
}

#[test]
fn retry_failed_node_preserves_conversation_and_resets_recovery_budgets() {
    let mut engine = engine_with(vec![node("a")]);
    let node_id = NodeId("a".to_string());
    engine
        .failed_nodes
        .insert(node_id.clone(), "boom".to_string());
    engine.transcripts.insert(
        node_id.clone(),
        vec![AgentTranscriptItem::UserMessage {
            content: "keep retry context".to_string(),
            attachments: Vec::new(),
        }],
    );
    engine
        .recovery
        .transient_streaks_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .submit_output_retries_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .request_input_retries_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .empty_turn_retries_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .mixed_tool_turn_retries_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .output_truncation_retries_by_node
        .insert(node_id.clone(), 2);
    engine
        .recovery
        .auto_continue_streaks_by_node
        .insert(node_id.clone(), 2);

    engine.retry_node(&node_id).unwrap();

    assert_eq!(engine.transcript(&node_id).len(), 1);
    assert!(!engine
        .recovery
        .transient_streaks_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .submit_output_retries_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .request_input_retries_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .empty_turn_retries_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .mixed_tool_turn_retries_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .output_truncation_retries_by_node
        .contains_key(&node_id));
    assert!(!engine
        .recovery
        .auto_continue_streaks_by_node
        .contains_key(&node_id));
    assert_eq!(engine.model_attempt_for_node(&node_id), 2);
}

#[test]
fn retry_failed_node_appends_new_human_message() {
    let mut engine = engine_with(vec![node("a")]);
    let node_id = NodeId("a".to_string());
    engine
        .failed_nodes
        .insert(node_id.clone(), "boom".to_string());

    engine
        .retry_node_with_message(&node_id, "Try again", Vec::new())
        .unwrap();

    assert!(matches!(
        engine.transcript(&node_id).last(),
        Some(AgentTranscriptItem::UserMessage { content, attachments })
            if content == "Try again" && attachments.is_empty()
    ));
    assert_eq!(engine.model_attempt_for_node(&node_id), 2);
}

#[test]
fn retry_failed_node_uses_the_latest_runtime_model() {
    struct RecordingAi {
        requests: Arc<Mutex<Vec<AgentRequest>>>,
    }

    #[async_trait]
    impl AiPort for RecordingAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.requests.lock().expect("requests lock").push(request);
            Ok(completed(json!({"summary": "recovered"})))
        }
    }

    let mut workflow = Workflow::new("wf");
    let mut agent = node("a");
    agent.agent.model = "broken-model".to_string();
    workflow.nodes.push(agent);
    let node_id = NodeId("a".to_string());
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    engine
        .failed_nodes
        .insert(node_id.clone(), "model unavailable".to_string());
    engine.transcripts.insert(
        node_id.clone(),
        vec![AgentTranscriptItem::UserMessage {
            content: "preserve this context".to_string(),
            attachments: Vec::new(),
        }],
    );
    let store = new_runtime_config_store();
    engine.set_runtime_config_store(store.clone());
    upsert_runtime_patch(
        &store,
        node_id.clone(),
        &NodeRuntimeConfigPatch {
            model: Some("working-model".to_string()),
            ..NodeRuntimeConfigPatch::default()
        },
    );
    engine.retry_node(&node_id).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let ai = RecordingAi {
        requests: Arc::clone(&requests),
    };

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::Completed(_)
        ));
    });

    let request = {
        let requests = requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        requests[0].clone()
    };
    assert_eq!(request.model, "working-model");
    assert!(request.transcript.iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::UserMessage { content, .. }
                if content == "preserve this context"
        )
    }));
}

#[test]
fn retry_interrupted_node_preserves_conversation() {
    let mut engine = engine_with(vec![node("a")]);
    let node_id = NodeId("a".to_string());
    engine.interrupted_nodes.insert(node_id.clone());
    engine
        .recovery
        .transient_streaks_by_node
        .insert(node_id.clone(), 2);
    engine.transcripts.insert(
        node_id.clone(),
        vec![AgentTranscriptItem::AssistantMessage {
            content: "keep me".to_string(),
        }],
    );

    engine.retry_node(&node_id).unwrap();

    assert_eq!(engine.transcript(&node_id).len(), 1);
    assert!(!engine
        .recovery
        .transient_streaks_by_node
        .contains_key(&node_id));
}

async fn run_once<A: AiPort, T: ToolPort>(
    engine: &mut InteractiveEngine,
    ai: &A,
    tools: &T,
) -> EngineRunResult {
    engine.run(ai, tools, &CancellationToken::new()).await
}

fn user_messages(request: &AgentRequest) -> Vec<&str> {
    request
        .transcript
        .iter()
        .filter_map(|item| match item {
            AgentTranscriptItem::UserMessage { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn mixed_tool_turn_is_corrected_and_retried_without_executing_calls() {
    let ai = ScriptedAi::new(vec![
        Err(AgentError::mixed_tool_turn(
            "test provider",
            "openflow_submit_node_output, write",
        )),
        Ok(completed(json!({"status": "complete"}))),
    ]);

    let result = run_once(&mut engine_with(vec![node("idea")]), &ai, &NoopToolPort).await;

    assert!(matches!(result, EngineRunResult::Completed(_)));
    let requests = ai.captured.lock().expect("requests lock");
    assert_eq!(requests.len(), 2);
    assert!(user_messages(&requests[1])
        .iter()
        .any(|m| m.contains("mixed harness and executable tools") && m.contains("write")));
}

#[tokio::test]
async fn output_truncation_retries_with_small_write_feedback() {
    let ai = ScriptedAi::new(vec![
        Err(AgentError::output_truncated(
            "test provider",
            "output token limit",
            vec![PartialToolCall::new(
                "call-1",
                "internal-1",
                Some("write".to_string()),
                r#"{"path":"docs/large.md","content":"secret partial"#,
            )],
        )),
        Ok(completed(json!({"status": "complete"}))),
    ]);

    let result = run_once(&mut engine_with(vec![node("idea")]), &ai, &NoopToolPort).await;

    assert!(matches!(result, EngineRunResult::Completed(_)));
    let requests = ai.captured.lock().expect("requests lock");
    assert_eq!(requests.len(), 2);
    let feedback = *user_messages(&requests[1])
        .iter()
        .find(|m| m.contains("output token limit"))
        .expect("retry feedback");
    assert!(feedback.contains("write"));
    assert!(feedback.contains("smaller"));
    assert!(!feedback.contains("secret partial"));
    assert!(!feedback.contains("docs/large.md"));
}

#[test]
fn output_truncation_fails_after_retry_cap() {
    let mut engine = engine_with(vec![node("idea")]);
    let node_id = NodeId::from("idea");
    let truncated =
        || AgentError::output_truncated("test provider", "output token limit", Vec::new());

    for _ in 0..MAX_OUTPUT_TRUNCATION_RETRIES {
        engine.test_deliver_ai(&node_id, Err(truncated()));
    }
    assert!(!engine.failed_nodes.contains_key(&node_id));

    engine.test_deliver_ai(&node_id, Err(truncated()));
    assert!(engine
        .failed_nodes
        .get(&node_id)
        .is_some_and(|error| error.contains("output token limit")));
}

#[test]
fn mixed_tool_turn_fails_after_retry_cap() {
    let mut engine = engine_with(vec![node("idea")]);
    let node_id = NodeId::from("idea");
    let mixed =
        || AgentError::mixed_tool_turn("test provider", "openflow_submit_node_output, write");

    for _ in 0..MAX_MIXED_TOOL_TURN_RETRIES {
        engine.test_deliver_ai(&node_id, Err(mixed()));
    }
    assert!(!engine.failed_nodes.contains_key(&node_id));

    engine.test_deliver_ai(&node_id, Err(mixed()));
    assert!(engine
        .failed_nodes
        .get(&node_id)
        .is_some_and(|error| error.contains("mixed incompatible tools")));
}

struct CompleteAi {
    output: Value,
    captured: Arc<Mutex<Vec<AgentRequest>>>,
}

impl CompleteAi {
    fn new(output: Value) -> Self {
        Self {
            output,
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl AiPort for CompleteAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.captured.lock().expect("lock").push(request);
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: self.output.clone(),
            raw_text: "{}".to_string(),
            ..Default::default()
        }))
    }
}

struct ScriptedAi {
    steps: Mutex<Vec<Result<AgentTurnOutcome, AgentError>>>,
    captured: Arc<Mutex<Vec<AgentRequest>>>,
}

impl ScriptedAi {
    fn new(steps: Vec<Result<AgentTurnOutcome, AgentError>>) -> Self {
        Self {
            steps: Mutex::new(steps),
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl AiPort for ScriptedAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.captured.lock().expect("lock").push(request);
        let mut steps = self.steps.lock().expect("lock");
        if steps.is_empty() {
            return Err(AgentError::Failed("scripted ai exhausted".to_string()));
        }
        steps.remove(0)
    }
}

struct InterleavedUserMessageAi {
    calls: Arc<AtomicUsize>,
    captured: Arc<Mutex<Vec<AgentRequest>>>,
}

#[async_trait]
impl AiPort for InterleavedUserMessageAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.captured.lock().expect("capture lock").push(request);
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![ToolCall::new(
                    "call-1",
                    "read",
                    json!({ "path": "README.md" }),
                )],
                reasoning: Vec::new(),
                usage: None,
            }));
        }
        Ok(message("tool finished"))
    }
}

struct InterleavedBlockingToolPort {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl ToolPort for InterleavedBlockingToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
        _policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        self.started.notify_one();
        self.release.notified().await;
        ToolBatchOutput {
            results: calls
                .into_iter()
                .map(|call| ToolResult {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: "ok".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                })
                .collect(),
            effects: ToolBatchEffects::default(),
        }
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
}

#[tokio::test]
async fn run_accepts_user_message_while_tool_is_in_flight() {
    let mut workflow = Workflow::new("interleaved-message");
    let mut chat = conversation_node("chat");
    chat.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![chat];
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");
    let ai = InterleavedUserMessageAi {
        calls: Arc::new(AtomicUsize::new(0)),
        captured: Arc::new(Mutex::new(Vec::new())),
    };
    let captured = Arc::clone(&ai.captured);
    let tools = InterleavedBlockingToolPort {
        started: Arc::new(tokio::sync::Notify::new()),
        release: Arc::new(tokio::sync::Notify::new()),
    };
    let tool_started = Arc::clone(&tools.started);
    let tool_release = Arc::clone(&tools.release);
    let cancel = CancellationToken::new();
    let (message_tx, mut message_rx) = tokio::sync::mpsc::unbounded_channel();
    let node_id = NodeId::from("chat");

    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut run =
            Box::pin(engine.run_with_user_messages(&ai, &tools, &cancel, &mut message_rx));
        tokio::select! {
            () = tool_started.notified() => {
                message_tx
                    .send(EngineUserMessage {
                        node_id,
                        text: "while tool runs".to_string(),
                        attachments: Vec::new(),
                        system_prompt: None,
                    })
                    .expect("message channel");
                tool_release.notify_one();
            }
            result = &mut run => panic!("run returned before tool release: {result:?}"),
        }
        run.await
    })
    .await
    .expect("interleaved run");

    assert!(matches!(result, EngineRunResult::NeedsInteraction { .. }));
    let (captured_len, contains_message) = {
        let captured = captured.lock().expect("capture lock");
        let contains_message = captured[1].transcript.iter().any(|item| {
            matches!(
                item,
                AgentTranscriptItem::UserMessage { content, .. }
                    if content == "while tool runs"
            )
        });
        (captured.len(), contains_message)
    };
    assert_eq!(captured_len, 2);
    assert!(contains_message);
}

#[tokio::test(start_paused = true)]
async fn run_sleeps_backoff_before_transient_retry() {
    let mut workflow = Workflow::new("backoff");
    workflow.settings.retry_policy.max_attempts = 1;
    workflow.settings.retry_policy.backoff_ms = 500;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = ScriptedAi::new(vec![
        Err(AgentError::Transient("timeout".to_string())),
        Ok(completed(json!({"summary": "ok"}))),
    ]);
    let started = tokio::time::Instant::now();

    let result = engine
        .run(&ai, &NoopToolPort, &CancellationToken::new())
        .await;

    assert!(matches!(result, EngineRunResult::Completed(_)));
    assert_eq!(ai.captured.lock().expect("lock").len(), 2);
    assert!(started.elapsed() >= Duration::from_millis(500));
}

#[tokio::test(start_paused = true)]
async fn run_cancel_during_backoff_returns_cancelled() {
    let mut workflow = Workflow::new("cancel");
    workflow.settings.retry_policy.max_attempts = 3;
    workflow.settings.retry_policy.backoff_ms = 5_000;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = ScriptedAi::new(vec![Err(AgentError::Transient("timeout".to_string()))]);
    let cancel = CancellationToken::new();
    let cancel_handle = cancel.clone();
    let started = tokio::time::Instant::now();
    let handle = tokio::spawn(async move { engine.run(&ai, &NoopToolPort, &cancel).await });
    tokio::task::yield_now().await;
    cancel_handle.cancel();
    let result = handle.await.expect("run task");

    assert!(matches!(result, EngineRunResult::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn checkpoint_preserves_transient_streaks_by_node() {
    let mut workflow = Workflow::new("wf");
    workflow.settings.retry_policy.max_attempts = 5;
    workflow.nodes = vec![node("a")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let node_a = NodeId("a".to_string());
    engine.test_deliver_ai(&node_a, Err(AgentError::Transient("blip".into())));

    let checkpoint = engine.prepare_stop_checkpoint();
    assert_eq!(checkpoint.transient_streaks_by_node.get(&node_a), Some(&1));

    let mut restored =
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    let again = restored.prepare_stop_checkpoint();
    assert_eq!(again.transient_streaks_by_node.get(&node_a), Some(&1));
}

#[test]
fn workflows_without_plan_mode_keep_execution_tools_and_no_packet_input() {
    let mut workflow = Workflow::new("normal-workflow");
    workflow.nodes = vec![node("implement")];
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");

    let request = engine
        .build_request(&NodeId::from("implement"))
        .expect("request");

    assert_eq!(request.tool_access_policy, ToolAccessPolicy::Execution);
    assert!(request.input.get("change_evidence_packet").is_none());
    assert!(engine.plan_mode_phase().is_none());
}

#[test]
fn plan_mode_checkpoint_pins_source_and_packet_when_workflow_settings_change() {
    let mut workflow = Workflow::new("plan-mode");
    let mut freeze = node("freeze");
    freeze.agent.request_user_input = true;
    let implement = node("implement");
    let verify = node("verify");
    let review = node("review");
    workflow.nodes = vec![freeze, implement, verify, review];
    workflow.edges = ["implement", "verify", "review"]
        .into_iter()
        .map(|target| crate::graph::Edge {
            id: crate::graph::EdgeId::from(format!("freeze-{target}")),
            from: NodeId::from("freeze"),
            to: NodeId::from(target),
        })
        .collect();
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("freeze"),
    });

    let freeze_id = NodeId::from("freeze");
    let downstream_ids = ["implement", "verify", "review"];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).expect("engine");
    assert_eq!(
        engine.plan_mode_phase(),
        Some(super::PlanModePhase::Planning)
    );
    engine.test_insert_in_flight(freeze_id.clone());
    mark_plan_sealed(&mut engine, &freeze_id);
    engine.on_ai_complete(
        &freeze_id,
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({
                "scope": "only this change",
                "planArtifact": "artifact:sealed-plan",
                "planArtifactSha256": "deadbeef"
            }),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    );
    let checkpoint = engine.prepare_stop_checkpoint();
    assert_eq!(
        engine.plan_mode_phase(),
        Some(super::PlanModePhase::Execution)
    );
    let expected_hash = checkpoint
        .frozen_change_evidence_packet
        .as_ref()
        .expect("packet")
        .sha256
        .clone();

    let mut changed_workflow = workflow;
    changed_workflow.settings.plan_mode = None;
    let mut restored = InteractiveEngine::from_checkpoint(changed_workflow, checkpoint, None)
        .expect("restore pinned plan mode");
    assert_eq!(
        restored.plan_mode_phase(),
        Some(super::PlanModePhase::Execution)
    );
    let packets: Vec<Value> = downstream_ids
        .iter()
        .map(|node_id| {
            let request = restored
                .build_request(&NodeId::from(*node_id))
                .expect("downstream request");
            assert_eq!(request.tool_access_policy, ToolAccessPolicy::Execution);
            request.input["change_evidence_packet"].clone()
        })
        .collect();

    assert!(packets.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(packets[0]["sha256"], json!(expected_hash));
    assert_eq!(packets[0]["sourceNodeId"], json!("freeze"));
}

#[test]
fn plan_mode_checkpoint_keeps_the_pinned_source_conversational_before_freeze() {
    let mut workflow = Workflow::new("plan-mode");
    let mut freeze = node("freeze");
    freeze.agent.request_user_input = true;
    workflow.nodes = vec![freeze];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("freeze"),
    });

    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).expect("engine");
    let checkpoint = engine.prepare_stop_checkpoint();
    let mut changed_workflow = workflow;
    changed_workflow.settings.plan_mode = None;
    changed_workflow.nodes[0].agent.request_user_input = false;

    let mut restored = InteractiveEngine::from_checkpoint(changed_workflow, checkpoint, None)
        .expect("restore pinned plan mode");
    let request = restored
        .build_request(&NodeId::from("freeze"))
        .expect("source request");

    assert_eq!(request.tool_access_policy, ToolAccessPolicy::Planning);
    assert!(request.allow_user_input);
}

#[test]
fn checkpoint_rejects_tampered_frozen_change_evidence_packet() {
    let mut workflow = Workflow::new("plan-mode");
    let mut freeze = node("freeze");
    freeze.agent.request_user_input = true;
    workflow.nodes = vec![freeze];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("freeze"),
    });
    let freeze_id = NodeId::from("freeze");
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).expect("engine");
    engine.test_insert_in_flight(freeze_id.clone());
    mark_plan_sealed(&mut engine, &freeze_id);
    engine.on_ai_complete(
        &freeze_id,
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({ "scope": "approved" }),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    );
    let mut checkpoint = engine.prepare_stop_checkpoint();
    checkpoint
        .frozen_change_evidence_packet
        .as_mut()
        .expect("packet")
        .content = json!({ "scope": "tampered" });

    assert!(matches!(
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None),
        Err(CheckpointError::InvalidFrozenChangeEvidencePacket)
    ));
}

#[test]
fn checkpoint_rejects_packet_from_a_node_other_than_the_pinned_source() {
    let mut workflow = Workflow::new("plan-mode");
    let mut freeze = node("freeze");
    freeze.agent.request_user_input = true;
    workflow.nodes = vec![freeze, node("other")];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("freeze"),
    });
    let freeze_id = NodeId::from("freeze");
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).expect("engine");
    engine.test_insert_in_flight(freeze_id.clone());
    mark_plan_sealed(&mut engine, &freeze_id);
    engine.on_ai_complete(
        &freeze_id,
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({ "scope": "approved" }),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    );
    let mut checkpoint = engine.prepare_stop_checkpoint();
    checkpoint
        .frozen_change_evidence_packet
        .as_mut()
        .expect("packet")
        .source_node_id = NodeId::from("other");

    assert!(matches!(
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None),
        Err(CheckpointError::InvalidFrozenChangeEvidencePacket)
    ));
}

#[tokio::test]
async fn plan_mode_denies_hidden_write_mcp_and_subagent_calls_before_tool_port() {
    let mut workflow = Workflow::new("plan-mode-policy");
    let mut planner = node("planner");
    planner.agent.request_user_input = true;
    workflow.nodes = vec![planner];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let ai = ScriptedAi::new(vec![
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: None,
            tool_calls: vec![
                ToolCall::new(
                    "write",
                    "write",
                    json!({ "path": "src/lib.rs", "content": "bad" }),
                ),
                ToolCall::new("mcp", "mcp/example", json!({})),
                ToolCall::new("subagent", "openflow_call_subagent", json!({})),
                ToolCall::new("read", "read", json!({ "path": "README.md" })),
            ],
            reasoning: Vec::new(),
            usage: None,
        })),
        Ok(needs_input("Should I include the optional migration?")),
    ]);
    let batches = Arc::new(Mutex::new(Vec::new()));
    let tools = PlanningRecordingToolPort {
        batches: Arc::clone(&batches),
    };
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");

    assert!(matches!(
        run_once(&mut engine, &ai, &tools).await,
        EngineRunResult::NeedsInteraction { .. }
    ));
    assert_eq!(
        *batches.lock().expect("batches lock"),
        vec![(vec!["read".to_string()], ToolAccessPolicy::Planning,)]
    );
    let transcript = engine.transcript(&NodeId::from("planner"));
    let denied = transcript
        .iter()
        .filter(|item| {
            matches!(
                item,
                AgentTranscriptItem::ToolResult { result }
                    if result.is_error && result.content.contains("blocked by Plan Mode")
            )
        })
        .count();
    assert_eq!(denied, 3);
}

#[test]
fn plan_source_seal_always_requires_explicit_human_approval() {
    let mut workflow = Workflow::new("plan-mode-approval");
    let mut planner = node("planner");
    planner.agent.request_user_input = true;
    planner.agent.tools.approval_mode = Some(ApprovalMode::ReadOnly);
    workflow.nodes = vec![planner];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let planner_id = NodeId::from("planner");
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");

    engine.test_deliver_ai(
        &planner_id,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: None,
            tool_calls: vec![ToolCall::new(
                "seal",
                crate::tools::WRITE_PLAN_ARTIFACT_TOOL,
                json!({ "_i": "Approve and seal the implementation plan" }),
            )],
            reasoning: Vec::new(),
            usage: None,
        })),
    );

    let pending = engine
        .pending_tool_batches
        .values()
        .next()
        .expect("pending plan seal");
    assert!(pending.requires_approval);
    assert_eq!(pending.node_id, planner_id);
    assert_eq!(
        pending.tool_calls[0].name,
        crate::tools::WRITE_PLAN_ARTIFACT_TOOL
    );
}

#[test]
fn plan_source_cannot_complete_before_an_approved_seal_succeeds() {
    let mut workflow = Workflow::new("plan-mode-freeze-gate");
    let mut planner = node("planner");
    planner.agent.request_user_input = true;
    workflow.nodes = vec![planner];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let planner_id = NodeId::from("planner");
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");

    engine.test_deliver_ai(
        &planner_id,
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({ "scope": "not approved" }),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    );

    assert!(engine.frozen_change_evidence_packet.is_none());
    assert!(!engine.outputs.contains_key(&planner_id));
    assert!(matches!(
        engine.transcript(&planner_id).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains(crate::tools::WRITE_PLAN_ARTIFACT_TOOL)
                && content.contains("explicit human approval")
    ));
}

#[test]
fn non_source_node_cannot_mutate_or_seal_the_run_plan() {
    let mut workflow = Workflow::new("plan-mode-owner");
    let mut planner = node("planner");
    planner.agent.request_user_input = true;
    workflow.nodes = vec![planner, node("research")];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let research_id = NodeId::from("research");
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");

    engine.test_deliver_ai(
        &research_id,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: None,
            tool_calls: vec![
                ToolCall::new(
                    "draft",
                    "write",
                    json!({
                        "path": crate::tools::PLAN_DRAFT_PATH,
                        "content": "# Plan\n"
                    }),
                ),
                ToolCall::new("seal", crate::tools::WRITE_PLAN_ARTIFACT_TOOL, json!({})),
            ],
            reasoning: Vec::new(),
            usage: None,
        })),
    );

    assert!(engine.pending_tool_batches.is_empty());
    let denied = engine
        .transcript(&research_id)
        .iter()
        .filter(|item| {
            matches!(
                item,
                AgentTranscriptItem::ToolResult { result }
                    if result.is_error
                        && result.content.contains("selected Plan Mode evidence source")
            )
        })
        .count();
    assert_eq!(denied, 2);
}

#[test]
fn committed_plan_artifact_records_reference_without_plan_payload() {
    let mut workflow = Workflow::new("plan-mode-redaction");
    let mut planner = node("planner");
    planner.agent.request_user_input = true;
    workflow.nodes = vec![planner];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let planner_id = NodeId::from("planner");
    let call = ToolCall::new(
        "plan-call",
        crate::tools::WRITE_PLAN_ARTIFACT_TOOL,
        json!({}),
    );
    let mut engine = InteractiveEngine::new(workflow, None, None).expect("engine");
    engine
        .transcripts
        .entry(planner_id.clone())
        .or_default()
        .push(AgentTranscriptItem::ToolCall { call: call.clone() });
    engine.test_insert_pending_batch(PendingToolBatch {
        approval_id: "plan-batch".to_string(),
        node_id: planner_id.clone(),
        tool_calls: vec![call],
        requires_approval: false,
        tool_access_policy: ToolAccessPolicy::Planning,
    });

    engine
        .on_tool_results(
            &planner_id,
            vec![ToolResult {
                tool_call_id: "plan-call".to_string(),
                tool_name: crate::tools::WRITE_PLAN_ARTIFACT_TOOL.to_string(),
                content: "Plan artifact stored".to_string(),
                is_error: false,
                artifact_ids: vec!["5b6bbf2b-a428-4ca2-8579-207b89d2463a".to_string()],
                output_meta: None,
            }],
        )
        .expect("tool results");

    let AgentTranscriptItem::ToolCall { call } = &engine.transcript(&planner_id)[0] else {
        panic!("expected plan tool call")
    };
    assert_eq!(
        call.arguments["sealed_from"],
        json!(crate::tools::PLAN_DRAFT_PATH)
    );
    assert_eq!(
        call.arguments["artifact_id"],
        json!("5b6bbf2b-a428-4ca2-8579-207b89d2463a")
    );
    assert!(call.arguments.get("markdown").is_none());
}

#[test]
fn checkpoint_preserves_protocol_retry_budgets() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![node("a")];
    let node_a = NodeId::from("a");
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    engine
        .recovery
        .mixed_tool_turn_retries_by_node
        .insert(node_a.clone(), 2);
    engine
        .recovery
        .output_truncation_retries_by_node
        .insert(node_a.clone(), 1);

    let checkpoint = engine.prepare_stop_checkpoint();
    assert_eq!(
        checkpoint.mixed_tool_turn_retries_by_node.get(&node_a),
        Some(&2)
    );
    assert_eq!(
        checkpoint.output_truncation_retries_by_node.get(&node_a),
        Some(&1)
    );

    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).unwrap();
    assert_eq!(
        restored
            .recovery
            .mixed_tool_turn_retries_by_node
            .get(&node_a),
        Some(&2)
    );
    assert_eq!(
        restored
            .recovery
            .output_truncation_retries_by_node
            .get(&node_a),
        Some(&1)
    );
}

#[test]
fn transient_streak_resets_after_successful_turn() {
    let mut workflow = Workflow::new("wf");
    workflow.settings.retry_policy.max_attempts = 2;
    workflow.settings.retry_policy.backoff_ms = 10;
    workflow.nodes = vec![node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    for _ in 0..2 {
        engine.test_deliver_ai(&node_a, Err(AgentError::Transient("blip".into())));
        assert!(!engine.failed_nodes.contains_key(&node_a));
    }

    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![],
            assistant_message: Some("working".to_string()),
            reasoning: vec![],
            usage: None,
        })),
    );

    for _ in 0..2 {
        engine.test_deliver_ai(&node_a, Err(AgentError::Transient("blip".into())));
        assert!(
            !engine.failed_nodes.contains_key(&node_a),
            "streak should have reset after successful turn"
        );
    }
}

#[test]
fn model_attempt_does_not_decrease_after_streak_reset() {
    let mut workflow = Workflow::new("wf");
    workflow.settings.retry_policy.max_attempts = 2;
    workflow.settings.retry_policy.backoff_ms = 10;
    workflow.nodes = vec![node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    for _ in 0..2 {
        engine.test_deliver_ai(&node_a, Err(AgentError::Transient("blip".into())));
    }

    let before = engine.model_attempt_for_node(&node_a);
    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![],
            assistant_message: Some("working".to_string()),
            reasoning: vec![],
            usage: None,
        })),
    );
    engine.test_deliver_ai(&node_a, Err(AgentError::Transient("blip".into())));

    let after = engine.model_attempt_for_node(&node_a);
    assert!(after >= before);
}

#[test]
fn transient_retry_backoff_blocks_only_the_failing_node() {
    let mut workflow = Workflow::new("wf");
    workflow.settings.retry_policy.max_attempts = 3;
    workflow.settings.retry_policy.backoff_ms = 60_000;
    workflow.nodes = vec![node("a"), node("b")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());
    let node_b = NodeId("b".to_string());

    engine.test_deliver_ai(
        &node_a,
        Err(AgentError::Transient("proxy blip".to_string())),
    );

    assert!(engine.test_is_in_retry_backoff(&node_a));
    assert!(!engine.test_is_in_retry_backoff(&node_b));
    let dispatchable = engine.test_gather_ai_node_ids();
    assert!(!dispatchable.contains(&node_a));
    assert!(dispatchable.contains(&node_b));
}

#[test]
fn revert_file_changes_for_batch_removes_only_matching_records() {
    let mut engine = engine_with(vec![node("idea")]);
    let node_id = NodeId("idea".to_string());
    engine.record_file_changes(
        &node_id,
        vec![
            FileChangeRecord {
                path: "a.txt".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: Some("batch-1".to_string()),
                tool_call_id: None,
                tool_name: None,
                diff_artifact_id: None,
                diff_size_bytes: None,
                timestamp_ms: 1,
            },
            FileChangeRecord {
                path: "a.txt".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: Some("batch-2".to_string()),
                tool_call_id: None,
                tool_name: None,
                diff_artifact_id: None,
                diff_size_bytes: None,
                timestamp_ms: 2,
            },
        ],
    );

    engine.revert_file_changes_for_batch("batch-1", &node_id);

    let records = engine.changed_files_by_node.get(&node_id).expect("records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].batch_id.as_deref(), Some("batch-2"));
}

#[test]
fn shared_context_is_appended_to_system_prompt() {
    let mut workflow = Workflow::new("shared");
    workflow.settings.shared_context = "Always follow the style guide.".to_string();
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = CompleteAi::new(json!({"summary": "ok"}));
    let captured = ai.captured.clone();

    block_on(run_once(&mut engine, &ai, &NoopToolPort));

    let request = captured.lock().expect("lock").pop().expect("request");
    assert!(request
        .system_content()
        .contains("--- Workflow context ---"));
    assert!(request
        .system_content()
        .contains("Always follow the style guide."));
}

#[test]
fn auto_start_node_runs_ai_and_completes() {
    let mut engine = engine_with(vec![node("idea")]);
    let ai = CompleteAi::new(json!({"summary": "ok"}));

    let result = block_on(run_once(&mut engine, &ai, &NoopToolPort));
    let EngineRunResult::Completed(report) = result else {
        panic!("expected completed run");
    };
    assert_eq!(report.outputs.len(), 1);
    assert_eq!(report.outputs[0].output, json!({"summary": "ok"}));
}

#[test]
fn legacy_non_auto_start_node_runs_without_initial_input() {
    let mut workflow = Workflow::new("test");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();

    let result = block_on(run_once(
        &mut engine,
        &CompleteAi::new(json!({"summary": "ok"})),
        &NoopToolPort,
    ));
    let EngineRunResult::Completed(report) = result else {
        panic!("expected completed run");
    };
    assert_eq!(report.outputs[0].output, json!({"summary": "ok"}));
}

#[test]
fn model_requested_user_input_resumes_ai_request() {
    let mut engine = engine_with(vec![interactive_node("idea")]);
    let ai = ScriptedAi::new(vec![
        Ok(needs_input("Which launch scope should I use?")),
        Ok(completed(json!({"summary": "ok"}))),
    ]);

    block_on(async {
        let pause = run_once(&mut engine, &ai, &NoopToolPort).await;
        assert!(matches!(pause, EngineRunResult::NeedsInteraction { .. }));
        engine
            .on_human_input(&NodeId::from("idea"), "Need a smaller launch scope")
            .unwrap();
        let done = run_once(&mut engine, &ai, &NoopToolPort).await;
        assert!(matches!(done, EngineRunResult::Completed(_)));
    });

    let request = {
        let captured = ai.captured.lock().expect("lock");
        captured.last().cloned()
    }
    .expect("request");
    assert_eq!(request.node_id, "idea");
    assert_eq!(
        request.transcript,
        vec![
            AgentTranscriptItem::AssistantMessage {
                content: "Which launch scope should I use?".to_string(),
            },
            AgentTranscriptItem::UserMessage {
                content: "Need a smaller launch scope".to_string(),
                attachments: Vec::new(),
            },
        ]
    );
}

#[test]
fn entrypoint_attachments_are_exposed_on_first_request_and_checkpoint() {
    let mut workflow = Workflow::new("entrypoint-attachment");
    workflow.nodes = vec![node("idea")];
    let attachment = image_attachment();
    let mut engine = InteractiveEngine::new_with_entrypoint_attachments(
        workflow.clone(),
        Some("Review this".to_string()),
        vec![attachment.clone()],
        None,
    )
    .unwrap();

    let request = engine.build_request(&NodeId::from("idea")).unwrap();
    assert_eq!(request.entrypoint_attachments, vec![attachment.clone()]);
    assert_eq!(
        request.input["entrypoint"]["attachments"],
        serde_json::to_value([attachment.clone()]).unwrap()
    );
    assert!(request.resolved_attachments.is_empty());

    let checkpoint = engine.prepare_stop_checkpoint();
    let serialized = serde_json::to_value(&checkpoint).unwrap();
    assert_eq!(
        serialized["entrypoint_attachments"],
        serde_json::to_value([attachment.clone()]).unwrap()
    );
    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    let mut restored = restored;
    let request = restored.build_request(&NodeId::from("idea")).unwrap();
    assert_eq!(request.entrypoint_attachments, vec![attachment]);
}

#[test]
fn legacy_checkpoint_defaults_entrypoint_attachments_to_empty() {
    let mut workflow = Workflow::new("legacy-checkpoint");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(
        workflow.clone(),
        Some("Legacy entrypoint".to_string()),
        None,
    )
    .unwrap();
    let checkpoint = engine.prepare_stop_checkpoint();
    let mut serialized = serde_json::to_value(checkpoint).unwrap();
    serialized
        .as_object_mut()
        .expect("checkpoint object")
        .remove("entrypoint_attachments");

    let checkpoint = serde_json::from_value(serialized).expect("legacy checkpoint");
    let mut restored =
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    let request = restored.build_request(&NodeId::from("idea")).unwrap();

    assert!(request.entrypoint_attachments.is_empty());
}

#[test]
fn structured_human_message_survives_history_and_checkpoint() {
    let mut workflow = Workflow::new("interactive-attachment");
    workflow.nodes = vec![interactive_node("idea")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let attachment = image_attachment();
    let ai = ScriptedAi::new(vec![Ok(needs_input("Which attachment should I review?"))]);

    block_on(async {
        let pause = run_once(&mut engine, &ai, &NoopToolPort).await;
        assert!(matches!(pause, EngineRunResult::NeedsInteraction { .. }));
    });
    engine
        .on_human_message(
            &NodeId::from("idea"),
            "Review this",
            vec![attachment.clone()],
        )
        .unwrap();

    assert_eq!(
        engine.conversation_history(&NodeId::from("idea")).last(),
        Some(&crate::conversation::ChatMessage {
            role: ChatRole::User,
            content: "Review this".to_string(),
            id: None,
            streaming: false,
            tool_call_id: None,
            message_kind: None,
            attachments: vec![attachment.clone()],
        })
    );
    let checkpoint = engine.prepare_stop_checkpoint();
    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    assert_eq!(
        restored.transcript(&NodeId::from("idea")).last(),
        Some(&AgentTranscriptItem::UserMessage {
            content: "Review this".to_string(),
            attachments: vec![attachment],
        })
    );
}

#[test]
fn wrong_node_human_input_is_rejected_without_advancing() {
    let mut engine = engine_with(vec![interactive_node("idea")]);
    let ai = ScriptedAi::new(vec![Ok(needs_input("Which node should I update?"))]);

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::NeedsInteraction { .. }
        ));
        let error = engine
            .on_human_input(&NodeId::from("other"), "Wrong node")
            .unwrap_err();
        assert_eq!(
            error,
            EngineInputError::WrongNode {
                expected: NodeId("idea".to_string()),
                got: NodeId("other".to_string())
            }
        );
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::NeedsInteraction { .. }
        ));
    });
    assert!(engine.node_output(&NodeId::from("idea")).is_none());
}

#[test]
fn tool_calls_pause_for_approval_and_resume_after_results() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = ScriptedAi::new(vec![
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall::new(
                "call-1",
                "read",
                json!({"path": "README.md"}),
            )],
            reasoning: vec![],
            usage: None,
        })),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    ]);

    block_on(async {
        let EngineRunResult::NeedsInteraction { approvals, .. } =
            run_once(&mut engine, &ai, &NoopToolPort).await
        else {
            panic!("expected approval pause");
        };
        let approval_id = approvals[0].approval_id.clone();
        engine.on_tool_decision(&approval_id, true, None).unwrap();
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::Completed(_)
        ));
    });
}

#[test]
fn apply_tool_calls_relativizes_absolute_paths_under_project_root() {
    let mut workflow = Workflow::new("tooling");
    workflow.nodes = vec![node("idea")];
    let root = "/Users/philipbotar/Developer/DailyPlanner".to_string();
    let mut engine = InteractiveEngine::new(workflow, None, Some(root.clone())).unwrap();
    let node_id = NodeId("idea".to_string());
    engine.test_deliver_ai(
        &node_id,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall::new(
                "call-1",
                "edit",
                json!({
                    "path": format!("{root}/package.json"),
                }),
            )],
            reasoning: vec![],
            usage: None,
        })),
    );

    let transcript = engine.transcript(&node_id);
    let AgentTranscriptItem::ToolCall { call } = &transcript[0] else {
        panic!("expected tool call in transcript");
    };
    assert_eq!(call.arguments["path"], "package.json");
}

#[test]
fn yolo_mode_skips_tool_approval() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = ScriptedAi::new(vec![
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall::new(
                "call-1",
                "read",
                json!({"path": "README.md"}),
            )],
            reasoning: vec![],
            usage: None,
        })),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    ]);

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::Completed(_)
        ));
    });
}

#[test]
fn runtime_approval_patch_applies_before_tool_decision() {
    // Mid-flight: node still AlwaysAsk; store flipped to Yolo after request build.
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let store = new_runtime_config_store();
    engine.set_runtime_config_store(store.clone());
    let runtime_patch = NodeRuntimeConfigPatch {
        model: None,
        approval_mode: Some(ApprovalMode::Yolo),
        reasoning_effort: None,
        reasoning_budget_tokens: None,
        fast_mode: None,
    };
    upsert_runtime_patch(&store, NodeId("idea".to_string()), &runtime_patch);

    let node_id = NodeId("idea".to_string());
    engine.test_deliver_ai(
        &node_id,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall::new(
                "call-1",
                "bash",
                json!({"command": "echo hi"}),
            )],
            reasoning: vec![],
            usage: None,
        })),
    );

    let ai = ScriptedAi::new(vec![Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
        output: json!({"summary": "ok"}),
        raw_text: "{}".to_string(),
        ..Default::default()
    }))]);
    block_on(async {
        assert!(
            matches!(
                run_once(&mut engine, &ai, &NoopToolPort).await,
                EngineRunResult::Completed(_)
            ),
            "live runtime approval patch must skip approval for next tool batch"
        );
    });
}

#[test]
fn misrouted_completion_is_rejected() {
    let mut engine = engine_with(vec![node("idea")]);
    engine.test_insert_in_flight(NodeId::from("idea"));

    engine.on_ai_complete(
        &NodeId::from("other"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "wrong"}),
            raw_text: "{}".to_string(),
            ..Default::default()
        })),
    );

    let result = block_on(run_once(
        &mut engine,
        &CompleteAi::new(json!({"summary": "nope"})),
        &NoopToolPort,
    ));
    let EngineRunResult::Failed(RunError::NodeFailed { node_id, kind }) = result else {
        panic!("expected failure");
    };
    assert_eq!(node_id, NodeId::from("other"));
    assert_eq!(
        kind,
        NodeFailureKind::MisroutedCompletion(
            "expected model completion for idea, got other".to_string()
        )
    );
}

#[tokio::test]
async fn stale_in_flight_in_parallel_layer_surfaces_needs_interaction() {
    struct NoAi;

    #[async_trait]
    impl AiPort for NoAi {
        async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Failed("should not invoke".to_string()))
        }
    }

    let mut engine = engine_with(vec![node("done"), node("stale")]);
    engine.test_insert_output(NodeId::from("done"), json!({"summary": "ok"}));
    engine.test_insert_in_flight(NodeId::from("stale"));

    let result = engine
        .run(&NoAi, &NoopToolPort, &CancellationToken::new())
        .await;

    let EngineRunResult::NeedsInteraction { retryables, .. } = result else {
        panic!("expected retry interaction, got {result:?}");
    };
    assert!(retryables.iter().any(|r| r.node_id.0 == "stale"));
}

#[test]
fn checkpoint_roundtrip_preserves_awaiting_input_pause() {
    let mut workflow = Workflow::new("pause");
    workflow.nodes = vec![interactive_node("review")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let ai = ScriptedAi::new(vec![Ok(needs_input("Which result should I approve?"))]);

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &ai, &NoopToolPort).await,
            EngineRunResult::NeedsInteraction { .. }
        ));
    });

    let checkpoint = engine.prepare_stop_checkpoint();
    let mut engine =
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &CompleteAi::new(json!({})), &NoopToolPort).await,
            EngineRunResult::NeedsInteraction { .. }
        ));
    });
}

#[test]
fn checkpoint_roundtrip_preserves_node_handoff() {
    let mut workflow = Workflow::new("handoff");
    workflow.nodes = vec![node("research")];
    let node_id = NodeId::from("research");
    let handoff = HandoffArtifact {
        format: HandoffFormat::Markdown,
        uri: "run://handoffs/research/HANDOFF.md".to_string(),
        media_type: "text/markdown".to_string(),
        sha256: "abc123".to_string(),
        size_bytes: 42,
    };
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    engine.test_deliver_ai(
        &node_id,
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: Some(handoff.clone()),
            output: json!({"summary": "done"}),
            raw_text: String::new(),
            ..Default::default()
        })),
    );

    let checkpoint = engine.prepare_stop_checkpoint();
    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).unwrap();

    assert_eq!(restored.node_handoff(&node_id), Some(handoff));
}

#[test]
fn checkpoint_rejects_unknown_node_ids_in_workflow() {
    let mut workflow = Workflow::new("wf-1");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let mut checkpoint = engine.prepare_stop_checkpoint();
    checkpoint
        .outputs
        .insert(NodeId::from("deleted"), json!({"x": 1}));

    match InteractiveEngine::from_checkpoint(workflow, checkpoint, None) {
        Err(CheckpointError::StaleNodeIds { .. }) => {}
        Ok(_) => panic!("expected stale node ids error"),
        Err(other) => panic!("expected stale node ids error, got {other:?}"),
    }
}

#[test]
fn checkpoint_rejects_workflow_id_mismatch() {
    let mut engine = engine_with(vec![node("idea")]);
    let checkpoint = engine.prepare_stop_checkpoint();
    let mut other = Workflow::new("two");
    other.nodes = vec![node("idea")];

    match InteractiveEngine::from_checkpoint(other, checkpoint, None) {
        Err(CheckpointError::WorkflowMismatch { .. }) => {}
        Ok(_) => panic!("expected workflow mismatch"),
        Err(other) => panic!("expected workflow mismatch, got {other:?}"),
    }
}

#[test]
fn checkpoint_rejects_unknown_nodes_in_every_recovery_map() {
    let mut workflow = Workflow::new("wf-1");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let checkpoint = serde_json::to_value(engine.prepare_stop_checkpoint()).unwrap();
    for &field in RecoveryCounters::CHECKPOINT_FIELDS {
        let mut value = checkpoint.clone();
        value[field] = json!({"deleted": 1});
        let restored = InteractiveEngine::from_checkpoint(
            workflow.clone(),
            serde_json::from_value(value).unwrap(),
            None,
        );
        assert!(
            matches!(restored, Err(CheckpointError::StaleNodeIds { missing }) if missing == vec![NodeId::from("deleted")]),
            "{field} must reject unknown nodes"
        );
    }
}

struct BarrierAi {
    barrier: Arc<tokio::sync::Barrier>,
}

#[async_trait]
impl AiPort for BarrierAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        // Completes only when both nodes' invocations are in flight simultaneously.
        self.barrier.wait().await;
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({ "node": request.node_id.0 }),
            raw_text: String::new(),
            ..Default::default()
        }))
    }
}

#[test]
fn same_layer_nodes_invoke_ai_concurrently() {
    block_on(async {
        let mut workflow = Workflow::new("wf");
        workflow.nodes.push(node("a"));
        workflow.nodes.push(node("b"));
        let mut engine = InteractiveEngine::new(workflow, Some("go".to_string()), None).unwrap();
        let ai = BarrierAi {
            barrier: Arc::new(tokio::sync::Barrier::new(2)),
        };
        let cancel = CancellationToken::new();
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            engine.run(&ai, &NoopToolPort, &cancel),
        )
        .await
        .expect("serial engine would deadlock on the barrier");
        assert!(matches!(result, EngineRunResult::Completed(_)));
    });
}

struct ToolOverlapAi {
    barrier: Arc<tokio::sync::Barrier>,
}

#[async_trait]
impl AiPort for ToolOverlapAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        if request.node_id.0 == "b" {
            // Held open until node a's tool batch reaches the same barrier.
            self.barrier.wait().await;
            return Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({ "node": "b" }),
                raw_text: String::new(),
                ..Default::default()
            }));
        }
        let has_tool_result = request
            .transcript
            .iter()
            .any(|item| matches!(item, AgentTranscriptItem::ToolResult { .. }));
        if has_tool_result {
            return Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({ "node": "a" }),
                raw_text: String::new(),
                ..Default::default()
            }));
        }
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: None,
            reasoning: vec![],
            usage: None,
            tool_calls: vec![ToolCall::new("t1", "bash", json!({}))],
        }))
    }
}

struct BarrierToolPort {
    barrier: Arc<tokio::sync::Barrier>,
}

#[async_trait]
impl ToolPort for BarrierToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
        _policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        self.barrier.wait().await;
        ToolBatchOutput {
            results: calls
                .into_iter()
                .map(|call| ToolResult {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: "ok".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                })
                .collect(),
            effects: ToolBatchEffects::default(),
        }
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
}

#[test]
fn tool_batch_runs_while_other_node_ai_is_in_flight() {
    block_on(async {
        let mut workflow = Workflow::new("wf");
        let mut a = node("a");
        a.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
        workflow.nodes.push(a);
        workflow.nodes.push(node("b"));
        let mut engine = InteractiveEngine::new(workflow, Some("go".to_string()), None).unwrap();
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let ai = ToolOverlapAi {
            barrier: Arc::clone(&barrier),
        };
        let tools = BarrierToolPort { barrier };
        let cancel = CancellationToken::new();
        let result = tokio::time::timeout(Duration::from_secs(5), engine.run(&ai, &tools, &cancel))
            .await
            .expect("tool batch and AI call must overlap");
        assert!(matches!(result, EngineRunResult::Completed(_)));
    });
}

struct EffectsToolPort;

#[async_trait]
impl ToolPort for EffectsToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
        _policy: ToolAccessPolicy,
    ) -> ToolBatchOutput {
        ToolBatchOutput {
            results: calls
                .into_iter()
                .map(|call| ToolResult {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: "ok".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                })
                .collect(),
            effects: ToolBatchEffects {
                file_changes: vec![FileChangeRecord {
                    path: "src/x.rs".to_string(),
                    op: crate::tools::FileChangeOp::Update,
                    rename_to: None,
                    diff_summary: None,
                    batch_id: None,
                    tool_call_id: None,
                    tool_name: None,
                    diff_artifact_id: None,
                    diff_size_bytes: None,
                    timestamp_ms: 1,
                }],
                reads: Vec::new(),
                read_call_paths: vec!["src/x.rs".to_string()],
                interrupted: false,
            },
        }
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
}

#[test]
fn tool_batch_effects_are_recorded_on_engine() {
    block_on(async {
        let mut workflow = Workflow::new("wf");
        let mut a = node("a");
        a.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
        workflow.nodes.push(a);
        let mut engine = InteractiveEngine::new(workflow, Some("go".to_string()), None).unwrap();
        let ai = ScriptedAi::new(vec![
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                reasoning: vec![],
                usage: None,
                tool_calls: vec![ToolCall::new(
                    "t1",
                    "write",
                    json!({
                        "path": "src/x.rs",
                        "content": "fn x() {}\n"
                    }),
                )],
            })),
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({ "ok": true }),
                raw_text: String::new(),
                ..Default::default()
            })),
        ]);
        let cancel = CancellationToken::new();
        let result = engine.run(&ai, &EffectsToolPort, &cancel).await;
        assert!(matches!(result, EngineRunResult::Completed(_)));
        let checkpoint = engine.prepare_stop_checkpoint();
        let changes = checkpoint
            .changed_files_by_node
            .get(&NodeId("a".to_string()))
            .expect("file change recorded");
        assert_eq!(changes[0].path, "src/x.rs");
    });
}

#[test]
fn checkpoint_stop_mid_tool_execution_marks_node_retryable() {
    let mut engine = engine_with(vec![node("idea")]);
    let node_id = NodeId::from("idea");

    engine.test_insert_pending_batch(PendingToolBatch {
        approval_id: "batch-1".to_string(),
        node_id: node_id.clone(),
        tool_calls: vec![ToolCall::new("call-1", "read", json!({"path": "foo.txt"}))],
        requires_approval: false,
        tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
    });
    assert_eq!(engine.take_ready_tool_batches().len(), 1);

    let checkpoint = engine.prepare_stop_checkpoint();
    assert!(!checkpoint.pending_tool_batches.contains_key("batch-1"));
    assert!(checkpoint.interrupted_nodes.contains(&node_id));
    assert!(checkpoint.transcripts[&node_id].iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::ToolResult { result }
                if result.is_error && result.content.contains("interrupted or cancelled")
        )
    }));
}

fn autonomous_node(id: &str) -> Node {
    let mut node = node(id);
    node.agent.request_user_input = false;
    node
}

fn interactive_node(id: &str) -> Node {
    let mut node = node(id);
    node.agent.request_user_input = true;
    node
}

fn conversation_node(id: &str) -> Node {
    let mut node = interactive_node(id);
    node.agent.conversation_mode = true;
    node
}

fn needs_input(message: &str) -> AgentTurnOutcome {
    AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
        raw_text: message.to_string(),
        assistant_message: message.to_string(),
        structured_input: None,
        reasoning: Vec::new(),
    })
}

fn completed(output: Value) -> AgentTurnOutcome {
    AgentTurnOutcome::Completed(AgentTurnSuccess {
        handoff: None,
        output,
        raw_text: "{}".to_string(),
        assistant_message: None,
        reasoning: Vec::new(),
        usage: None,
    })
}

fn message(message: &str) -> AgentTurnOutcome {
    AgentTurnOutcome::Message(AgentMessageTurn {
        raw_text: message.to_string(),
        assistant_message: message.to_string(),
        reasoning: Vec::new(),
        usage: None,
    })
}

#[test]
fn empty_provider_turn_retries_with_tool_nudge_before_failing() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());
    let empty = || {
        Err(AgentError::Failed(
            "provider returned neither tool calls nor recoverable output".to_string(),
        ))
    };

    for attempt in 1..=3 {
        engine.test_deliver_ai(&node_a, empty());
        assert!(
            !engine.failed_nodes.contains_key(&node_a),
            "attempt {attempt} should nudge, not fail"
        );
        assert_eq!(engine.transcript(&node_a).len(), attempt);
    }
    assert!(matches!(
        engine.transcript(&node_a).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("No human input is available")
    ));

    engine.test_deliver_ai(&node_a, empty());
    assert!(engine.failed_nodes.contains_key(&node_a));
}

#[test]
fn interactive_empty_provider_turn_keeps_human_input_available() {
    let mut engine = engine_with(vec![interactive_node("grill")]);
    let grill = NodeId("grill".to_string());

    engine.test_deliver_ai(
        &grill,
        Err(AgentError::Failed(
            "provider returned neither tool calls nor recoverable output".to_string(),
        )),
    );

    let transcript = engine.transcript(&grill);
    assert!(matches!(
        transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("openflow_request_user_input")
                && !content.contains("No human input is available")
    ));
}

#[test]
fn interactive_empty_provider_turn_pauses_for_human_after_retry_cap() {
    let mut engine = engine_with(vec![interactive_node("grill")]);
    let grill = NodeId("grill".to_string());
    let empty = || {
        Err(AgentError::Failed(
            "Custom OpenAI-compatible API model `minimax-m3` returned no tool calls and no usable text."
                .to_string(),
        ))
    };

    for _ in 0..3 {
        engine.test_deliver_ai(&grill, empty());
        assert!(!engine.failed_nodes.contains_key(&grill));
        assert!(!engine.awaiting_nodes.contains(&grill));
    }

    engine.test_deliver_ai(&grill, empty());
    assert!(
        !engine.failed_nodes.contains_key(&grill),
        "conversational nodes must pause, not fail, after empty-turn retries"
    );
    assert!(engine.awaiting_nodes.contains(&grill));
    assert!(matches!(
        engine.transcript(&grill).last(),
        Some(AgentTranscriptItem::AssistantMessage { content })
            if content.contains("empty turns")
    ));
}

#[test]
fn empty_after_narrated_write_uses_write_specific_nudge() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    engine.test_deliver_ai(
        &brief,
        Ok(message(
            "Writing the brief to `docs/feature-briefs/002.md` now.",
        )),
    );
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("narrated creating or updating a file")
    ));

    engine.test_deliver_ai(
        &brief,
        Err(AgentError::Failed(
            "provider returned neither tool calls nor recoverable output".to_string(),
        )),
    );
    assert!(!engine.failed_nodes.contains_key(&brief));
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("already intended to write a file")
                && content.contains("write tool call")
    ));
}

#[test]
fn incomplete_write_is_rejected_with_chunked_write_nudge() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    engine
        .recovery
        .auto_continue_streaks_by_node
        .insert(node_a.clone(), 5);
    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall::new(
                "w1",
                "write",
                json!({ "path": "docs/feature-briefs/002.md" }),
            )],
            assistant_message: None,
            reasoning: vec![],
            usage: None,
        })),
    );

    assert!(engine.pending_tool_batches.is_empty());
    assert!(!engine
        .recovery
        .auto_continue_streaks_by_node
        .contains_key(&node_a));
    let transcript = engine.transcript(&node_a);
    assert!(transcript.iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::ToolResult { result }
                if result.is_error && result.content.contains("missing field `content`")
        )
    }));
    assert!(matches!(
        transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("missing required `content`")
                && content.contains("small stub")
    ));
}

#[test]
fn write_with_empty_content_is_allowed_through_to_pending() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall::new(
                "w1",
                "write",
                json!({ "path": "docs/stub.md", "content": "" }),
            )],
            assistant_message: None,
            reasoning: vec![],
            usage: None,
        })),
    );

    assert_eq!(engine.pending_tool_batches.len(), 1);
    assert!(!engine.transcript(&node_a).iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::UserMessage { content, .. }
                if content.contains("missing required `content`")
        )
    }));
}

#[test]
fn autonomous_node_auto_continues_instead_of_awaiting_input() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(&node_a, Ok(needs_input("Now I'll write the tests.")));

    assert!(!engine.awaiting_nodes.contains(&node_a), "must not pause");
    assert!(!engine.failed_nodes.contains_key(&node_a));
    let transcript = engine.transcript(&node_a);
    assert!(matches!(
        transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("Call write immediately")
    ));
    assert!(matches!(
        &transcript[transcript.len() - 2],
        AgentTranscriptItem::AssistantMessage { content }
            if content == "Now I'll write the tests."
    ));
}

#[test]
fn autonomous_node_repairs_plain_message_instead_of_awaiting_input() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(&node_a, Ok(message("I will inspect the repository next.")));

    assert!(!engine.awaiting_nodes.contains(&node_a));
    assert!(!engine.failed_nodes.contains_key(&node_a));
    assert!(matches!(
        engine.transcript(&node_a).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("No human input is available")
    ));
}

#[test]
fn autonomous_node_fails_after_consecutive_auto_continue_cap() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    for _ in 0..10 {
        engine.test_deliver_ai(&node_a, Ok(needs_input("still narrating")));
        assert!(!engine.failed_nodes.contains_key(&node_a));
    }
    engine.test_deliver_ai(&node_a, Ok(needs_input("still narrating")));
    assert!(engine.failed_nodes.contains_key(&node_a));
    assert!(!engine.awaiting_nodes.contains(&node_a));
}

#[test]
fn autonomous_auto_continue_streak_resets_on_tool_calls() {
    let mut engine = engine_with(vec![autonomous_node("a")]);
    let node_a = NodeId("a".to_string());

    for _ in 0..9 {
        engine.test_deliver_ai(&node_a, Ok(needs_input("narration")));
    }
    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall::new("c1", "read", json!({}))],
            assistant_message: None,
            reasoning: vec![],
            usage: None,
        })),
    );
    for _ in 0..10 {
        engine.test_deliver_ai(&node_a, Ok(needs_input("narration")));
        assert!(
            !engine.failed_nodes.contains_key(&node_a),
            "streak should have reset after tool-call progress"
        );
    }
}

#[test]
fn interactive_node_still_pauses_for_real_questions() {
    let mut engine = engine_with(vec![interactive_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(
        &node_a,
        Ok(needs_input("Which environment should I target?")),
    );
    assert!(engine.awaiting_nodes.contains(&node_a));
}

#[test]
fn interactive_node_accepts_a_complete_response_without_a_forced_question() {
    let mut engine = engine_with(vec![interactive_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(
        &node_a,
        Ok(needs_input(
            "The list now includes the requested item and remains sorted.",
        )),
    );

    assert!(
        engine.awaiting_nodes.contains(&node_a),
        "an explicit input-request tool call must end a conversational turn without requiring a question"
    );
}

#[test]
fn direct_conversation_plain_reply_ends_the_turn() {
    let mut engine = engine_with(vec![conversation_node("assistant")]);
    let node_id = NodeId::from("assistant");

    engine.test_deliver_ai(
        &node_id,
        Ok(message(
            "The list now includes the requested item and remains sorted.",
        )),
    );

    assert!(engine.awaiting_nodes.contains(&node_id));
    assert!(matches!(
        engine.transcript(&node_id).last(),
        Some(AgentTranscriptItem::AssistantMessage { content })
            if content == "The list now includes the requested item and remains sorted."
    ));
    assert!(
        engine.transcript(&node_id).iter().all(|item| !matches!(
            item,
            AgentTranscriptItem::UserMessage { content, .. }
                if content.contains("openflow_request_user_input")
        )),
        "a natural direct-chat reply must not receive a harness-tool nudge"
    );
}

#[test]
fn structured_question_pauses_and_survives_checkpoint_restore() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![interactive_node("a")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let node_a = NodeId("a".to_string());
    let structured_input = StructuredUserInput {
        questions: vec![UserInputQuestion {
            id: "target_env".to_string(),
            header: "Target".to_string(),
            question: "Which environment should I target?".to_string(),
            selection_mode: UserInputSelectionMode::Single,
            options: vec![
                UserInputOption {
                    label: "Staging".to_string(),
                    description: "Use the shared staging environment.".to_string(),
                },
                UserInputOption {
                    label: "Production".to_string(),
                    description: "Use the live production environment.".to_string(),
                },
            ],
        }],
    };

    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: "Choose a deployment target.".to_string(),
            structured_input: Some(structured_input.clone()),
            reasoning: Vec::new(),
        })),
    );

    assert!(engine.awaiting_nodes.contains(&node_a));
    assert_eq!(
        engine.gather_await_inputs()[0].structured_input,
        Some(structured_input.clone())
    );

    let checkpoint = engine.prepare_stop_checkpoint();
    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).unwrap();
    assert_eq!(
        restored.gather_await_inputs()[0].structured_input,
        Some(structured_input)
    );
}

#[test]
fn structured_question_is_not_projected_when_disabled_for_the_node() {
    let mut workflow = Workflow::new("direct chat");
    let mut assistant = interactive_node("assistant");
    assistant.agent.tools.allow_structured_user_input = false;
    workflow.nodes = vec![assistant];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_id = NodeId::from("assistant");

    engine.test_deliver_ai(
        &node_id,
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: "Yo! What can I help you with?".to_string(),
            structured_input: Some(StructuredUserInput {
                questions: vec![UserInputQuestion {
                    id: "continue_chat".to_string(),
                    header: "Continue".to_string(),
                    question: "What would you like to do?".to_string(),
                    selection_mode: UserInputSelectionMode::Single,
                    options: vec![
                        UserInputOption {
                            label: "Ask something".to_string(),
                            description: "Send a question or task.".to_string(),
                        },
                        UserInputOption {
                            label: "Just chat".to_string(),
                            description: "Have a casual conversation.".to_string(),
                        },
                    ],
                }],
            }),
            reasoning: Vec::new(),
        })),
    );

    assert!(engine.awaiting_nodes.contains(&node_id));
    assert_eq!(engine.gather_await_inputs()[0].structured_input, None);
}

#[test]
fn structured_only_request_retries_with_free_text_feedback_when_disabled() {
    let mut workflow = Workflow::new("direct chat");
    let mut assistant = interactive_node("assistant");
    assistant.agent.tools.allow_structured_user_input = false;
    workflow.nodes = vec![assistant];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_id = NodeId::from("assistant");

    engine.test_deliver_ai(
        &node_id,
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: String::new(),
            structured_input: Some(StructuredUserInput {
                questions: vec![UserInputQuestion {
                    id: "continue_chat".to_string(),
                    header: "Continue".to_string(),
                    question: "What would you like to do?".to_string(),
                    selection_mode: UserInputSelectionMode::Single,
                    options: vec![
                        UserInputOption {
                            label: "Ask something".to_string(),
                            description: "Send a question or task.".to_string(),
                        },
                        UserInputOption {
                            label: "Just chat".to_string(),
                            description: "Have a casual conversation.".to_string(),
                        },
                    ],
                }],
            }),
            reasoning: Vec::new(),
        })),
    );

    assert!(!engine.awaiting_nodes.contains(&node_id));
    assert!(matches!(
        engine.transcript(&node_id).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("Structured questions are not available")
                && content.contains("one direct human-facing question")
    ));
}

#[test]
fn invalid_structured_question_retries_even_when_prompt_looks_direct() {
    let mut engine = engine_with(vec![interactive_node("a")]);
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: "Which environment should I target?".to_string(),
            structured_input: Some(StructuredUserInput {
                questions: vec![UserInputQuestion {
                    id: "target_env".to_string(),
                    header: "Header longer than twelve".to_string(),
                    question: "Which environment should I target?".to_string(),
                    selection_mode: UserInputSelectionMode::Single,
                    options: Vec::new(),
                }],
            }),
            reasoning: Vec::new(),
        })),
    );

    assert!(!engine.awaiting_nodes.contains(&node_a));
    assert!(matches!(
        engine.transcript(&node_a).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("structured questions")
    ));
}

#[test]
fn conversational_node_auto_continues_plain_message_without_pausing() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    engine.test_deliver_ai(
        &brief,
        Ok(message("Auth and Postgres are in scope. Storage can wait.")),
    );

    assert!(
        !engine.awaiting_nodes.contains(&brief),
        "plain text must not pause; only openflow_request_user_input pauses"
    );
    assert!(!engine.failed_nodes.contains_key(&brief));
    let transcript = engine.transcript(&brief);
    assert!(matches!(
        transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("openflow_request_user_input")
                && !content.contains("No human input is available")
                && !content.contains("narrated creating or updating")
    ));
    assert!(matches!(
        &transcript[transcript.len() - 2],
        AgentTranscriptItem::AssistantMessage { content }
            if content == "Auth and Postgres are in scope. Storage can wait."
    ));
}

#[test]
fn narrated_write_without_tool_gets_write_specific_nudge() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    engine.test_deliver_ai(
        &brief,
        Ok(message(
            "Writing the brief to `docs/feature-briefs/002.md` now.",
        )),
    );

    assert!(!engine.awaiting_nodes.contains(&brief));
    assert!(!engine.failed_nodes.contains_key(&brief));
    assert_eq!(
        engine.recovery.auto_continue_streaks_by_node.get(&brief),
        Some(&1)
    );
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("narrated creating or updating a file")
                && content.contains("Call write immediately")
    ));
}

#[test]
fn plan_mode_narrated_write_nudge_uses_run_local_draft_and_seal_tool() {
    let mut workflow = Workflow::new("plan-mode");
    workflow.nodes = vec![interactive_node("planner")];
    workflow.settings.plan_mode = Some(PlanModeConfig {
        evidence_source_node_id: NodeId::from("planner"),
    });
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let planner = NodeId::from("planner");

    engine.test_deliver_ai(
        &planner,
        Ok(message("Writing the complete consolidated plan now.")),
    );

    assert!(matches!(
        engine.transcript(&planner).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains(crate::tools::PLAN_DRAFT_PATH)
                && content.contains(crate::tools::WRITE_PLAN_ARTIFACT_TOOL)
                && content.contains("call write")
    ));
}

#[test]
fn narrated_write_after_successful_write_nudges_edit_chunks() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    engine.transcripts.insert(
        brief.clone(),
        vec![
            AgentTranscriptItem::ToolCall {
                call: ToolCall::new(
                    "w1",
                    "write",
                    json!({
                        "path": "docs/feature-briefs/002.md",
                        "content": "# Stub\n"
                    }),
                ),
            },
            AgentTranscriptItem::ToolResult {
                result: ToolResult {
                    tool_call_id: "w1".to_string(),
                    tool_name: "write".to_string(),
                    content: "Updated docs/feature-briefs/002.md".to_string(),
                    is_error: false,
                    artifact_ids: vec![],
                    output_meta: None,
                },
            },
        ],
    );

    engine.test_deliver_ai(
        &brief,
        Ok(message(
            "Writing the full canonical feature brief in one write.",
        )),
    );

    assert!(!engine.failed_nodes.contains_key(&brief));
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("A write already succeeded")
                && content.contains("Call edit")
    ));
}

#[test]
fn conversational_node_pauses_after_text_only_auto_continue_cap() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    for _ in 0..10 {
        engine.test_deliver_ai(&brief, Ok(message("still narrating")));
        assert!(!engine.failed_nodes.contains_key(&brief));
        assert!(!engine.awaiting_nodes.contains(&brief));
    }
    engine.test_deliver_ai(&brief, Ok(message("still narrating")));
    assert!(
        !engine.failed_nodes.contains_key(&brief),
        "conversational nodes must pause, not fail, after text-only streak"
    );
    assert!(engine.awaiting_nodes.contains(&brief));
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::AssistantMessage { content })
            if content.contains("kept narrating")
    ));
}

#[test]
fn valid_conversational_pause_resets_protocol_recovery_budgets() {
    let mut engine = engine_with(vec![interactive_node("brief")]);
    let brief = NodeId("brief".to_string());

    engine
        .recovery
        .empty_turn_retries_by_node
        .insert(brief.clone(), 2);
    engine
        .recovery
        .request_input_retries_by_node
        .insert(brief.clone(), 2);
    engine
        .recovery
        .submit_output_retries_by_node
        .insert(brief.clone(), 2);
    engine
        .recovery
        .auto_continue_streaks_by_node
        .insert(brief.clone(), 2);

    engine.test_deliver_ai(
        &brief,
        Ok(needs_input("Which Supabase products should be included?")),
    );

    assert!(engine.awaiting_nodes.contains(&brief));
    assert!(!engine
        .recovery
        .empty_turn_retries_by_node
        .contains_key(&brief));
    assert!(!engine
        .recovery
        .request_input_retries_by_node
        .contains_key(&brief));
    assert!(!engine
        .recovery
        .submit_output_retries_by_node
        .contains_key(&brief));
    assert!(!engine
        .recovery
        .auto_continue_streaks_by_node
        .contains_key(&brief));

    engine
        .on_human_input(&brief, "Auth and Postgres. What else do you think?")
        .unwrap();
    engine.test_deliver_ai(
        &brief,
        Ok(message("Storage is the most useful third surface.")),
    );

    assert!(
        !engine.awaiting_nodes.contains(&brief),
        "plain follow-up narration must auto-continue, not pause"
    );
    assert!(!engine.failed_nodes.contains_key(&brief));
    assert!(matches!(
        engine.transcript(&brief).last(),
        Some(AgentTranscriptItem::UserMessage { content, .. })
            if content.contains("openflow_request_user_input")
    ));
}

#[test]
fn malformed_request_input_fails_after_retry_cap_without_pausing() {
    let mut engine = engine_with(vec![interactive_node("a")]);
    let node_a = NodeId("a".to_string());

    for _ in 0..=MAX_MALFORMED_REQUEST_INPUT_RETRIES {
        engine.test_deliver_ai(&node_a, Ok(needs_input("")));
    }

    assert!(!engine.awaiting_nodes.contains(&node_a));
    let error = engine
        .failed_nodes
        .get(&node_a)
        .expect("malformed request input should fail the node");
    assert!(error.contains("invalid human-input requests"), "{error}");
}

#[test]
fn malformed_request_input_retries_reset_on_tool_call_progress() {
    let mut engine = engine_with(vec![interactive_node("a")]);
    let node_a = NodeId("a".to_string());

    for _ in 0..3 {
        engine.test_deliver_ai(&node_a, Ok(needs_input("")));
        assert!(!engine.awaiting_nodes.contains(&node_a));
    }

    engine.test_deliver_ai(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall::new("c1", "read", json!({}))],
            assistant_message: None,
            reasoning: vec![],
            usage: None,
        })),
    );

    engine.test_deliver_ai(&node_a, Ok(needs_input("")));
    assert!(
        !engine.awaiting_nodes.contains(&node_a),
        "invalid input after progress must retry instead of pausing"
    );
}

#[test]
fn checkpoint_roundtrips_auto_continue_streaks() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let node_a = NodeId("a".to_string());

    engine.test_deliver_ai(&node_a, Ok(needs_input("narration")));

    let checkpoint = engine.prepare_stop_checkpoint();
    let mut value = serde_json::to_value(&checkpoint).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("auto_continue_streaks_by_node");
    let legacy: super::checkpoint::InteractiveEngineCheckpoint =
        serde_json::from_value(value).unwrap();
    assert!(legacy.auto_continue_streaks_by_node.is_empty());

    let restored = InteractiveEngine::from_checkpoint(workflow, checkpoint, None).unwrap();
    assert_eq!(
        restored.recovery.auto_continue_streaks_by_node.get(&node_a),
        Some(&1)
    );
}
