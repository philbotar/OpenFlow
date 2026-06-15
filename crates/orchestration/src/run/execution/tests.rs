use super::*;
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::{IncidentRecorder, IncidentScope};
use crate::run::coordinator::RunCoordinator;
use crate::run::state::{TraceStatus, WorkflowRunState};
use crate::tools::ToolRegistry;
use async_trait::async_trait;
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink, ApprovalMode, ChatRole, NodeId,
    NodeToolConfig, SubagentStatus, SubagentSummary, ToolCall, ToolCallStatus, ToolRef, ToolTier,
    Workflow,
};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

fn sample_agent_request() -> AgentRequest {
    AgentRequest {
        workflow_id: "wf-1".into(),
        node_id: "choose-feature".into(),
        node_label: "Choose feature".to_string(),
        model: "test-model".to_string(),
        system_messages: Vec::new(),
        task_prompt: String::new(),
        input: json!({}),
        output_schema: json!({}),
        tool_config: NodeToolConfig::default(),
        available_tools: Vec::new(),
        transcript: Vec::new(),
        model_attempt: 1,
        reasoning_effort: None,
        reasoning_budget_tokens: None,
    }
}

#[tokio::test]
async fn adapter_emits_clarifying_question_after_streamed_preamble() {
    struct StreamingNeedsInputAi;

    #[async_trait]
    impl engine::AiPort for StreamingNeedsInputAi {
        async fn invoke(
            &self,
            _request: AgentRequest,
        ) -> Result<AgentTurnOutcome, engine::AgentError> {
            panic!("AiInvocationAdapter should call invoke_stream");
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, engine::AgentError> {
            sink.on_stream_event(AiStreamEvent::AssistantDelta {
                content: "That's clear! Let me confirm one detail before proceeding:".to_string(),
            });
            Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: "{}".to_string(),
                assistant_message: "Should tool rows animate like Cursor's shimmer?".to_string(),
            }))
        }
    }

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let node_interrupts = Arc::new(parking_lot::Mutex::new(BTreeMap::new()));
    let adapter = AiInvocationAdapter::new(
        Arc::new(StreamingNeedsInputAi),
        event_tx,
        node_interrupts,
        CancellationToken::new(),
    );
    adapter
        .invoke(sample_agent_request())
        .await
        .expect("invoke succeeds");

    let mut streamed_preamble = false;
    let mut emitted_question = false;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            ExecutionEvent::ChatMessageDelta { delta, .. } if !delta.is_empty() => {
                streamed_preamble |= delta.contains("confirm one detail");
            }
            ExecutionEvent::ChatMessage { role, content, .. }
                if role == ChatRole::Assistant
                    && content.contains("Should tool rows animate like Cursor's shimmer?") =>
            {
                emitted_question = true;
            }
            _ => {}
        }
    }

    assert!(streamed_preamble, "expected streamed preamble delta");
    assert!(
        emitted_question,
        "expected clarifying question ChatMessage after streamed preamble"
    );
}

fn workflow() -> Workflow {
    let mut workflow = Workflow::new("trace");
    let mut first = engine::Node::agent("First", 0.0, 0.0);
    first.id = NodeId("first".to_string());
    first.agent.model = "test-model".to_string();
    workflow.nodes = vec![first];
    workflow
}

#[test]
fn tool_updated_keeps_tool_running_and_records_last_output() {
    let workflow = Workflow::new("w");
    let node_id = NodeId("node-a".to_string());
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        RunTelemetry::ToolCallProposed {
            node_id: node_id.clone(),
            label: "Agent".to_string(),
            tool_call: engine::ToolCall {
                id: "tool-1".to_string(),
                name: "bash".to_string(),
                arguments: serde_json::json!({"command": "cargo test", "_i": "run tests"}),
            },
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        RunTelemetry::ToolStarted {
            node_id: node_id.clone(),
            tool_call_id: "tool-1".to_string(),
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({"command": "cargo test"}),
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        RunTelemetry::ToolUpdated {
            node_id: node_id.clone(),
            tool_call_id: "tool-1".to_string(),
            tool_name: "bash".to_string(),
            content: "running test x".to_string(),
            output_meta: None,
        },
    );

    let summary = &state.tool_calls_by_node[&node_id][0];
    assert_eq!(summary.status, engine::ToolCallStatus::Running);
    assert_eq!(summary.last_output.as_deref(), Some("running test x"));
}

#[test]
fn reducer_aborted_deactivates_run_and_marks_in_progress_tools() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolCallProposed {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            tool_call: ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({ "path": "README.md" }),
            },
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolStarted {
            node_id: NodeId("first".to_string()),
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            arguments: json!({ "path": "README.md" }),
        },
    );
    apply_event_to_run_state(&workflow, &mut state, ExecutionEvent::Aborted);

    assert!(!state.active);
    assert!(state.last_error.is_none());
    let calls = &state.tool_calls_by_node[&NodeId("first".to_string())];
    assert_eq!(calls[0].status, ToolCallStatus::Aborted);
    assert_eq!(
        state.status_by_node.get(&NodeId("first".to_string())),
        Some(&crate::run::state::AgentStatus::Stopped)
    );
    assert_eq!(state.run_trace[0].status, TraceStatus::Stopped);
    assert_eq!(state.run_trace[0].message, "Stopped");
}

