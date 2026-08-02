use super::{WorkflowAuthoringProjectContext, WorkflowAuthoringService};
use crate::api::WorkflowAuthoringRole;
use crate::settings::model::AppSettings;
use async_trait::async_trait;
use engine::{
    AgentError, AgentMessageTurn, AgentNeedUserInput, AgentRequest, AgentToolCallBatch,
    AgentTranscriptItem, AgentTurnOutcome, AgentTurnSuccess, AiPort, ToolCall, Workflow,
    WorkflowId,
};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MockAuthoringAi {
    response: serde_json::Value,
}

#[async_trait]
impl AiPort for MockAuthoringAi {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: None,
            output: self.response.clone(),
            raw_text: self.response.to_string(),
            assistant_message: Some("Built draft".to_string()),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

struct CapturingPromptAi {
    response: serde_json::Value,
    system_messages: std::sync::Mutex<Vec<String>>,
}

#[async_trait]
impl AiPort for CapturingPromptAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        *self.system_messages.lock().expect("system messages lock") = request.system_messages;
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: None,
            output: self.response.clone(),
            raw_text: self.response.to_string(),
            assistant_message: Some("Built draft".to_string()),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_materializes_valid_draft() {
    let ai = MockAuthoringAi {
        response: json!({
            "assistantMessage": "Here is a two-step workflow.",
            "workflowDraft": {
                "name": "Demo",
                "sharedContext": "",
                "nodes": [
                    {
                        "id": "root",
                        "label": "Root",
                        "systemPrompt": "You are root.",
                        "taskPrompt": "Summarize the idea.",
                        "outputSchema": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": { "summary": { "type": "string" } },
                            "required": ["summary"]
                        },
                        "autoStart": true
                    },
                    {
                        "id": "plan",
                        "label": "Plan",
                        "systemPrompt": "You plan.",
                        "taskPrompt": "Plan from upstream.",
                        "outputSchema": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": { "steps": { "type": "array", "items": { "type": "string" } } },
                            "required": ["steps"]
                        },
                        "autoStart": true
                    }
                ],
                "edges": [{ "id": "root-plan", "from": "root", "to": "plan" }]
            }
        }),
    };

    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
    assert!(result.draft_changed);
}

struct NaturalConversationAi;

#[async_trait]
impl AiPort for NaturalConversationAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        assert!(request.conversation_mode);
        assert!(request
            .available_tools
            .iter()
            .all(|tool| !tool.name.starts_with("openflow_")));
        Ok(AgentTurnOutcome::Message(AgentMessageTurn {
            raw_text: "MCPs can provide live market data and news.".to_string(),
            assistant_message: "MCPs can provide live market data and news.".to_string(),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_keeps_informational_questions_in_chat() {
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let initial_draft = service
        .get_session(&session_id)
        .expect("authoring session")
        .current_draft;

    let result = service
        .send_turn(
            &session_id,
            "What MCPs can improve the information available?".to_string(),
            &AppSettings::default(),
            &NaturalConversationAi,
            |_| {},
            |_| {},
        )
        .await
        .expect("conversational turn");

    assert_eq!(
        result.assistant_message,
        "MCPs can provide live market data and news."
    );
    assert_eq!(result.draft, initial_draft);
    assert!(!result.draft_changed);
    assert_eq!(result.messages.len(), 2);
}

struct ProposalThenToolsAi {
    calls: AtomicUsize,
    first_completed: bool,
}

#[async_trait]
impl AiPort for ProposalThenToolsAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(request
            .available_tools
            .iter()
            .any(|tool| tool.name == "openflow_set_workflow_meta"));
        match call {
            0 if self.first_completed => Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: json!({
                    "assistantMessage": "I'll propose a risk-controlled workflow. Please review and accept it in the UI."
                }),
                raw_text: String::new(),
                assistant_message: Some(
                    "I'll propose a risk-controlled workflow. Please review and accept it in the UI."
                        .to_string(),
                ),
                reasoning: Vec::new(),
                usage: None,
            })),
            0 => Ok(AgentTurnOutcome::Message(AgentMessageTurn {
                raw_text: "I'll propose a risk-controlled workflow. Please review and accept it in the UI."
                    .to_string(),
                assistant_message: "I'll propose a risk-controlled workflow. Please review and accept it in the UI."
                    .to_string(),
                reasoning: Vec::new(),
                usage: None,
            })),
            1 => Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![ToolCall {
                    id: "set-weekly-options-name".to_string(),
                    provider_call_id: None,
                    name: "openflow_set_workflow_meta".to_string(),
                    arguments: json!({ "name": "Weekly Options Research" }),
                }],
                reasoning: Vec::new(),
                usage: None,
            })),
            2 => Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: json!({ "assistantMessage": "Proposed a workflow for review." }),
                raw_text: String::new(),
                assistant_message: Some("Proposed a workflow for review.".to_string()),
                reasoning: Vec::new(),
                usage: None,
            })),
            _ => panic!("unexpected proposal authoring invoke count {call}"),
        }
    }
}

