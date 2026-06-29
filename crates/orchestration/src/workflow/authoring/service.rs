use crate::api::{
    WorkflowAuthoringMessage, WorkflowAuthoringTurnResult, WorkflowAuthoringValidation,
};
use crate::settings::model::AppSettings;
use crate::workflow::authoring::{
    layout_workflow_by_layers, materialize_authoring_draft, validate_authoring_workflow,
    WorkflowAuthoringDraft,
};
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, NodeId, Workflow, WorkflowId,
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

        let mut transcript: Vec<AgentTranscriptItem> = messages
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

        let mut model_attempt = 1u8;
        let mut malformed_submit_retries = 0u8;
        let output = loop {
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
                transcript: transcript.clone(),
                model_attempt,
                reasoning_effort: None,
                reasoning_budget_tokens: None,
            };

            match ai.invoke(request).await {
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. })) => break output,
                Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    assistant_message,
                    ..
                })) if model_attempt <= MAX_AUTHORING_CLARIFICATION_RETRIES => {
                    transcript.push(AgentTranscriptItem::AssistantMessage {
                        content: assistant_message,
                    });
                    transcript.push(AgentTranscriptItem::UserMessage {
                        content: AUTHORING_DRAFT_REQUIRED_FEEDBACK.to_string(),
                    });
                    model_attempt += 1;
                }
                Ok(AgentTurnOutcome::NeedsUserInput(need)) => {
                    let assistant_message = need.assistant_message;
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
                    }
                    return Ok(WorkflowAuthoringTurnResult {
                        session_id: session_id.to_string(),
                        assistant_message,
                        draft: current_draft,
                        validation: WorkflowAuthoringValidation {
                            valid: false,
                            errors: vec![
                                "Model requested clarification instead of a draft".to_string()
                            ],
                            warnings: Vec::new(),
                            dag: None,
                        },
                        messages,
                    });
                }
                Ok(AgentTurnOutcome::ToolCalls(_)) => {
                    return Err("authoring model attempted tool calls".to_string());
                }
                Err(error)
                    if error.is_malformed_submit_output()
                        && malformed_submit_retries < MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES =>
                {
                    malformed_submit_retries += 1;
                    model_attempt += 1;
                    transcript.push(AgentTranscriptItem::UserMessage {
                        content: malformed_submit_output_feedback(&error),
                    });
                }
                Err(error) => return Err(error.to_string()),
            }
        };

        let assistant_message = output
            .get("assistantMessage")
            .or_else(|| output.get("assistant_message"))
            .and_then(|value| value.as_str())
            .unwrap_or("Updated workflow draft.")
            .to_string();

        let draft_value = extract_workflow_draft_from_output(&output)?;

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

fn extract_workflow_draft_from_output(
    output: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    if let Some(draft) = output
        .get("workflowDraft")
        .or_else(|| output.get("workflow_draft"))
    {
        return Ok(draft.clone());
    }

    let Some(map) = output.as_object() else {
        return Err("missing workflowDraft in model output".to_string());
    };

    if map.contains_key("name") && map.contains_key("nodes") {
        let mut draft = map.clone();
        draft.remove("assistantMessage");
        draft.remove("assistant_message");
        return Ok(serde_json::Value::Object(draft));
    }

    Err(
        "missing workflowDraft in model output — the model must include a workflowDraft object with name, nodes, and edges"
            .to_string(),
    )
}

const MAX_AUTHORING_CLARIFICATION_RETRIES: u8 = 1;
const MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES: u8 = 3;
const AUTHORING_DRAFT_REQUIRED_FEEDBACK: &str = "You must call submit_output with a complete workflowDraft. Do not ask clarifying questions — make reasonable assumptions and produce the best draft you can from what you have.";

fn malformed_submit_output_feedback(error: &AgentError) -> String {
    format!(
        "Your openflow_submit_node_output call was invalid ({error}). \
         Call openflow_submit_node_output again with arguments shaped as \
         {{\"output\": {{\"assistantMessage\": \"...\", \"workflowDraft\": {{...}}}}, \"assistant_message\": null}}. \
         Put schema fields under \"output\", not at the top level."
    )
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