#[test]
fn reducer_node_completed_pushes_summary_completion_message() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeCompleted {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            output: json!({"summary": "Captured the welcome message."}),
        },
    );

    let chat = &state.chat_logs[&NodeId("first".to_string())];
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].content, "Captured the welcome message.");
    assert_eq!(
        chat[0].message_kind,
        Some(engine::ChatMessageKind::NodeCompleted)
    );
}

#[test]
fn reducer_node_completed_skips_chat_when_summary_missing() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeCompleted {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            output: json!({"ok": true}),
        },
    );

    assert!(!state.chat_logs.contains_key(&NodeId("first".to_string())));
}

#[test]
fn reducer_stream_finalize_strips_echoed_tool_call_markup() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    let message_id = "stream-1".to_string();
    let echoed = concat!(
        "I'll submit the audit now.",
        "<tool_call>\n<function=openflow_submit_node_output>\n",
        "<parameter=output>{\"summary\":\"done\"}</parameter>\n",
        "</function>\n</tool_call>"
    );

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            role: ChatRole::Assistant,
            delta: echoed.to_string(),
            finalize: false,
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            role: ChatRole::Assistant,
            delta: String::new(),
            finalize: true,
        },
    );

    let chat = &state.chat_logs[&node_id];
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].content, "I'll submit the audit now.");
    assert!(!chat[0].streaming);
}

#[test]
fn reducer_stream_finalize_drops_markup_only_messages() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    let message_id = "stream-2".to_string();
    let echoed = "<tool_call>\n<function=search>\n</function>\n</tool_call>";

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            role: ChatRole::Assistant,
            delta: echoed.to_string(),
            finalize: false,
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id,
            role: ChatRole::Assistant,
            delta: String::new(),
            finalize: true,
        },
    );

    assert!(!state.chat_logs.contains_key(&node_id));
}

#[test]
fn reducer_stream_delta_keeps_raw_content_before_finalize() {
    // Mid-stream content stays raw: the UI strips markup for display
    // (`stripToolCallMarkup` mirror), and stripping the stored content per
    // delta is lossy when markup spans delta boundaries.
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    let message_id = "stream-3".to_string();

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            role: ChatRole::Assistant,
            delta: "Planning.<tool_cal".to_string(),
            finalize: false,
        },
    );

    let chat = &state.chat_logs[&node_id];
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].content, "Planning.<tool_cal");
    assert!(chat[0].streaming);
}

#[test]
fn reducer_stream_finalize_strips_markup_split_across_deltas() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    let message_id = "stream-4".to_string();

    for (delta, finalize) in [
        ("Answer.<tool_call name=", false),
        ("\"x\">stuff</tool_call>", false),
        ("", true),
    ] {
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::ChatMessageDelta {
                node_id: node_id.clone(),
                message_id: message_id.clone(),
                role: ChatRole::Assistant,
                delta: delta.to_string(),
                finalize,
            },
        );
    }

    let chat = &state.chat_logs[&node_id];
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].content, "Answer.");
    assert!(!chat[0].streaming);
}

#[test]
fn reducer_stream_thinking_delta_uses_thinking_role() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    let message_id = "think-1".to_string();

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            role: ChatRole::Thinking,
            delta: "Let me reason step by step.".to_string(),
            finalize: false,
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id,
            role: ChatRole::Thinking,
            delta: String::new(),
            finalize: true,
        },
    );

    let chat = &state.chat_logs[&node_id];
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].role, ChatRole::Thinking);
    assert_eq!(chat[0].content, "Let me reason step by step.");
    assert!(!chat[0].streaming);
}

#[test]
fn reducer_tool_completed_restores_thinking_status() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeStarted {
            node_id: node_id.clone(),
            label: "First".to_string(),
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolStarted {
            node_id: node_id.clone(),
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            arguments: json!({ "path": "README.md" }),
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolCompleted {
            node_id: node_id.clone(),
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            content: "done".to_string(),
            is_error: false,
            output_meta: None,
            artifact_ids: Vec::new(),
        },
    );

    assert_eq!(
        state.status_by_node.get(&node_id),
        Some(&crate::run::state::AgentStatus::Started)
    );
}

