#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "engine tests use unwrap/expect and panic for concise failure messages"
)]

use super::{
    checkpoint::CheckpointError, EngineInputError, EngineRunResult, InteractiveEngine,
    PendingToolBatch, RunError,
};
use crate::conversation::AgentTranscriptItem;
use crate::execution::NodeFailureKind;
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ToolBatchEffects, ToolBatchOutput, ToolPort,
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

struct NoopToolPort;

#[async_trait]
impl ToolPort for NoopToolPort {
    async fn execute_batch(
        &self,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
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

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn mark_node_interrupted_closes_dangling_tool_calls() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes.push(node("a"));
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "bash".to_string(),
        arguments: json!({}),
    };
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
    let mut workflow = Workflow::new("wf");
    workflow.nodes.push(node("a"));
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
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

async fn run_once<A: AiPort, T: ToolPort>(
    engine: &mut InteractiveEngine,
    ai: &A,
    tools: &T,
) -> EngineRunResult {
    engine.run(ai, tools, &CancellationToken::new()).await
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
            assistant_message: None,
            usage: None,
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

#[tokio::test(start_paused = true)]
async fn run_sleeps_backoff_before_transient_retry() {
    struct TransientOnceAi {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AiPort for TransientOnceAi {
        async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(AgentError::Transient("timeout".to_string()));
            }
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
                usage: None,
            }))
        }
    }

    let mut workflow = Workflow::new("backoff");
    workflow.settings.retry_policy.max_attempts = 1;
    workflow.settings.retry_policy.backoff_ms = 500;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = TransientOnceAi {
        calls: Arc::new(AtomicUsize::new(0)),
    };
    let calls = ai.calls.clone();
    let cancel = CancellationToken::new();
    let started = tokio::time::Instant::now();

    let result = engine.run(&ai, &NoopToolPort, &cancel).await;

    assert!(matches!(result, EngineRunResult::Completed(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(started.elapsed() >= Duration::from_millis(500));
}

#[tokio::test(start_paused = true)]
async fn run_cancel_during_backoff_returns_cancelled() {
    struct AlwaysTransientAi;

    #[async_trait]
    impl AiPort for AlwaysTransientAi {
        async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Transient("timeout".to_string()))
        }
    }

    let mut workflow = Workflow::new("cancel");
    workflow.settings.retry_policy.max_attempts = 3;
    workflow.settings.retry_policy.backoff_ms = 5_000;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let cancel = CancellationToken::new();
    let cancel_handle = cancel.clone();
    let started = tokio::time::Instant::now();
    let handle =
        tokio::spawn(async move { engine.run(&AlwaysTransientAi, &NoopToolPort, &cancel).await });
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
    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Err(AgentError::Transient("blip".into())));

    let checkpoint = engine.prepare_stop_checkpoint();
    assert_eq!(checkpoint.transient_streaks_by_node.get(&node_a), Some(&1));

    let mut restored =
        InteractiveEngine::from_checkpoint(workflow, checkpoint, None).expect("restore");
    let again = restored.prepare_stop_checkpoint();
    assert_eq!(again.transient_streaks_by_node.get(&node_a), Some(&1));
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
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Err(AgentError::Transient("blip".into())));
        assert!(!engine.failed_nodes.contains_key(&node_a));
    }

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![],
            assistant_message: Some("working".to_string()),
            usage: None,
        })),
    );

    for _ in 0..2 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Err(AgentError::Transient("blip".into())));
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
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Err(AgentError::Transient("blip".into())));
    }

    let before = engine.model_attempt_for_node(&node_a);
    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![],
            assistant_message: Some("working".to_string()),
            usage: None,
        })),
    );
    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Err(AgentError::Transient("blip".into())));

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

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
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
    let mut workflow = Workflow::new("revert");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
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
                timestamp_ms: 1,
            },
            FileChangeRecord {
                path: "a.txt".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: Some("batch-2".to_string()),
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
    let mut workflow = Workflow::new("test");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = CompleteAi::new(json!({"summary": "ok"}));

    let result = block_on(run_once(&mut engine, &ai, &NoopToolPort));
    let EngineRunResult::Completed(report) = result else {
        panic!("expected completed run");
    };
    assert_eq!(report.outputs.len(), 1);
    assert_eq!(report.outputs[0].output, json!({"summary": "ok"}));
}