async fn assert_proposal_text_requires_a_real_draft(first_completed: bool) {
    let ai = ProposalThenToolsAi {
        calls: AtomicUsize::new(0),
        first_completed,
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let result = service
        .send_turn(
            &session_id,
            "Create a risk-controlled weekly-options research workflow with validated market data, parallel analysis, red-team review, a risk committee, and a human approval gate.".to_string(),
            &AppSettings::default(),
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("proposal turn");

    assert!(result.draft_changed);
    assert_eq!(
        result.draft.as_ref().expect("draft").name,
        "Weekly Options Research"
    );
    assert_eq!(ai.calls.load(Ordering::SeqCst), 3);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn submitted_proposal_text_is_retried_until_a_real_draft_exists() {
    assert_proposal_text_requires_a_real_draft(true).await;
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn plain_proposal_text_is_retried_until_a_real_draft_exists() {
    assert_proposal_text_requires_a_real_draft(false).await;
}

#[test]
fn only_direct_workflow_change_requests_enable_authoring_tools() {
    assert!(!super::service::explicit_workflow_change_request(
        "What MCPs can improve the information available?"
    ));
    assert!(!super::service::explicit_workflow_change_request(
        "How do I create a workflow?"
    ));
    assert!(!super::service::explicit_workflow_change_request(
        "Can you explain how to edit a workflow?"
    ));
    assert!(!super::service::explicit_workflow_change_request(
        "I want to understand how to update this workflow."
    ));
    assert!(!super::service::explicit_workflow_change_request(
        "Please explain how to modify the workflow."
    ));
    assert!(super::service::explicit_workflow_change_request(
        "Please add an MCP research node to the workflow."
    ));
    assert!(super::service::explicit_workflow_change_request(
        "Build a simple planner"
    ));
    assert!(super::service::explicit_workflow_change_request(
        "Can you update the current workflow?"
    ));
    assert!(super::service::explicit_workflow_change_request(
        "Revise the current workflow draft based on the run."
    ));
    assert!(super::service::explicit_workflow_change_request(
        "Explain this, then update the workflow."
    ));
    assert!(super::service::explicit_workflow_change_request(
        "I want to understand the current draft and revise the workflow."
    ));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn project_authoring_uses_project_specific_preamble() {
    let project_dir = tempfile::tempdir().expect("project tempdir");
    let project_path = project_dir.path().display().to_string();
    let ai = CapturingPromptAi {
        response: single_node_draft("Repo Flow", "root", "Root"),
        system_messages: std::sync::Mutex::new(Vec::new()),
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service
        .start_project_session(
            None,
            WorkflowAuthoringProjectContext {
                id: "project-1".to_string(),
                name: "OpenFlow".to_string(),
                path: project_path.clone(),
                default_execution_cwd: Some(project_path.clone()),
            },
        )
        .session_id;
    let settings = AppSettings::default();

    service
        .send_turn(
            &session_id,
            "Build a repo triage workflow".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    let prompt = ai
        .system_messages
        .lock()
        .expect("system messages lock")
        .join("\n\n");
    assert!(prompt.contains("You are creating a workflow for an OpenFlow project."));
    assert!(prompt.contains("Project name: OpenFlow"));
    assert!(prompt.contains(&format!("Project path: {project_path}")));
    assert!(prompt.contains(&format!("Default execution cwd: {project_path}")));
    assert!(prompt.contains("Read-only project tools are available"));
    assert!(prompt.contains("Answer informational questions, explain concepts"));
    assert!(prompt.contains("ask a concise clarifying question and wait"));
    assert!(prompt.contains("requestUserInput: false for autonomous planning, coding"));
    assert!(prompt.contains("openflow_add_node"));
}

struct ProjectReadToolsAi {
    calls: AtomicUsize,
}

#[async_trait]
impl AiPort for ProjectReadToolsAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => {
                let tool_names = request
                    .available_tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect::<Vec<_>>();
                assert!(tool_names.contains(&"read"));
                assert!(tool_names.contains(&"search"));
                assert!(tool_names.contains(&"find"));
                assert!(!tool_names.contains(&"write"));
                Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "read-project-manifest".to_string(),
                        provider_call_id: None,
                        name: "read".to_string(),
                        arguments: json!({ "path": "PROJECT.md" }),
                    }],
                    reasoning: Vec::new(),
                    usage: None,
                }))
            }
            1 => {
                assert!(
                    request.transcript.iter().any(|item| {
                        matches!(
                            item,
                            AgentTranscriptItem::ToolResult { result }
                                if result.tool_name == "read"
                                    && !result.is_error
                                    && result.content.contains("Project-specific workflow guidance")
                        )
                    }),
                    "expected project file content in the authoring transcript"
                );
                let output = single_node_draft("Repo-aware Flow", "root", "Root");
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    handoff: None,
                    output: output.clone(),
                    raw_text: output.to_string(),
                    assistant_message: Some("Built a repo-aware workflow.".to_string()),
                    reasoning: Vec::new(),
                    usage: None,
                }))
            }
            _ => panic!("unexpected project authoring invoke count {call}"),
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn project_authoring_exposes_and_executes_read_tools() {
    let project_dir = tempfile::tempdir().expect("project tempdir");
    std::fs::write(
        project_dir.path().join("PROJECT.md"),
        "# Project-specific workflow guidance\n",
    )
    .expect("seed project guidance");
    let ai = ProjectReadToolsAi {
        calls: AtomicUsize::new(0),
    };
    let service = WorkflowAuthoringService::new();
    let project_path = project_dir.path().display().to_string();
    let session_id = service
        .start_project_session(
            None,
            WorkflowAuthoringProjectContext {
                id: "project-1".to_string(),
                name: "Project".to_string(),
                path: project_path.clone(),
                default_execution_cwd: Some(project_path),
            },
        )
        .session_id;

    let result = service
        .send_turn(
            &session_id,
            "Build a workflow for this repo".to_string(),
            &AppSettings::default(),
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("project authoring turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
}

struct IncrementalAuthoringAi {
    calls: AtomicUsize,
}

#[async_trait]
impl AiPort for IncrementalAuthoringAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![
                    ToolCall {
                        id: "call-meta".to_string(),
                        provider_call_id: None,
                        name: "openflow_set_workflow_meta".to_string(),
                        arguments: json!({ "name": "Demo" }),
                    },
                    ToolCall {
                        id: "call-root".to_string(),
                        provider_call_id: None,
                        name: "openflow_add_node".to_string(),
                        arguments: json!({
                            "id": "root",
                            "label": "Root",
                            "systemPrompt": "You are root.",
                            "taskPrompt": "Summarize the idea.",
                            "autoStart": true
                        }),
                    },
                ],
                reasoning: vec![],
                usage: None,
            })),
            1 => Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: String::new(),
                assistant_message: None,
                tool_calls: vec![
                    ToolCall {
                        id: "call-plan".to_string(),
                        provider_call_id: None,
                        name: "openflow_add_node".to_string(),
                        arguments: json!({
                            "id": "plan",
                            "label": "Plan",
                            "systemPrompt": "You plan.",
                            "taskPrompt": "Plan from upstream.",
                            "autoStart": true
                        }),
                    },
                    ToolCall {
                        id: "call-edge".to_string(),
                        provider_call_id: None,
                        name: "openflow_add_edge".to_string(),
                        arguments: json!({ "id": "root-plan", "from": "root", "to": "plan" }),
                    },
                ],
                reasoning: vec![],
                usage: None,
            })),
            2 => {
                assert!(
                    request
                        .available_tools
                        .iter()
                        .any(|tool| tool.name == "openflow_add_node"),
                    "expected authoring tools on request"
                );
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    handoff: None,
                    output: json!({ "assistantMessage": "Built a two-step workflow." }),
                    raw_text: String::new(),
                    assistant_message: Some("Built a two-step workflow.".to_string()),
                    reasoning: Vec::new(),
                    usage: None,
                }))
            }
            _ => panic!("unexpected authoring invoke count {call}"),
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_builds_draft_via_incremental_authoring_tools() {
    let ai = IncrementalAuthoringAi {
        calls: AtomicUsize::new(0),
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service
        .start_session(Some(empty_authoring_base()))
        .session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
    assert!(result.draft_changed);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 3);
}

