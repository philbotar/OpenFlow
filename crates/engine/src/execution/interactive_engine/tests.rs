#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "engine tests use unwrap/expect and panic for concise failure messages"
)]

use super::{
    EngineInputError, EnginePollResult, EngineRunResult, InteractiveEngine, RunError, RunEventKind,
};
use crate::conversation::{AgentTranscriptItem, ChatMessage, ChatRole};
use crate::execution::NodeFailureKind;
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, ToolPort,
};
use crate::tools::{ApprovalMode, FileChangeRecord, ToolCall, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
        _engine: &mut InteractiveEngine,
        _node_id: &NodeId,
        _label: &str,
        calls: Vec<ToolCall>,
    ) -> Vec<ToolResult> {
        calls
            .into_iter()
            .map(|call| ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content: "noop".to_string(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            })
            .collect()
    }

    fn augment_request(&self, _node_id: &NodeId, _request: &mut AgentRequest) {}
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
            }))
        }
    }

    let mut workflow = Workflow::new("backoff");
    workflow.settings.retry_policy.max_attempts = 1;
    workflow.settings.retry_policy.backoff_ms = 500;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();
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
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();
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
fn revert_file_changes_for_batch_removes_only_matching_records() {
    let mut workflow = Workflow::new("revert");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();
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
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected CallAi");
    };
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
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    let result = engine.poll();
    assert!(matches!(
        result,
        EnginePollResult::CallAi { ref node_id, .. } if node_id == "idea"
    ));

    let EnginePollResult::CallAi { request, .. } = result else {
        panic!("expected CallAi");
    };
    assert_eq!(request.node_id, "idea");
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "...".to_string(),
            assistant_message: None,
        })),
    );

    let final_result = engine.poll();
    assert!(matches!(final_result, EnginePollResult::Completed(_)));
}

#[test]
fn non_auto_start_node_pauses_awaiting_input() {
    let mut workflow = Workflow::new("test");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];

    let mut engine = InteractiveEngine::new(workflow, None).unwrap();
    let result = engine.poll();
    assert!(matches!(
        result,
        EnginePollResult::AwaitInput { ref node_id, is_initial: true, .. } if node_id == "idea"
    ));
}

#[test]
fn awaiting_manual_node_repeats_context_until_input_arrives() {
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

#[test]
fn wrong_node_human_input_is_rejected_without_advancing() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();
    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));

    let error = engine
        .on_human_input(&NodeId::from("other"), "Wrong node")
        .unwrap_err();
    let result = engine.poll();

    assert_eq!(
        error,
        EngineInputError::WrongNode {
            expected: NodeId("idea".to_string()),
            got: NodeId("other".to_string())
        }
    );
    assert!(matches!(
        result,
        EnginePollResult::AwaitInput { ref node_id, .. } if node_id == "idea"
    ));
    assert!(engine.node_output(&NodeId::from("idea")).is_none());
}

#[test]
fn manual_node_user_input_starts_ai_request() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
    engine
        .on_human_input(&NodeId::from("idea"), "Need a smaller launch scope")
        .unwrap();

    let result = engine.poll();
    let EnginePollResult::CallAi { request, .. } = result else {
        panic!("expected ai request");
    };
    assert_eq!(request.node_id, "idea");
    assert_eq!(
        request.transcript,
        vec![AgentTranscriptItem::UserMessage {
            content: "Need a smaller launch scope".to_string(),
        }]
    );
}

#[test]
fn non_question_request_input_retries_before_pauses() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
    engine
        .on_human_input(&NodeId::from("idea"), "Need a smaller launch scope")
        .unwrap();
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));

    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message:
                "Let me check the existing animation patterns before I ask anything:".to_string(),
        })),
    );
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    assert!(!engine.awaiting_nodes.contains(&NodeId("idea".to_string())));

    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: "Should the loading indicator use a shimmer or a spinner?"
                .to_string(),
        })),
    );

    assert!(matches!(
        engine.poll(),
        EnginePollResult::AwaitInput { ref node_id, is_initial: false, .. } if node_id == "idea"
    ));
    assert_eq!(
        engine.conversation_history(&NodeId::from("idea")).last(),
        Some(&ChatMessage::text(
            ChatRole::Assistant,
            "Should the loading indicator use a shimmer or a spinner?".to_string(),
        ))
    );
}