#[test]
fn non_auto_start_node_pauses_awaiting_input() {
    let mut workflow = Workflow::new("test");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();

    let result = block_on(run_once(
        &mut engine,
        &CompleteAi::new(json!({})),
        &NoopToolPort,
    ));
    match result {
        EngineRunResult::NeedsInteraction { inputs, .. } => {
            assert_eq!(inputs.len(), 1);
            assert_eq!(inputs[0].node_id, NodeId::from("idea"));
            assert!(inputs[0].is_initial);
        }
        other => panic!("expected pause, got {other:?}"),
    }
}

#[test]
fn manual_node_user_input_starts_ai_request() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let ai = CompleteAi::new(json!({"summary": "ok"}));

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
        vec![AgentTranscriptItem::UserMessage {
            content: "Need a smaller launch scope".to_string(),
        }]
    );
}

#[test]
fn wrong_node_human_input_is_rejected_without_advancing() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &CompleteAi::new(json!({})), &NoopToolPort).await,
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
            run_once(&mut engine, &CompleteAi::new(json!({})), &NoopToolPort).await,
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
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
            }],
            usage: None,
        })),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
            usage: None,
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
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
            }],
            usage: None,
        })),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
            usage: None,
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
fn misrouted_completion_is_rejected() {
    let mut workflow = Workflow::new("misroute");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    engine.test_insert_in_flight(NodeId::from("idea"));

    engine.on_ai_complete(
        &NodeId::from("other"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "wrong"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
            usage: None,
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

    let mut workflow = Workflow::new("stale-run");
    workflow.nodes = vec![node("done"), node("stale")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
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
    let mut manual = node("review");
    manual.agent.auto_start = false;
    workflow.nodes = vec![manual];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();

    block_on(async {
        assert!(matches!(
            run_once(&mut engine, &CompleteAi::new(json!({})), &NoopToolPort).await,
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
    let mut workflow = Workflow::new("one");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let checkpoint = engine.prepare_stop_checkpoint();
    let mut other = Workflow::new("two");
    other.nodes = vec![node("idea")];

    match InteractiveEngine::from_checkpoint(other, checkpoint, None) {
        Err(CheckpointError::WorkflowMismatch { .. }) => {}
        Ok(_) => panic!("expected workflow mismatch"),
        Err(other) => panic!("expected workflow mismatch, got {other:?}"),
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
            assistant_message: None,
            usage: None,
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
                assistant_message: None,
                usage: None,
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
                assistant_message: None,
                usage: None,
            }));
        }
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: None,
            usage: None,
            tool_calls: vec![ToolCall {
                id: "t1".to_string(),
                name: "bash".to_string(),
                arguments: json!({}),
            }],
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
                usage: None,
                tool_calls: vec![ToolCall {
                    id: "t1".to_string(),
                    name: "write".to_string(),
                    arguments: json!({}),
                }],
            })),
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({ "ok": true }),
                raw_text: String::new(),
                assistant_message: None,
                usage: None,
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
fn checkpoint_stop_mid_tool_execution_retains_approved_batch() {
    let mut workflow = Workflow::new("mid-tool");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();

    engine.test_insert_pending_batch(PendingToolBatch {
        approval_id: "batch-1".to_string(),
        node_id: NodeId::from("idea"),
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "read".to_string(),
            arguments: json!({"path": "foo.txt"}),
        }],
        requires_approval: false,
    });
    engine.test_insert_in_flight(NodeId::from("idea"));

    let checkpoint = engine.prepare_stop_checkpoint();
    assert!(checkpoint.pending_tool_batches.contains_key("batch-1"));
}