struct MixedToolTurnRetryAi {
    calls: AtomicUsize,
}

#[async_trait]
impl AiPort for MixedToolTurnRetryAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match call {
            0 => Err(AgentError::mixed_tool_turn(
                "Custom OpenAI-compatible API",
                "openflow_submit_node_output, openflow_add_node",
            )),
            1 => {
                assert!(
                    request.transcript.iter().any(|item| {
                        matches!(
                            item,
                            AgentTranscriptItem::UserMessage { content, .. }
                                if content.contains("mixed finish/submit tools and authoring tools")
                                    && content.contains("openflow_submit_node_output, openflow_add_node")
                        )
                    }),
                    "expected mixed-tool-turn feedback in transcript"
                );
                Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![
                        ToolCall {
                            id: "call-meta".to_string(),
                            provider_call_id: None,
                            name: "openflow_set_workflow_meta".to_string(),
                            arguments: json!({ "name": "Demo" }),
                        },
                        ToolCall {
                            id: "call-root".to_string(),
                            provider_call_id: None,
                            name: "openflow_add_node".to_string(),
                            arguments: json!({
                                "id": "root",
                                "label": "Root",
                                "systemPrompt": "You are root.",
                                "taskPrompt": "Summarize the idea.",
                                "autoStart": true
                            }),
                        },
                    ],
                    reasoning: vec![],
                    usage: None,
                }))
            }
            2 => Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: json!({ "assistantMessage": "Built a one-step workflow." }),
                raw_text: String::new(),
                assistant_message: Some("Built a one-step workflow.".to_string()),
                reasoning: Vec::new(),
                usage: None,
            })),
            _ => panic!("unexpected authoring invoke count {call}"),
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_retries_mixed_tool_turn_and_materializes_draft() {
    let ai = MixedToolTurnRetryAi {
        calls: AtomicUsize::new(0),
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service
        .start_session(Some(empty_authoring_base()))
        .session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 1);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 3);
}