#[test]
fn conversation_follow_up_repauses_same_node() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
    engine
        .on_human_input(&NodeId::from("idea"), "Need a smaller launch scope")
        .unwrap();
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "...".to_string(),
            assistant_message: "Which approval step is mandatory?".to_string(),
        })),
    );

    let result = engine.poll();
    assert!(matches!(
        result,
        EnginePollResult::AwaitInput { ref node_id, is_initial: false, .. } if node_id == "idea"
    ));
    assert_eq!(
        engine.conversation_history(&NodeId::from("idea")),
        vec![
            ChatMessage::text(ChatRole::User, "Need a smaller launch scope"),
            ChatMessage::text(ChatRole::Assistant, "Which approval step is mandatory?"),
        ]
    );
}

#[test]
fn tool_calls_pause_for_approval_and_resume_after_results() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.catalog.tools = vec![crate::ToolRef {
        name: "read".to_string(),
        tier: Some(crate::ToolTier::Read),
    }];
    idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
                intent: Some("Reading repo overview".to_string()),
            }],
        })),
    );

    let pending = engine.poll();
    let EnginePollResult::AwaitToolApproval {
        ref approval_id,
        ref node_id,
        ..
    } = pending
    else {
        panic!("expected approval");
    };
    assert_eq!(node_id, "idea");
    let approval_id = approval_id.clone();

    engine.on_tool_decision(&approval_id, true).unwrap();
    let runnable = engine.poll();
    assert!(matches!(
        runnable,
        EnginePollResult::RunTools { ref node_id, .. } if node_id == "idea"
    ));

    engine
        .on_tool_results(
            &NodeId::from("idea"),
            vec![ToolResult {
                tool_call_id: "call-1".to_string(),
                tool_name: "read".to_string(),
                content: "# README".to_string(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            }],
        )
        .unwrap();

    let resumed = engine.poll();
    let EnginePollResult::CallAi { request, .. } = resumed else {
        panic!("expected resumed ai request");
    };
    assert!(matches!(
        request.transcript.as_slice(),
        [
            AgentTranscriptItem::ToolCall { .. },
            AgentTranscriptItem::ToolResult { .. }
        ]
    ));
}

#[test]
fn conversation_completion_sets_output_and_advances() {
    let mut workflow = Workflow::new("manual");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    let final_node = node("final");
    workflow.nodes = vec![idea, final_node];
    workflow.edges = vec![crate::Edge::new("idea", "final")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
    engine
        .on_human_input(&NodeId::from("idea"), "Workflow execution with approvals")
        .unwrap();
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            raw_text: "...".to_string(),
            assistant_message: Some("Locked. Advancing.".to_string()),
            output: json!({"summary": "Workflow execution with approvals"}),
        })),
    );

    assert_eq!(
        engine.node_output(&NodeId::from("idea")),
        Some(json!({"summary": "Workflow execution with approvals"}))
    );
    let next = engine.poll();
    assert!(matches!(
        next,
        EnginePollResult::CallAi { ref node_id, .. } if node_id == "final"
    ));
}

#[test]
fn poll_targets_first_manual_node_in_layer_order() {
    let mut workflow = Workflow::new("indexed");
    let mut first = node("first");
    first.agent.auto_start = false;
    let mut second = node("second");
    second.agent.auto_start = false;
    workflow.nodes = vec![first, second];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    match engine.poll() {
        EnginePollResult::AwaitInput { node_id, .. } => assert_eq!(node_id, "first"),
        other => panic!("expected AwaitInput, got {other:?}"),
    }
}

#[test]
fn yolo_mode_skips_tool_approval() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
                intent: None,
            }],
        })),
    );

    let pending = engine.poll();
    assert!(matches!(
        pending,
        EnginePollResult::RunTools { ref node_id, .. } if node_id == "idea"
    ));
}

