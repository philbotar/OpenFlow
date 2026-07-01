use super::WorkflowAuthoringService;
use crate::api::WorkflowAuthoringRole;
use crate::settings::model::AppSettings;
use async_trait::async_trait;
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
    AgentTurnSuccess, AiPort,
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
            output: self.response.clone(),
            raw_text: self.response.to_string(),
            assistant_message: Some("Built draft".to_string()),
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
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
        )
        .await
        .expect("turn");

    assert!(result.validation.valid);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
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
            output: output.clone(),
            raw_text: output.to_string(),
            assistant_message: Some("Updated draft".to_string()),
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
    let session_id = service.start_session(None);
    let settings = AppSettings::default();

    let first = service
        .send_turn(
            &session_id,
            "Build a one-node workflow".to_string(),
            &settings,
            &ai,
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
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a one-node workflow".to_string(),
            &settings,
            &ai,
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
            }))
        } else {
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: self.draft_response.clone(),
                raw_text: self.draft_response.to_string(),
                assistant_message: Some("Built draft".to_string()),
                usage: None,
            }))
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_retries_clarification_and_materializes_draft() {
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
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
}

struct AlwaysClarifyAi;

#[async_trait]
impl AiPort for AlwaysClarifyAi {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            raw_text: "What kind of workflow?".to_string(),
            assistant_message: "What kind of workflow do you want?".to_string(),
        }))
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn send_turn_returns_assistant_message_when_clarification_exhausted() {
    let ai = AlwaysClarifyAi;
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
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
    assert!(!result.validation.valid);
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
            return Err(AgentError::Failed(
                "OpenAI-compatible final output tool arguments were not valid JSON: missing field `output`"
                    .to_string(),
            ));
        }
        assert!(
            request
                .transcript
                .iter()
                .any(|item| matches!(item, AgentTranscriptItem::UserMessage { content } if content.contains("openflow_submit_node_output"))),
            "expected malformed-submit feedback in transcript"
        );
        Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
            output: self.draft_response.clone(),
            raw_text: self.draft_response.to_string(),
            assistant_message: Some("Built draft".to_string()),
            usage: None,
        }))
    }
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
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let result = service
        .send_turn(
            &session_id,
            "Build a simple planner".to_string(),
            &settings,
            &ai,
        )
        .await
        .expect("turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert_eq!(result.draft.as_ref().expect("draft").nodes.len(), 2);
    assert_eq!(ai.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn end_session_removes_authoring_session() {
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None);
    assert!(service.get_session(&session_id).is_some());
    assert!(service.end_session(&session_id));
    assert!(service.get_session(&session_id).is_none());
    assert!(!service.end_session(&session_id));
}

#[test]
fn start_session_evicts_oldest_when_at_capacity() {
    let service = WorkflowAuthoringService::new();
    let mut ids = Vec::with_capacity(65);
    ids.push(service.start_session(None));
    for _ in 1..64 {
        ids.push(service.start_session(None));
    }
    assert_eq!(service.session_count(), 64);
    let latest = service.start_session(None);
    assert_eq!(service.session_count(), 64);
    let remaining = ids
        .iter()
        .filter(|id| service.get_session(id).is_some())
        .count();
    assert_eq!(remaining, 63);
    assert!(service.get_session(&latest).is_some());
}
