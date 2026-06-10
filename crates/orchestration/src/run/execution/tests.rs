use super::*;
use crate::run::state::{TraceStatus, WorkflowRunState};
use crate::tools::ToolRegistry;
use async_trait::async_trait;
use engine::{
    AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess,
    AiStreamEvent, AiStreamSink, ChatRole, NodeToolConfig, SubagentStatus, SubagentSummary,
    ToolCall, ToolCallStatus, ToolRef, ToolTier,
};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;

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
    let adapter = AiInvocationAdapter::new(Arc::new(StreamingNeedsInputAi), event_tx);
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
                intent: None,
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
                intent: None,
            },
        },
    );
    apply_event_to_run_state(
        &workflow,
        &mut state,
        ExecutionEvent::ToolApprovalRequested {
            request: engine::PendingToolApproval {
                approval_id: "approval-1".to_string(),
                node_id: "first".to_string(),
                node_label: "First".to_string(),
                tool_call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
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
                        intent: None,
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
                    intent: None,
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