#[test]
fn record_entrypoint_message_appends_user_chat_without_status_change() {
    let mut state = WorkflowRunState::running_for_workflow(&Workflow::new("w"));
    state.status_by_node.insert(
        NodeId("root".into()),
        crate::run::state::AgentStatus::Queued,
    );
    record_entrypoint_message(&mut state, "root", "Plan ORCHID-91".to_string());
    assert_eq!(state.chat_logs[&NodeId("root".into())].len(), 1);
    assert_eq!(
        state.chat_logs[&NodeId("root".into())][0].role,
        ChatRole::User
    );
    assert_eq!(
        state.status_by_node[&NodeId("root".into())],
        crate::run::state::AgentStatus::Queued
    );
}

#[test]
fn record_user_input_restores_thinking_status() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    let node_id = NodeId("first".to_string());
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeAwaitingInput {
            node_id: node_id.clone(),
            label: "First".to_string(),
            context: "Need more detail".to_string(),
            is_initial: false,
        },
    );
    record_user_input(&mut state, "first", "Continue".to_string());

    assert!(!state.awaiting_node_ids.contains(&node_id));
    assert_eq!(
        state.status_by_node.get(&node_id),
        Some(&crate::run::state::AgentStatus::Started)
    );
}

#[test]
fn reducer_tracks_tool_approval_and_completion() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolCallProposed {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            tool_call: ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "README.md"}),
            },
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolApprovalRequested {
            request: engine::PendingToolApproval {
                approval_id: "approval-1".to_string(),
                node_id: NodeId::from("first"),
                node_label: "First".to_string(),
                tool_call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                },
                tier: ToolTier::Read,
            },
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolCompleted {
            node_id: NodeId("first".to_string()),
            tool_call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            content: "done".to_string(),
            is_error: false,
            output_meta: None,
            artifact_ids: Vec::new(),
        },
    );

    assert_eq!(state.pending_approvals.len(), 1);
    assert_eq!(
        state.tool_calls_by_node[&NodeId("first".to_string())][0].tool_name,
        "read"
    );
    let chat = &state.chat_logs[&NodeId("first".to_string())];
    assert_eq!(chat[0].tool_call_id.as_deref(), Some("call-1"));
    assert!(chat[1]
        .content
        .contains("Approval required for tool 'read'."));
    assert_eq!(
        state.tool_calls_by_node[&NodeId("first".to_string())][0]
            .last_output
            .as_deref(),
        Some("done")
    );
}

#[tokio::test]
async fn headless_run_auto_approves_read_tool_and_reenters_model_loop() {
    #[derive(Clone, Default)]
    struct ScriptedAi {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl AiPort for ScriptedAi {
        async fn invoke(
            &self,
            request: AgentRequest,
        ) -> Result<AgentTurnOutcome, engine::AgentError> {
            let mut calls = self.calls.lock();
            *calls += 1;
            if *calls == 1 {
                assert_eq!(request.available_tools.len(), 3);
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: Some("Inspecting docs".to_string()),
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                    }],
                }));
            }
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let mut workflow = workflow();
    workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
        tier: Some(engine::ToolTier::Read),
    }];
    let snapshot = run_workflow_headless(
        workflow,
        None,
        ScriptedAi::default(),
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot.outputs[&NodeId("first".to_string())],
        json!({"summary": "done"})
    );
    assert!(!snapshot.tool_calls_by_node[&NodeId("first".to_string())].is_empty());
}

#[tokio::test]
async fn headless_run_requires_scripted_approval_for_prompted_tool() {
    #[derive(Clone)]
    struct PromptingAi;

    #[async_trait]
    impl AiPort for PromptingAi {
        async fn invoke(
            &self,
            _request: AgentRequest,
        ) -> Result<AgentTurnOutcome, engine::AgentError> {
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                }],
            }))
        }
    }

    let mut workflow = workflow();
    workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
        tier: Some(engine::ToolTier::Read),
    }];
    workflow.nodes[0].agent.tools.approval_mode = Some(engine::ApprovalMode::AlwaysAsk);
    let error = run_workflow_headless(
        workflow,
        None,
        PromptingAi,
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, WorkflowExecutionError::MissingApproval(_)));
}