#[test]
fn denied_tool_call_resumes_with_error_result() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
                intent: None,
            }],
        })),
    );
    let EnginePollResult::AwaitToolApproval { approval_id, .. } = engine.poll() else {
        panic!("expected approval");
    };

    engine.on_tool_decision(&approval_id, false).unwrap();

    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected resumed AI request");
    };
    assert!(matches!(
        request.transcript.last(),
        Some(AgentTranscriptItem::ToolResult { result })
            if result.is_error && result.content == "denied by user"
    ));
}

#[test]
fn transient_failure_retries_then_succeeds() {
    let mut workflow = Workflow::new("retry");
    workflow.settings.retry_policy.max_attempts = 1;
    workflow.settings.retry_policy.backoff_ms = 25;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Err(AgentError::Transient("timeout".to_string())),
    );

    let retry = engine.poll();
    assert!(matches!(
        retry,
        EnginePollResult::CallAi { ref node_id, .. } if node_id == "idea"
    ));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );

    let EnginePollResult::Completed(report) = engine.poll() else {
        panic!("expected completed report");
    };
    assert!(report.events.iter().any(
        |event| event.kind == RunEventKind::Retrying && event.message.contains("backoff_ms=25")
    ));
    assert_eq!(report.outputs.len(), 1);
}

#[test]
fn permanent_failure_pauses_for_manual_retry() {
    let mut workflow = Workflow::new("retry");
    workflow.settings.retry_policy.max_attempts = 3;
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Err(AgentError::Permanent("schema".to_string())),
    );

    assert!(matches!(engine.poll(), EnginePollResult::Failed(_)));
    assert!(matches!(engine.poll(), EnginePollResult::Failed(_)));
    engine
        .retry_node(&NodeId::from("idea"))
        .expect("manual retry");
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
}

#[test]
fn interrupted_error_is_retryable_without_terminal_failure() {
    let mut workflow = Workflow::new("interrupt");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(&NodeId::from("idea"), Err(AgentError::Interrupted));

    engine
        .retry_node(&NodeId::from("idea"))
        .expect("retry after interrupt");
    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected AI retry with preserved transcript machinery");
    };
    assert_eq!(request.model_attempt, 2);
}

#[test]
fn retry_node_preserves_transcript_and_bumps_attempt() {
    let mut workflow = Workflow::new("transcript");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "{}".to_string(),
            assistant_message: "What scope?".to_string(),
        })),
    );
    engine
        .on_human_input(&NodeId::from("idea"), "full app")
        .expect("input");
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Err(AgentError::Permanent("boom".to_string())),
    );
    let len_before = engine.transcript(&NodeId::from("idea")).len();
    engine.retry_node(&NodeId::from("idea")).expect("retry");
    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected retry AI call");
    };
    assert_eq!(request.transcript.len(), len_before);
    assert_eq!(request.model_attempt, 2);
}

#[test]
fn malformed_submit_output_retries_then_succeeds() {
    let mut workflow = Workflow::new("submit-retry");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Err(AgentError::Failed(
            "OpenAI-compatible final output tool arguments were not valid JSON: missing field `output`"
                .to_string(),
        )),
    );

    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected retry AI call");
    };
    assert!(matches!(
        request.transcript.last(),
        Some(AgentTranscriptItem::UserMessage { content })
            if content.contains("openflow_submit_node_output")
    ));

    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );

    let EnginePollResult::Completed(report) = engine.poll() else {
        panic!("expected completed report");
    };
    assert!(report.events.iter().any(|event| {
        event.kind == RunEventKind::Retrying && event.message.contains("malformed submit-output")
    }));
}

#[test]
fn tool_config_names_survive_into_request() {
    let mut workflow = Workflow::new("tools");
    let mut idea = node("idea");
    idea.agent.tools.catalog.tools = vec![crate::ToolRef {
        name: "search".to_string(),
        tier: Some(crate::ToolTier::Read),
    }];
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    let EnginePollResult::CallAi { request, .. } = engine.poll() else {
        panic!("expected AI request");
    };

    assert!(request.available_tools.is_empty());
    assert_eq!(request.tool_config.catalog.tools[0].name, "search");
}