fn autonomous_node(id: &str) -> Node {
    let mut node = node(id);
    node.agent.request_user_input = false;
    node
}

fn needs_input(message: &str) -> AgentTurnOutcome {
    AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
        raw_text: message.to_string(),
        assistant_message: message.to_string(),
    })
}

#[test]
fn empty_provider_turn_retries_with_tool_nudge_before_failing() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());
    let empty = || {
        Err(AgentError::Failed(
            "provider returned neither tool calls nor recoverable output".to_string(),
        ))
    };

    for attempt in 1..=3 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, empty());
        assert!(
            !engine.failed_nodes.contains_key(&node_a),
            "attempt {attempt} should nudge, not fail"
        );
        assert_eq!(engine.transcript(&node_a).len(), attempt);
    }

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, empty());
    assert!(engine.failed_nodes.contains_key(&node_a));
}

#[test]
fn autonomous_node_auto_continues_instead_of_awaiting_input() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Ok(needs_input("Now I'll write the tests.")));

    assert!(!engine.awaiting_nodes.contains(&node_a), "must not pause");
    assert!(!engine.failed_nodes.contains_key(&node_a));
    let transcript = engine.transcript(&node_a);
    assert!(matches!(
        transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content })
            if content.contains("call a tool")
    ));
    assert!(matches!(
        &transcript[transcript.len() - 2],
        AgentTranscriptItem::AssistantMessage { content }
            if content == "Now I'll write the tests."
    ));
}

#[test]
fn autonomous_node_fails_after_consecutive_auto_continue_cap() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    for _ in 0..10 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Ok(needs_input("still narrating")));
        assert!(!engine.failed_nodes.contains_key(&node_a));
    }
    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Ok(needs_input("still narrating")));
    assert!(engine.failed_nodes.contains_key(&node_a));
    assert!(!engine.awaiting_nodes.contains(&node_a));
}

#[test]
fn autonomous_auto_continue_streak_resets_on_tool_calls() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    for _ in 0..9 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Ok(needs_input("narration")));
    }
    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall {
                id: "c1".to_string(),
                name: "read".to_string(),
                arguments: json!({}),
            }],
            assistant_message: None,
            usage: None,
        })),
    );
    for _ in 0..10 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Ok(needs_input("narration")));
        assert!(
            !engine.failed_nodes.contains_key(&node_a),
            "streak should have reset after tool-call progress"
        );
    }
}

#[test]
fn interactive_node_still_pauses_for_real_questions() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
        &node_a,
        Ok(needs_input("Which environment should I target?")),
    );
    assert!(engine.awaiting_nodes.contains(&node_a));
}

#[test]
fn malformed_request_input_retries_reset_on_tool_call_progress() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![node("a")];
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let node_a = NodeId("a".to_string());

    for _ in 0..3 {
        engine.test_insert_in_flight(node_a.clone());
        engine.on_ai_complete(&node_a, Ok(needs_input("Working on the next file.")));
        assert!(!engine.awaiting_nodes.contains(&node_a));
    }

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(
        &node_a,
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            tool_calls: vec![ToolCall {
                id: "c1".to_string(),
                name: "read".to_string(),
                arguments: json!({}),
            }],
            assistant_message: None,
            usage: None,
        })),
    );

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Ok(needs_input("Now updating the route.")));
    assert!(
        !engine.awaiting_nodes.contains(&node_a),
        "narration after progress must self-nudge, not pause"
    );
}

#[test]
fn checkpoint_roundtrips_auto_continue_streaks() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes = vec![autonomous_node("a")];
    let mut engine = InteractiveEngine::new(workflow.clone(), None, None).unwrap();
    let node_a = NodeId("a".to_string());

    engine.test_insert_in_flight(node_a.clone());
    engine.on_ai_complete(&node_a, Ok(needs_input("narration")));

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
        restored.auto_continue_streaks_by_node.get(&node_a),
        Some(&1)
    );
}