#[tokio::test]
async fn apply_execution_event_tool_failure_persists_jsonl_incident_scope() {
    // Coordinator tests already cover direct incident mapping from apply_execution_event.
    // This execution-side variant also verifies JSONL persistence and scoped fields.
    let dir = TempDir::new().expect("tempdir");
    let incident_path = dir.path().join("incidents.jsonl");
    let incidents = Arc::new(IncidentRecorder::new(Arc::new(FileIncidentStore::new(
        incident_path.clone(),
    ))));
    let coordinator =
        RunCoordinator::new_with_incidents(tokio::runtime::Handle::current(), incidents.clone());

    let workflow = workflow();
    let expected_workflow_id = workflow.id.to_string();
    let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
    run_state.run_id = Some("run-execution-incident-1".to_string());
    let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
    coordinator
        .test_seed_session(workflow, run_state, action_tx)
        .await;

    coordinator
        .apply_execution_event(ExecutionEvent::ToolCompleted {
            node_id: NodeId("first".to_string()),
            tool_call_id: "tool-incident-1".to_string(),
            tool_name: "read".to_string(),
            content: "[not_found] missing file — use project file references".to_string(),
            is_error: true,
            output_meta: None,
            artifact_ids: Vec::new(),
        })
        .await
        .expect("apply execution event");

    let persisted_lines = fs::read_to_string(&incident_path).expect("read incidents jsonl");
    let non_empty_lines: Vec<&str> = persisted_lines
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(non_empty_lines.len(), 1);

    let listed = incidents.list_unresolved(10).expect("list incidents");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].code, "tool.not_found");
    match &listed[0].scope {
        IncidentScope::Node {
            run_id,
            workflow_id,
            node_id,
        } => {
            assert_eq!(run_id, "run-execution-incident-1");
            assert_eq!(workflow_id, &expected_workflow_id);
            assert_eq!(node_id, &NodeId("first".to_string()));
        }
        scope => panic!("expected node scope, got {scope:?}"),
    }
}

#[test]
fn reducer_node_interrupted_keeps_run_active() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeInterrupted {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
        },
    );

    assert!(state.active);
    assert_eq!(
        state.status_by_node[&NodeId("first".to_string())],
        crate::run::state::AgentStatus::Interrupted
    );
    assert_eq!(state.run_trace[0].status, TraceStatus::Paused);
}

#[test]
fn reducer_node_errored_keeps_run_active() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeErrored {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            error: "boom".to_string(),
        },
    );

    assert!(state.active);
    assert_eq!(
        state.status_by_node[&NodeId("first".to_string())],
        crate::run::state::AgentStatus::Failed
    );
    assert_eq!(state.last_error.as_deref(), Some("boom"));
    assert_eq!(state.run_trace[0].status, TraceStatus::Failed);
}

#[test]
fn reducer_marks_failure_terminal_with_label() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::NodeFailed {
            node_id: NodeId("first".to_string()),
            label: "First".to_string(),
            error: "boom".to_string(),
        },
    );

    assert!(!state.active);
    assert_eq!(state.last_error.as_deref(), Some("boom"));
    assert_eq!(state.run_trace[0].node_label, "First");
    assert_eq!(state.run_trace[0].status, TraceStatus::Failed);
}

#[test]
fn subagents_declared_event_updates_run_state() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![
                SubagentSummary {
                    id: "first-subagent-1".to_string(),
                    name: "Researcher".to_string(),
                    purpose: "Investigate API behavior".to_string(),
                    status: SubagentStatus::Declared,
                },
                SubagentSummary {
                    id: "first-subagent-2".to_string(),
                    name: "Writer".to_string(),
                    purpose: "Summarize findings".to_string(),
                    status: SubagentStatus::Declared,
                },
            ],
        },
    );

    let subs = &state.subagents_by_node[&NodeId("first".to_string())];
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].name, "Researcher");
    assert_eq!(subs[0].purpose, "Investigate API behavior");
    assert_eq!(subs[0].status, SubagentStatus::Declared);
    assert_eq!(subs[0].id, "first-subagent-1");
    assert_eq!(subs[1].name, "Writer");
    assert_eq!(subs[1].id, "first-subagent-2");

    assert!(state.chat_logs[&NodeId("first".to_string())]
        .iter()
        .any(|m| m.content.contains("Registered 2 subagent")));
}

#[test]
fn subagents_are_scoped_to_declaring_node() {
    let mut second = engine::Node::agent("Second", 100.0, 0.0);
    second.id = NodeId("second".to_string());
    second.agent.model = "test-model".to_string();
    let mut workflow = workflow();
    workflow.nodes.push(second);
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "first-subagent-1".to_string(),
                name: "Researcher".to_string(),
                purpose: "Investigate".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );

    assert!(state
        .subagents_by_node
        .contains_key(&NodeId("first".to_string())));
    assert!(!state
        .subagents_by_node
        .contains_key(&NodeId("second".to_string())));
}

#[test]
fn fresh_run_state_has_empty_subagents() {
    let workflow = workflow();
    let state = WorkflowRunState::running_for_workflow(&workflow);
    assert!(state.subagents_by_node.is_empty());
}

