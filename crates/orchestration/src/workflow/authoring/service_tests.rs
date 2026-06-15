use super::WorkflowAuthoringService;
use crate::settings::model::AppSettings;
use async_trait::async_trait;
use engine::{AgentRequest, AgentTurnOutcome, AgentTurnSuccess, AiPort, AgentError};
use serde_json::json;

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
        }))
    }
}

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

    let mut service = WorkflowAuthoringService::new();
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