fn empty_authoring_base() -> Workflow {
    Workflow {
        id: WorkflowId::from("authoring-test-base"),
        name: "Scratch".to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
        settings: engine::WorkflowSettings::default(),
    }
}

fn single_node_draft(name: &str, node_id: &str, label: &str) -> serde_json::Value {
    json!({
        "assistantMessage": format!("Built {name}."),
        "workflowDraft": {
            "name": name,
            "sharedContext": "",
            "nodes": [{
                "id": node_id,
                "label": label,
                "systemPrompt": "You are helpful.",
                "taskPrompt": "Do the work.",
                "outputSchema": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": { "result": { "type": "string" } },
                    "required": ["result"]
                },
                "autoStart": true
            }],
            "edges": []
        }
    })
}

struct MultiTurnMockAi {
    calls: AtomicUsize,
}

#[async_trait]
impl AiPort for MultiTurnMockAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let output = if call == 0 {
            assert_eq!(request.transcript.len(), 1);
            single_node_draft("Draft v1", "root", "Root")
        } else {
            assert_eq!(request.transcript.len(), 3);
            assert!(request.task_prompt.contains("Draft v1"));
            single_node_draft("Draft v2", "root", "Root Updated")
        };
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: None,
            output: output.clone(),
            raw_text: output.to_string(),
            assistant_message: Some("Updated draft".to_string()),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_preserves_session_for_follow_up_messages() {
    let ai = MultiTurnMockAi {
        calls: AtomicUsize::new(0),
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();

    let first = service
        .send_turn(
            &session_id,
            "Build a one-node workflow".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("first turn");
    assert_eq!(first.messages.len(), 2);
    assert_eq!(first.draft.as_ref().expect("draft").name, "Draft v1");

    let second = service
        .send_turn(
            &session_id,
            "Rename the root node".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("second turn");
    assert_eq!(second.messages.len(), 4);
    assert_eq!(second.draft.as_ref().expect("draft").name, "Draft v2");
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_accepts_flat_draft_fields_in_output() {
    let ai = MockAuthoringAi {
        response: json!({
            "assistantMessage": "Built a flat draft.",
            "name": "Demo",
            "sharedContext": "",
            "nodes": [
                {
                    "id": "root",
                    "label": "Root",
                    "systemPrompt": "You are root.",
                    "taskPrompt": "Summarize the idea.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "summary": { "type": "string" } },
                        "required": ["summary"]
                    },
                    "autoStart": true
                }
            ],
            "edges": []
        }),
    };

    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a one-node workflow".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 1);
}

struct ClarificationThenDraftAi {
    calls: AtomicUsize,
    draft_response: serde_json::Value,
}

#[async_trait]
impl AiPort for ClarificationThenDraftAi {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: "What kind of workflow?".to_string(),
                assistant_message: "What kind of workflow do you want?".to_string(),
                structured_input: None,
                reasoning: vec![],
            }))
        } else {
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: self.draft_response.clone(),
                raw_text: self.draft_response.to_string(),
                assistant_message: Some("Built draft".to_string()),
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_returns_clarification_for_the_next_user_turn() {
    let draft_response = json!({
        "assistantMessage": "Here is a two-step workflow.",
        "workflowDraft": {
            "name": "Demo",
            "sharedContext": "",
            "nodes": [
                {
                    "id": "root",
                    "label": "Root",
                    "systemPrompt": "You are root.",
                    "taskPrompt": "Summarize the idea.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "summary": { "type": "string" } },
                        "required": ["summary"]
                    },
                    "autoStart": true
                },
                {
                    "id": "plan",
                    "label": "Plan",
                    "systemPrompt": "You plan.",
                    "taskPrompt": "Plan from upstream.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "steps": { "type": "array", "items": { "type": "string" } } },
                        "required": ["steps"]
                    },
                    "autoStart": true
                }
            ],
            "edges": [{ "id": "root-plan", "from": "root", "to": "plan" }]
        }
    });
    let ai = ClarificationThenDraftAi {
        calls: AtomicUsize::new(0),
        draft_response,
    };

    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 4);
    assert!(!result.draft_changed);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 1);
}