#[test]
fn tool_call_xml_echo_is_dropped_from_transcript() {
    let mut workflow = Workflow::new("tooling");
    let mut idea = node("idea");
    idea.agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "...".to_string(),
            assistant_message: Some(
                "<tool_call><function=read></function></tool_call>".to_string(),
            ),
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
                intent: None,
            }],
        })),
    );

    assert!(!engine
        .transcript(&NodeId::from("idea"))
        .iter()
        .any(|item| matches!(
            item,
            AgentTranscriptItem::AssistantMessage { content }
                if content.contains("<tool_call>")
        )));
}

#[test]
fn completion_tool_call_xml_echo_is_dropped_from_transcript() {
    let mut workflow = Workflow::new("completion");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: Some(
                "<tool_call><function=read></function></tool_call>".to_string(),
            ),
        })),
    );

    assert!(engine.transcript(&NodeId::from("idea")).is_empty());
}

#[test]
fn misrouted_completion_is_rejected() {
    let mut workflow = Workflow::new("misroute");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("other"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "wrong"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );

    let EnginePollResult::Failed(RunError::NodeFailed { node_id, kind }) = engine.poll() else {
        panic!("expected failure");
    };
    assert_eq!(node_id, "other");
    assert_eq!(
        kind,
        NodeFailureKind::MisroutedCompletion(
            "expected model completion for idea, got other".to_string()
        )
    );
}

#[test]
fn started_event_is_provider_neutral_and_emitted_once_per_poll_attempt() {
    let mut workflow = Workflow::new("events");
    workflow.nodes = vec![node("idea")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"summary": "ok"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );

    let EnginePollResult::Completed(report) = engine.poll() else {
        panic!("expected completion");
    };
    let started = report
        .events
        .iter()
        .filter(|event| event.kind == RunEventKind::Started)
        .collect::<Vec<_>>();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].message, "invoking model");
}

#[test]
fn inbound_ports_drive_engine_inputs() {
    use crate::ports::inbound::{HumanInput, HumanInputPort, ToolApprovalInput, ToolApprovalPort};

    let mut workflow = Workflow::new("ports");
    let mut idea = node("idea");
    idea.agent.auto_start = false;
    idea.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![idea];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::AwaitInput { .. }));
    HumanInputPort::submit_human_input(
        &mut engine,
        HumanInput {
            node_id: NodeId("idea".to_string()),
            text: "Need context".to_string(),
        },
    )
    .unwrap();
    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
    engine.on_ai_complete(
        &NodeId::from("idea"),
        Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: "{}".to_string(),
            assistant_message: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
                intent: None,
            }],
        })),
    );
    let EnginePollResult::AwaitToolApproval { approval_id, .. } = engine.poll() else {
        panic!("expected approval");
    };
    ToolApprovalPort::submit_tool_approval(
        &mut engine,
        ToolApprovalInput {
            approval_id,
            allow: false,
        },
    )
    .unwrap();

    assert!(matches!(engine.poll(), EnginePollResult::CallAi { .. }));
}

#[test]
fn parallel_sibling_nodes_start_ai_together_in_layer() {
    let mut workflow = Workflow::new("parallel");
    workflow.nodes = vec![node("plan"), node("risk")];
    let mut engine = InteractiveEngine::new(workflow, None).unwrap();

    let first = engine.poll();
    let second = engine.poll();
    assert!(matches!(
        first,
        EnginePollResult::CallAi { ref node_id, .. } if node_id == "plan"
    ));
    assert!(matches!(
        second,
        EnginePollResult::CallAi { ref node_id, .. } if node_id == "risk"
    ));

    engine.on_ai_complete(
        &NodeId::from("plan"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"plan": "a"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );
    engine.on_ai_complete(
        &NodeId::from("risk"),
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: json!({"risk": "b"}),
            raw_text: "{}".to_string(),
            assistant_message: None,
        })),
    );

    assert!(matches!(engine.poll(), EnginePollResult::Completed(_)));
}