#[test]
fn subagent_started_event_transitions_status() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    // First declare the subagent
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "first-subagent-1".to_string(),
                name: "Worker".to_string(),
                purpose: "Do work".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );

    // Then start it
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentStarted {
            node_id: NodeId("first".to_string()),
            subagent_id: "first-subagent-1".to_string(),
        },
    );

    let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
    assert_eq!(sub.status, SubagentStatus::Active);
    assert!(state.chat_logs[&NodeId("first".to_string())]
        .iter()
        .any(|m| m.content.contains("Subagent first-subagent-1 started")));
}

#[test]
fn subagent_completed_event_transitions_status() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "first-subagent-1".to_string(),
                name: "Worker".to_string(),
                purpose: "Do work".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentStarted {
            node_id: NodeId("first".to_string()),
            subagent_id: "first-subagent-1".to_string(),
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentCompleted {
            node_id: NodeId("first".to_string()),
            subagent_id: "first-subagent-1".to_string(),
        },
    );

    let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
    assert_eq!(sub.status, SubagentStatus::Completed);
}

#[test]
fn subagent_failed_event_transitions_status() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "first-subagent-1".to_string(),
                name: "Worker".to_string(),
                purpose: "Do work".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentStarted {
            node_id: NodeId("first".to_string()),
            subagent_id: "first-subagent-1".to_string(),
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentFailed {
            node_id: NodeId("first".to_string()),
            subagent_id: "first-subagent-1".to_string(),
            error: "API error".to_string(),
        },
    );

    let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
    assert_eq!(sub.status, SubagentStatus::Failed);
    assert!(state.chat_logs[&NodeId("first".to_string())]
        .iter()
        .any(|m| m
            .content
            .contains("Subagent first-subagent-1 failed: API error")));
}

#[test]
fn declare_subagents_tool_is_always_in_definitions() {
    let registry = ToolRegistry::new();
    let definitions = registry.definitions_for(&NodeToolConfig::default());
    let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"openflow_declare_subagents"));
    assert!(names.contains(&"openflow_call_subagent"));
}

#[test]
fn resolve_execution_cwd_uses_process_directory_when_unset() {
    let cwd = resolve_execution_cwd(None).expect("fallback cwd");
    assert!(cwd.is_dir());
}

#[test]
fn resolve_execution_cwd_rejects_invalid_directory() {
    let error = resolve_execution_cwd(Some("/definitely/not/a/real/openflow/path"))
        .expect_err("invalid path");
    assert!(error.contains("execution folder"));
}

#[test]
fn subagents_declared_event_appends_without_replacing() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "saved-agent-1".to_string(),
                name: "Researcher".to_string(),
                purpose: "Saved agent".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::SubagentsDeclared {
            node_id: NodeId("first".to_string()),
            summaries: vec![SubagentSummary {
                id: "first-subagent-1".to_string(),
                name: "Worker".to_string(),
                purpose: "Ad hoc".to_string(),
                status: SubagentStatus::Declared,
            }],
        },
    );

    let subs = &state.subagents_by_node[&NodeId("first".to_string())];
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].id, "saved-agent-1");
    assert_eq!(subs[1].id, "first-subagent-1");
}

#[test]
fn file_changed_event_appends_to_run_state() {
    let workflow = workflow();
    let mut state = WorkflowRunState::running_for_workflow(&workflow);
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::FileChanged {
            node_id: NodeId("first".to_string()),
            record: engine::FileChangeRecord {
                path: "src/main.rs".to_string(),
                op: engine::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: None,
                timestamp_ms: 1,
            },
        },
    );
    assert_eq!(state.changed_files.len(), 1);
    assert_eq!(state.changed_files[0].path, "src/main.rs");
    let node_files = state
        .changed_files_by_node
        .get(&NodeId("first".to_string()))
        .expect("per-node ledger");
    assert_eq!(node_files.len(), 1);
    assert_eq!(node_files[0].path, "src/main.rs");
}

fn parallel_pause_workflow() -> Workflow {
    let mut workflow = Workflow::new("parallel-pause");
    let mut wait = engine::Node::agent("Wait", 0.0, 0.0);
    wait.id = NodeId("wait".to_string());
    wait.agent.auto_start = false;
    let mut fail = engine::Node::agent("Fail", 200.0, 0.0);
    fail.id = NodeId("fail".to_string());
    fail.agent.auto_start = true;
    fail.agent.model = "test-model".to_string();
    workflow.nodes = vec![wait, fail];
    workflow
}

fn interactive_run_params<A>(
    workflow: Workflow,
    execution_cwd: std::path::PathBuf,
    ai: A,
) -> InteractiveWorkflowRunParams<A>
where
    A: AiPort + Send + Sync + 'static,
{
    InteractiveWorkflowRunParams {
        workflow,
        entrypoint: None,
        execution_cwd,
        artifact_root: super::new_artifact_root(),
        resume_checkpoint: None,
        checkpoint_sink: Arc::new(parking_lot::Mutex::new(None)),
        ai,
        agent_snapshots: BTreeMap::new(),
        snapshot_store: Arc::new(
            crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
        ),
        lsp: crate::lsp::LspSettings::from_env(),
        pending_engine_reverts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        node_interrupts: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
    }
}

