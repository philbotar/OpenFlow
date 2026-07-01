use crate::api::{
    WorkflowAuthoringMessage, WorkflowAuthoringRole, WorkflowAuthoringTurnResult,
    WorkflowAuthoringValidation,
};
use crate::settings::model::AppSettings;
use crate::workflow::authoring::{
    layout_workflow_by_layers, materialize_authoring_draft, validate_authoring_workflow,
    workflow_draft_value_from_model_output, AuthoringError, WorkflowAuthoringDraft,
};
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, NodeId, Workflow, WorkflowId,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const MAX_AUTHORING_SESSIONS: usize = 64;

#[derive(Clone)]
pub struct WorkflowAuthoringSession {
    pub id: String,
    pub messages: Vec<WorkflowAuthoringMessage>,
    pub current_draft: Option<Workflow>,
}

pub struct WorkflowAuthoringService {
    // ponytail: std mutex; lock only in brief scopes, never held across ai.invoke().await
    sessions: Arc<Mutex<HashMap<String, WorkflowAuthoringSession>>>,
}

impl WorkflowAuthoringService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .expect("authoring sessions mutex poisoned")
            .len()
    }

    pub fn start_session(&self, base_workflow: Option<Workflow>) -> String {
        let id = Uuid::new_v4().to_string();
        let session = WorkflowAuthoringSession {
            id: id.clone(),
            messages: Vec::new(),
            current_draft: base_workflow,
        };
        let mut sessions = self
            .sessions
            .lock()
            .expect("authoring sessions mutex poisoned");
        // ponytail: drop oldest when cap hit; upgrade to LRU if sessions need fair retention
        if sessions.len() >= MAX_AUTHORING_SESSIONS {
            if let Some(oldest) = sessions.keys().next().cloned() {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(id.clone(), session);
        id
    }

    #[must_use]
    pub fn end_session(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .expect("authoring sessions mutex poisoned")
            .remove(session_id)
            .is_some()
    }

    pub fn get_session(&self, session_id: &str) -> Option<WorkflowAuthoringSession> {
        self.sessions
            .lock()
            .expect("authoring sessions mutex poisoned")
            .get(session_id)
            .cloned()
    }

    pub async fn send_turn<A: AiPort + Send + Sync>(
        &self,
        session_id: &str,
        user_message: String,
        settings: &AppSettings,
        ai: &A,
    ) -> Result<WorkflowAuthoringTurnResult, AuthoringError> {
        let (messages, current_draft) = {
            let mut sessions = self
                .sessions
                .lock()
                .expect("authoring sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or(AuthoringError::SessionNotFound)?;
            session.messages.push(WorkflowAuthoringMessage {
                role: WorkflowAuthoringRole::User,
                content: user_message.clone(),
            });
            (session.messages.clone(), session.current_draft.clone())
        };

        let model = settings
            .active_profile()
            .default_model
            .clone()
            .unwrap_or_else(|| "gpt-5.5".to_string());

        let mut transcript: Vec<AgentTranscriptItem> = messages
            .iter()
            .map(|message| match message.role {
                WorkflowAuthoringRole::Assistant => AgentTranscriptItem::AssistantMessage {
                    content: message.content.clone(),
                },
                WorkflowAuthoringRole::User => AgentTranscriptItem::UserMessage {
                    content: message.content.clone(),
                },
            })
            .collect();

        let base_context = current_draft
            .as_ref()
            .map(|workflow| serde_json::to_string_pretty(workflow).unwrap_or_default())
            .unwrap_or_default();

        let system_prompt = authoring_system_prompt();
        let output_schema = authoring_output_schema();
        let task_prompt = if base_context.is_empty() {
            "Create or update the workflow draft from the conversation.".to_string()
        } else {
            format!(
                "Update the workflow draft from the conversation.\n\nCurrent draft JSON:\n{base_context}"
            )
        };

        let mut model_attempt = 1u8;
        let mut malformed_submit_retries = 0u8;
        let output = loop {
            let request = AgentRequest {
                workflow_id: WorkflowId::from("workflow-authoring"),
                node_id: NodeId::from("authoring"),
                node_label: "Workflow authoring".to_string(),
                model: model.clone(),
                system_messages: vec![system_prompt.clone()],
                task_prompt: task_prompt.clone(),
                input: json!({ "userMessage": user_message }),
                output_schema: output_schema.clone(),
                tool_config: Default::default(),
                available_tools: Vec::new(),
                // ponytail: clone per invoke until AgentRequest borrows transcript
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
                        role: WorkflowAuthoringRole::Assistant,
                        content: assistant_message.clone(),
                    });
                    {
                        let mut sessions = self
                            .sessions
                            .lock()
                            .expect("authoring sessions mutex poisoned");
                        let session = sessions
                            .get_mut(session_id)
                            .ok_or(AuthoringError::SessionNotFound)?;
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
                    return Err(AuthoringError::ModelToolCalls);
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
                Err(error) => return Err(error.into()),
            }
        };

        let assistant_message = output
            .get("assistantMessage")
            .or_else(|| output.get("assistant_message"))
            .and_then(|value| value.as_str())
            .unwrap_or("Updated workflow draft.")
            .to_string();

        let draft_value = workflow_draft_value_from_model_output(&output)?;

        let draft: WorkflowAuthoringDraft = serde_json::from_value(draft_value)
            .map_err(|error| AuthoringError::InvalidDraft(error.to_string()))?;

        let base_id = current_draft.as_ref().map(|workflow| workflow.id.clone());
        let mut workflow = materialize_authoring_draft(draft, base_id, &model);
        layout_workflow_by_layers(&mut workflow)
            .map_err(|error| AuthoringError::LayoutFailed(error.to_string()))?;
        let validation = validate_authoring_workflow(&workflow);

        let mut messages = messages;
        messages.push(WorkflowAuthoringMessage {
            role: WorkflowAuthoringRole::Assistant,
            content: assistant_message.clone(),
        });

        {
            let mut sessions = self
                .sessions
                .lock()
                .expect("authoring sessions mutex poisoned");
            let session = sessions
                .get_mut(session_id)
                .ok_or(AuthoringError::SessionNotFound)?;
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