struct AlwaysClarifyAi;

#[async_trait]
impl AiPort for AlwaysClarifyAi {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "What kind of workflow?".to_string(),
            assistant_message: "What kind of workflow do you want?".to_string(),
            structured_input: None,
            reasoning: vec![],
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_returns_assistant_message_when_model_requests_clarification() {
    let ai = AlwaysClarifyAi;
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[0].role, WorkflowAuthoringRole::User);
    assert_eq!(result.messages[0].content, "Build a simple planner");
    assert_eq!(result.messages[1].role, WorkflowAuthoringRole::Assistant);
    assert_eq!(
        result.messages[1].content,
        "What kind of workflow do you want?"
    );
    assert_eq!(
        result.assistant_message,
        "What kind of workflow do you want?"
    );
    assert!(result.validation.valid);
    assert!(!result.draft_changed);
}

struct MalformedSubmitThenDraftAi {
    calls: AtomicUsize,
    draft_response: serde_json::Value,
}

#[async_trait]
impl AiPort for MalformedSubmitThenDraftAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Err(AgentError::malformed_submit_output(
                "OpenAI-compatible",
                "missing field `output`",
            ));
        }
        assert!(
            request
                .transcript
                .iter()
                .any(|item| matches!(item, AgentTranscriptItem::UserMessage { content, .. } if content.contains("openflow_submit_node_output"))),
            "expected malformed-submit feedback in transcript"
        );
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: None,
            output: self.draft_response.clone(),
            raw_text: self.draft_response.to_string(),
            assistant_message: Some("Built draft".to_string()),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_retries_missing_submit_output_and_materializes_draft() {
    struct MissingSubmitThenDraftAi {
        calls: AtomicUsize,
        draft_response: serde_json::Value,
    }

    #[async_trait]
    impl AiPort for MissingSubmitThenDraftAi {
        async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                return Err(AgentError::Failed(
                    "provider returned neither tool calls nor recoverable output".to_string(),
                ));
            }
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: self.draft_response.clone(),
                raw_text: self.draft_response.to_string(),
                assistant_message: Some("Built draft".to_string()),
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    let draft_response = json!({
        "assistantMessage": "Here is a two-step workflow.",
        "workflowDraft": {
            "name": "Demo",
            "sharedContext": "",
            "nodes": [
                {
                    "id": "root",
                    "label": "Root",
                    "systemPrompt": "You are root.",
                    "taskPrompt": "Summarize the idea.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "summary": { "type": "string" } },
                        "required": ["summary"]
                    },
                    "autoStart": true
                }
            ],
            "edges": []
        }
    });
    let ai = MissingSubmitThenDraftAi {
        calls: AtomicUsize::new(0),
        draft_response,
    };

    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid);
    assert!(
        result.messages.iter().any(|message| {
            message.role == WorkflowAuthoringRole::Thinking
                && message.content.contains("no submit output")
        }),
        "expected a thinking message describing the retry"
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_retries_malformed_submit_output_and_materializes_draft() {
    let draft_response = json!({
        "assistantMessage": "Here is a two-step workflow.",
        "workflowDraft": {
            "name": "Demo",
            "sharedContext": "",
            "nodes": [
                {
                    "id": "root",
                    "label": "Root",
                    "systemPrompt": "You are root.",
                    "taskPrompt": "Summarize the idea.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "summary": { "type": "string" } },
                        "required": ["summary"]
                    },
                    "autoStart": true
                },
                {
                    "id": "plan",
                    "label": "Plan",
                    "systemPrompt": "You plan.",
                    "taskPrompt": "Plan from upstream.",
                    "outputSchema": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": { "steps": { "type": "array", "items": { "type": "string" } } },
                        "required": ["steps"]
                    },
                    "autoStart": true
                }
            ],
            "edges": [{ "id": "root-plan", "from": "root", "to": "plan" }]
        }
    });
    let ai = MalformedSubmitThenDraftAi {
        calls: AtomicUsize::new(0),
        draft_response,
    };

    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
}