#[tokio::test]
async fn interrupt_during_slow_tool_emits_node_interrupted() {
    #[derive(Clone)]
    struct BashSleepAi;

    #[async_trait]
    impl AiPort for BashSleepAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "call-sleep".to_string(),
                    name: "bash".to_string(),
                    arguments: json!({"command": "sleep 30", "timeout": 30}),
                }],
            }))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let temp = TempDir::new().expect("tempdir");
    let mut workflow = workflow();
    workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
        name: "bash".to_string(),
        tier: Some(ToolTier::Exec),
    }];
    workflow.nodes[0].agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    let node_id = workflow.nodes[0].id.clone();
    let params = interactive_run_params(workflow, temp.path().to_path_buf(), BashSleepAi);
    let node_interrupts = params.node_interrupts.clone();
    let (handle, mut event_rx, _action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    let mut tool_started = false;
    let mut interrupted = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(10), event_rx.recv()).await {
        match event {
            ExecutionEvent::ToolStarted { node_id: id, .. } if id == node_id => {
                tool_started = true;
                if let Some((_, token)) = node_interrupts.lock().get(&node_id) {
                    token.cancel();
                }
            }
            ExecutionEvent::NodeInterrupted { node_id: id, .. } if id == node_id => {
                interrupted = true;
                break;
            }
            ExecutionEvent::Finished(_) | ExecutionEvent::Aborted => break,
            ExecutionEvent::NodeFailed { node_id: id, .. } if id == node_id => break,
            _ => {}
        }
    }

    handle.abort();
    assert!(tool_started, "expected bash tool to start before interrupt");
    assert!(
        interrupted,
        "expected NodeInterrupted after per-node cancel"
    );
}

#[tokio::test]
async fn retrying_failed_node_does_not_re_emit_sibling_input_pause() {
    #[derive(Clone)]
    struct FailOnlyAi;

    #[async_trait]
    impl AiPort for FailOnlyAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            assert_eq!(request.node_id.0, "fail");
            Err(AgentError::Permanent("boom".to_string()))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let temp = TempDir::new().expect("tempdir");
    let workflow = parallel_pause_workflow();
    let (handle, mut event_rx, action_tx, _cancel, _) = spawn_interactive_workflow_run(
        &tokio::runtime::Handle::current(),
        interactive_run_params(workflow, temp.path().to_path_buf(), FailOnlyAi),
    );

    let mut wait_input_events = 0usize;
    let mut sent_retry = false;

    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        match event {
            ExecutionEvent::NodeAwaitingInput { node_id, .. } if node_id.0 == "wait" => {
                wait_input_events += 1;
            }
            ExecutionEvent::NodeErrored { node_id, .. } if node_id.0 == "fail" && !sent_retry => {
                sent_retry = true;
                action_tx
                    .send(ExecutionAction::RetryNode {
                        node_id: NodeId("fail".to_string()),
                    })
                    .expect("retry action");
            }
            ExecutionEvent::NodeErrored { node_id, .. } if node_id.0 == "fail" && sent_retry => {
                break;
            }
            _ => {}
        }
        if wait_input_events > 1 {
            break;
        }
    }

    handle.abort();
    assert!(sent_retry, "expected fail node to error before retry");
    assert_eq!(
        wait_input_events, 1,
        "retry must not re-emit NodeAwaitingInput for the sibling wait node"
    );
}

#[tokio::test]
async fn headless_retries_transient_node_error() {
    #[derive(Clone)]
    struct TransientTwiceAi {
        calls: Arc<AtomicUsize>,
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    #[async_trait]
    impl AiPort for TransientTwiceAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call < 2 {
                return Err(AgentError::Transient("timeout".to_string()));
            }
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "ok"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    let mut wf = workflow();
    wf.settings.retry_policy.max_attempts = 1;

    let snapshot = run_workflow_headless(
        wf,
        None,
        TransientTwiceAi {
            calls: Arc::new(AtomicUsize::new(0)),
        },
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .await
    .expect("headless should auto-retry transient failure");

    assert_eq!(snapshot.report.outputs.len(), 1);
}

#[tokio::test]
async fn headless_exhausted_transient_retries_returns_missing_retry() {
    #[derive(Clone)]
    struct AlwaysTransientAi;

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    #[async_trait]
    impl AiPort for AlwaysTransientAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Transient("timeout".to_string()))
        }
    }

    let mut wf = workflow();
    wf.settings.retry_policy.max_attempts = 1;

    let error = run_workflow_headless(
        wf,
        None,
        AlwaysTransientAi,
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .await
    .expect_err("exhausted transient retries should surface MissingRetry");

    assert!(matches!(
        error,
        WorkflowExecutionError::MissingRetry(node_id) if node_id.0 == "first"
    ));
}

