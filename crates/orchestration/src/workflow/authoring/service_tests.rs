use super::WorkflowAuthoringService;
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

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
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

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
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

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
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

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
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
