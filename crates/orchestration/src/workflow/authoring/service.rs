use crate::api::{
    WorkflowAuthoringMessage, WorkflowAuthoringTurnResult, WorkflowAuthoringValidation,
};
use crate::settings::model::AppSettings;
use crate::workflow::authoring::{
    layout_workflow_by_layers, materialize_authoring_draft, validate_authoring_workflow,
    WorkflowAuthoringDraft,
};
use engine::{
    AgentRequest, AgentTranscriptItem, AgentTurnOutcome, AgentTurnSuccess, AiPort, NodeId,
    Workflow, WorkflowId,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct WorkflowAuthoringSession {
    pub id: String,
    pub messages: Vec<WorkflowAuthoringMessage>,
    pub current_draft: Option<Workflow>,
}

pub struct WorkflowAuthoringService {
    sessions: Arc<tokio::sync::Mutex<HashMap<String, WorkflowAuthoringSession>>>,
}

impl WorkflowAuthoringService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn start_session(&self, base_workflow: Option<Workflow>) -> String {
        let id = Uuid::new_v4().to_string();
        let session = WorkflowAuthoringSession {
            id: id.clone(),
            messages: Vec::new(),
            current_draft: base_workflow,
        };
        if let Ok(mut sessions) = self.sessions.try_lock() {
            sessions.insert(id.clone(), session);
        } else {
            self.sessions.blocking_lock().insert(id.clone(), session);
        }
        id
    }

    pub fn get_session(&self, session_id: &str) -> Option<WorkflowAuthoringSession> {
        self.sessions
            .try_lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).cloned())
            .or_else(|| self.sessions.blocking_lock().get(session_id).cloned())
    }

    pub async fn send_turn<A: AiPort + Send + Sync>(
        &self,
        session_id: &str,
        user_message: String,
        settings: &AppSettings,
        ai: &A,
    ) -> Result<WorkflowAuthoringTurnResult, String> {
        {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| "authoring session not found".to_string())?;
            session.messages.push(WorkflowAuthoringMessage {
                role: "user".to_string(),
                content: user_message.clone(),
            });
        }

        let snapshot = {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| "authoring session not found".to_string())?;
            (session.messages.clone(), session.current_draft.clone())
        };
        let (messages, current_draft) = snapshot;

        let model = settings
            .active_profile()
            .default_model
            .clone()
            .unwrap_or_else(|| "gpt-5.5".to_string());

        let transcript: Vec<AgentTranscriptItem> = messages
            .iter()
            .map(|message| match message.role.as_str() {
                "assistant" => AgentTranscriptItem::AssistantMessage {
                    content: message.content.clone(),
                },
                _ => AgentTranscriptItem::UserMessage {
                    content: message.content.clone(),
                },
            })
            .collect();

        let base_context = current_draft
            .as_ref()
            .map(|workflow| serde_json::to_string_pretty(workflow).unwrap_or_default())
            .unwrap_or_default();

        let request = AgentRequest {
            workflow_id: WorkflowId::from("workflow-authoring"),
            node_id: NodeId::from("authoring"),
            node_label: "Workflow authoring".to_string(),
            model: model.clone(),
            system_messages: vec![authoring_system_prompt()],
            task_prompt: if base_context.is_empty() {
                "Create or update the workflow draft from the conversation.".to_string()
            } else {
                format!(
                    "Update the workflow draft from the conversation.\n\nCurrent draft JSON:\n{base_context}"
                )
            },
            input: json!({ "userMessage": user_message }),
            output_schema: authoring_output_schema(),
            tool_config: Default::default(),
            available_tools: Vec::new(),
            transcript,
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
        };

        let outcome = ai
            .invoke(request)
            .await
            .map_err(|error| error.to_string())?;

        let output = match outcome {
            AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. }) => output,
            AgentTurnOutcome::NeedsUserInput(need) => {
                return Ok(WorkflowAuthoringTurnResult {
                    session_id: session_id.to_string(),
                    assistant_message: need.assistant_message,
                    draft: current_draft,
                    validation: WorkflowAuthoringValidation {
                        valid: false,
                        errors: vec!["Model requested clarification instead of a draft".to_string()],
                        warnings: Vec::new(),
                        dag: None,
                    },
                    messages,
                });
            }
            AgentTurnOutcome::ToolCalls(_) => {
                return Err("authoring model attempted tool calls".to_string());
            }
        };

        let assistant_message = output
            .get("assistantMessage")
            .and_then(|value| value.as_str())
            .unwrap_or("Updated workflow draft.")
            .to_string();

        let draft_value = output
            .get("workflowDraft")
            .ok_or_else(|| "missing workflowDraft in model output".to_string())?;

        let draft: WorkflowAuthoringDraft = serde_json::from_value(draft_value.clone())
            .map_err(|error| format!("invalid workflowDraft: {error}"))?;

        let base_id = current_draft.as_ref().map(|workflow| workflow.id.clone());
        let mut workflow = materialize_authoring_draft(draft, base_id, &model);
        layout_workflow_by_layers(&mut workflow)
            .map_err(|error| format!("layout failed: {error}"))?;
        let validation = validate_authoring_workflow(&workflow);

        let mut messages = messages;
        messages.push(WorkflowAuthoringMessage {
            role: "assistant".to_string(),
            content: assistant_message.clone(),
        });

        {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| "authoring session not found".to_string())?;
            session.messages = messages.clone();
            session.current_draft = Some(workflow.clone());
        }

        Ok(WorkflowAuthoringTurnResult {
            session_id: session_id.to_string(),
            assistant_message,
            draft: Some(workflow),
            validation,
            messages,
        })
    }
}

impl Default for WorkflowAuthoringService {
    fn default() -> Self {
        Self::new()
    }
}

fn authoring_system_prompt() -> String {
    include_str!("prompts/workflow_authoring_system.txt").to_string()
}

fn authoring_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "assistantMessage": { "type": "string" },
            "workflowDraft": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string" },
                    "sharedContext": { "type": "string" },
                    "nodes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string" },
                                "label": { "type": "string" },
                                "systemPrompt": { "type": "string" },
                                "taskPrompt": { "type": "string" },
                                "outputSchema": { "type": "object" },
                                "autoStart": { "type": "boolean" }
                            },
                            "required": ["id", "label", "systemPrompt", "taskPrompt"]
                        }
                    },
                    "edges": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string" },
                                "from": { "type": "string" },
                                "to": { "type": "string" }
                            },
                            "required": ["id", "from", "to"]
                        }
                    }
                },
                "required": ["name", "nodes", "edges"]
            }
        },
        "required": ["assistantMessage", "workflowDraft"]
    })
}