#[tokio::test]
async fn headless_run_errors_on_retryable_node_failure_instead_of_hanging() {
    #[derive(Clone)]
    struct PermanentFailAi;

    #[async_trait]
    impl AiPort for PermanentFailAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            Err(AgentError::Permanent("boom".to_string()))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let error = run_workflow_headless(
        workflow(),
        None,
        PermanentFailAi,
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
        None,
    )
    .await
    .expect_err("permanent failure should surface as MissingRetry");

    assert!(matches!(
        error,
        WorkflowExecutionError::MissingRetry(node_id) if node_id.0 == "first"
    ));
}

fn manual_review_workflow() -> Workflow {
    let mut workflow = Workflow::new("stop-continue");
    let mut node = engine::Node::agent("review", 0.0, 0.0);
    node.id = engine::NodeId("review".to_string());
    node.agent.auto_start = false;
    node.agent.model = "test-model".to_string();
    workflow.nodes = vec![node];
    workflow
}

fn interactive_run_params_with_sink<A>(
    workflow: Workflow,
    execution_cwd: std::path::PathBuf,
    ai: A,
) -> (
    InteractiveWorkflowRunParams<A>,
    Arc<parking_lot::Mutex<Option<engine::InteractiveEngineCheckpoint>>>,
)
where
    A: AiPort + Send + Sync + 'static,
{
    let checkpoint_sink = Arc::new(parking_lot::Mutex::new(None));
    let params = InteractiveWorkflowRunParams {
        workflow,
        entrypoint: None,
        execution_cwd,
        artifact_root: super::new_artifact_root(),
        resume_checkpoint: None,
        checkpoint_sink: checkpoint_sink.clone(),
        ai,
        agent_snapshots: BTreeMap::new(),
        snapshot_store: Arc::new(
            crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
        ),
        lsp: crate::lsp::LspSettings::from_env(),
        pending_engine_reverts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        node_interrupts: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
    };
    (params, checkpoint_sink)
}

#[tokio::test]
async fn stop_then_continue_restores_awaiting_input() {
    #[derive(Clone)]
    struct CompleteAi;

    #[async_trait]
    impl AiPort for CompleteAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let temp = TempDir::new().expect("tempdir");
    let workflow = manual_review_workflow();
    let artifact_root = super::new_artifact_root();
    let (params, checkpoint_sink) =
        interactive_run_params_with_sink(workflow.clone(), temp.path().to_path_buf(), CompleteAi);
    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    let mut saw_awaiting = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if matches!(
            event,
            ExecutionEvent::NodeAwaitingInput { ref node_id, .. } if node_id.0 == "review"
        ) {
            saw_awaiting = true;
            action_tx.send(ExecutionAction::Stop).expect("stop");
        }
        if matches!(event, ExecutionEvent::Aborted) {
            break;
        }
    }
    handle.await.expect("drive task");

    assert!(saw_awaiting, "expected awaiting input before stop");
    let checkpoint = checkpoint_sink
        .lock()
        .clone()
        .expect("checkpoint after stop");
    assert!(checkpoint
        .awaiting_nodes
        .contains(&engine::NodeId("review".to_string())));

    let (resume_params, _) =
        interactive_run_params_with_sink(workflow, temp.path().to_path_buf(), CompleteAi);
    let resume_params = InteractiveWorkflowRunParams {
        artifact_root,
        resume_checkpoint: Some(checkpoint),
        ..resume_params
    };
    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), resume_params);

    let mut saw_awaiting_again = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if matches!(
            event,
            ExecutionEvent::NodeAwaitingInput { ref node_id, .. } if node_id.0 == "review"
        ) {
            saw_awaiting_again = true;
            action_tx
                .send(ExecutionAction::ProvideInput {
                    node_id: engine::NodeId("review".to_string()),
                    text: "continue".to_string(),
                })
                .expect("input");
        }
        if matches!(
            event,
            ExecutionEvent::NodeCompleted { ref node_id, .. } if node_id.0 == "review"
        ) {
            break;
        }
    }
    handle.await.expect("resume drive task");
    assert!(saw_awaiting_again, "expected awaiting input after continue");
}