struct InvalidDraftThenValidAi {
    calls: AtomicUsize,
}

#[async_trait]
impl AiPort for InvalidDraftThenValidAi {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let output = if call == 0 {
            // Missing `edges` — must trigger a correction retry, not a hard failure.
            json!({
                "assistantMessage": "Draft without edges.",
                "workflowDraft": {
                    "name": "Demo",
                    "nodes": [{
                        "id": "root",
                        "label": "Root",
                        "systemPrompt": "You are root.",
                        "taskPrompt": "Do the work."
                    }]
                }
            })
        } else {
            assert!(
                request.transcript.iter().any(|item| matches!(
                    item,
                    AgentTranscriptItem::UserMessage { content, .. }
                        if content.contains("missing field `edges`")
                )),
                "expected invalid-draft feedback in transcript"
            );
            single_node_draft("Demo", "root", "Root")
        };
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            handoff: None,
            output: output.clone(),
            raw_text: output.to_string(),
            assistant_message: Some("Built draft".to_string()),
            reasoning: Vec::new(),
            usage: None,
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_retries_invalid_draft_until_it_converges() {
    let ai = InvalidDraftThenValidAi {
        calls: AtomicUsize::new(0),
    };
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a one-node workflow".to_string(),
            &settings,
            &ai,
            |_| {},
            |_| {},
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
    assert!(
        result
            .messages
            .iter()
            .any(|message| message.role == WorkflowAuthoringRole::Thinking),
        "expected a thinking message describing the retry"
    );
}

#[test]
fn start_session_seeds_feature_plan_template_when_no_base() {
    let service = WorkflowAuthoringService::new();
    let started = service.start_session(None);
    assert_eq!(started.draft.as_ref().expect("draft").nodes.len(), 4);
    assert_eq!(
        started.draft.as_ref().expect("draft").name,
        "Untitled workflow"
    );
    let session = service.get_session(&started.session_id).expect("session");
    assert_eq!(
        session.current_draft.as_ref().expect("draft").nodes.len(),
        4
    );
}

#[test]
fn end_session_removes_authoring_session() {
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    assert!(service.get_session(&session_id).is_some());
    assert!(service.end_session(&session_id));
    assert!(service.get_session(&session_id).is_none());
    assert!(!service.end_session(&session_id));
}

#[test]
fn start_session_evicts_oldest_when_at_capacity() {
    let service = WorkflowAuthoringService::new();
    let mut ids = Vec::with_capacity(65);
    ids.push(service.start_session(None).session_id);
    for _ in 1..64 {
        ids.push(service.start_session(None).session_id);
    }
    assert_eq!(service.session_count(), 64);
    let latest = service.start_session(None).session_id;
    assert_eq!(service.session_count(), 64);
    let remaining = ids
        .iter()
        .filter(|id| service.get_session(id).is_some())
        .count();
    assert_eq!(remaining, 63);
    assert!(service.get_session(&latest).is_some());
}