#[tokio::test]
async fn stop_mid_run_then_continue_completes_node() {
    #[derive(Clone)]
    struct SlowCompleteAi;

    #[async_trait]
    impl AiPort for SlowCompleteAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            _request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "resumed"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let temp = TempDir::new().expect("tempdir");
    let mut workflow = Workflow::new("mid-run");
    let mut node = engine::Node::agent("idea", 0.0, 0.0);
    node.id = engine::NodeId("idea".to_string());
    node.agent.model = "test-model".to_string();
    workflow.nodes = vec![node];

    let artifact_root = super::new_artifact_root();
    let (params, checkpoint_sink) = interactive_run_params_with_sink(
        workflow.clone(),
        temp.path().to_path_buf(),
        SlowCompleteAi,
    );
    let (handle, mut event_rx, _action_tx, cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    let mut stopped = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if matches!(event, ExecutionEvent::NodeStarted { .. }) && !stopped {
            stopped = true;
            cancel.cancel();
        }
        if matches!(event, ExecutionEvent::Aborted) {
            break;
        }
    }
    handle.await.expect("drive task");
    assert!(stopped, "expected to stop during node execution");

    let checkpoint = checkpoint_sink.lock().clone().expect("checkpoint");
    let (resume_params, _) =
        interactive_run_params_with_sink(workflow, temp.path().to_path_buf(), SlowCompleteAi);
    let resume_params = InteractiveWorkflowRunParams {
        artifact_root,
        resume_checkpoint: Some(checkpoint),
        ..resume_params
    };
    let (handle, mut event_rx, _, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), resume_params);

    let mut completed = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if matches!(
            event,
            ExecutionEvent::NodeCompleted { ref node_id, .. } if node_id.0 == "idea"
        ) {
            completed = true;
            break;
        }
    }
    handle.await.expect("resume drive task");
    assert!(completed, "expected node to complete after continue");
}

fn write_tool_workflow() -> Workflow {
    let mut workflow = Workflow::new("write-approval");
    let mut node = engine::Node::agent("writer", 0.0, 0.0);
    node.id = NodeId("writer".to_string());
    node.agent.model = "test-model".to_string();
    node.agent.tools.catalog.tools = vec![ToolRef {
        name: "write".to_string(),
        tier: Some(ToolTier::Write),
    }];
    node.agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);
    workflow.nodes = vec![node];
    workflow
}

#[tokio::test]
async fn resolve_approval_uses_engine_node_id_after_stop_and_continue() {
    #[derive(Clone)]
    struct WriteToolAi;

    #[async_trait]
    impl AiPort for WriteToolAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            self.invoke_stream(request, &NoopStreamSink).await
        }

        async fn invoke_stream(
            &self,
            request: AgentRequest,
            _sink: &dyn AiStreamSink,
        ) -> Result<AgentTurnOutcome, AgentError> {
            if request.transcript.is_empty() {
                return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-write".to_string(),
                        name: "write".to_string(),
                        arguments: json!({"path": "out.txt", "content": "deny-me\n"}),
                    }],
                }));
            }
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done"}),
                raw_text: "{}".to_string(),
                assistant_message: None,
            }))
        }
    }

    struct NoopStreamSink;

    impl AiStreamSink for NoopStreamSink {
        fn on_stream_event(&self, _event: AiStreamEvent) {}
    }

    let temp = TempDir::new().expect("tempdir");
    let workflow = write_tool_workflow();
    let (params, checkpoint_sink) =
        interactive_run_params_with_sink(workflow.clone(), temp.path().to_path_buf(), WriteToolAi);
    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    let mut _approval_id = None;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if let ExecutionEvent::ToolApprovalRequested { request } = &event {
            _approval_id = Some(request.approval_id.clone());
            action_tx.send(ExecutionAction::Stop).expect("stop");
        }
        if matches!(event, ExecutionEvent::Aborted) {
            break;
        }
    }
    handle.await.expect("drive task");

    let checkpoint = checkpoint_sink.lock().clone().expect("checkpoint");
    let (resume_params, _) =
        interactive_run_params_with_sink(workflow, temp.path().to_path_buf(), WriteToolAi);
    let resume_params = InteractiveWorkflowRunParams {
        resume_checkpoint: Some(checkpoint),
        ..resume_params
    };
    let (handle, mut event_rx, action_tx, _cancel, _) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), resume_params);

    let mut denied_node_id = None;
    while let Ok(Some(event)) = timeout(Duration::from_secs(5), event_rx.recv()).await {
        if let ExecutionEvent::ToolApprovalRequested { request } = &event {
            action_tx
                .send(ExecutionAction::ResolveApproval {
                    approval_id: request.approval_id.clone(),
                    allow: false,
                    reason: Some("not now".to_string()),
                })
                .expect("deny");
        }
        if let ExecutionEvent::ToolDenied { node_id, .. } = &event {
            denied_node_id = Some(node_id.clone());
        }
        if matches!(event, ExecutionEvent::Finished(_)) {
            break;
        }
    }
    handle.await.expect("resume drive task");

    assert_eq!(denied_node_id, Some(NodeId("writer".to_string())));
}
