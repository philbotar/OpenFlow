use crate::api::{
    WorkflowAuthoringDraftEvent, WorkflowAuthoringMessage, WorkflowAuthoringRole,
    WorkflowAuthoringStartResult, WorkflowAuthoringThinkingEvent, WorkflowAuthoringTurnResult,
    WorkflowAuthoringValidation,
};
use crate::run::prep::provider_reasoning_for_profile;
use crate::settings::model::AppSettings;
use crate::workflow::authoring::tools::{
    authoring_tool_definitions, is_authoring_tool, AuthoringToolState, MAX_AUTHORING_TOOL_ROUNDS,
};
use crate::workflow::authoring::{
    default_authoring_template_workflow, layout_workflow_by_layers, materialize_authoring_draft,
    validate_authoring_workflow, workflow_draft_value_from_model_output, AuthoringError,
    WorkflowAuthoringDraft,
};
use engine::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink, NodeId, Workflow, WorkflowId,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const MAX_AUTHORING_SESSIONS: usize = 64;
const DEFAULT_AUTHORING_MODEL: &str = "gpt-5.5";

#[derive(Clone)]
pub struct WorkflowAuthoringSession {
    pub id: String,
    pub messages: Vec<WorkflowAuthoringMessage>,
    pub current_draft: Option<Workflow>,
    pub project_context: Option<WorkflowAuthoringProjectContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowAuthoringProjectContext {
    pub id: String,
    pub name: String,
    pub path: String,
    pub default_execution_cwd: Option<String>,
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

    pub fn start_session(&self, base_workflow: Option<Workflow>) -> WorkflowAuthoringStartResult {
        self.start_session_with_project_context(base_workflow, None)
    }

    pub fn start_project_session(
        &self,
        base_workflow: Option<Workflow>,
        project_context: WorkflowAuthoringProjectContext,
    ) -> WorkflowAuthoringStartResult {
        self.start_session_with_project_context(base_workflow, Some(project_context))
    }

    fn start_session_with_project_context(
        &self,
        base_workflow: Option<Workflow>,
        project_context: Option<WorkflowAuthoringProjectContext>,
    ) -> WorkflowAuthoringStartResult {
        let id = Uuid::new_v4().to_string();
        let current_draft = match base_workflow {
            Some(workflow) => Some(workflow),
            None => Some(default_authoring_template_workflow(DEFAULT_AUTHORING_MODEL)),
        };
        let session = WorkflowAuthoringSession {
            id: id.clone(),
            messages: Vec::new(),
            current_draft: current_draft.clone(),
            project_context,
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
        WorkflowAuthoringStartResult {
            session_id: id,
            draft: current_draft,
        }
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

    pub async fn send_turn<A, F, G>(
        &self,
        session_id: &str,
        user_message: String,
        settings: &AppSettings,
        ai: &A,
        on_thinking: F,
        on_draft_update: G,
    ) -> Result<WorkflowAuthoringTurnResult, AuthoringError>
    where
        A: AiPort + Send + Sync,
        F: Fn(WorkflowAuthoringThinkingEvent) + Send + Sync,
        G: Fn(WorkflowAuthoringDraftEvent) + Send + Sync,
    {
        let (messages, current_draft, project_context) = {
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
            (
                session.messages.clone(),
                session.current_draft.clone(),
                session.project_context.clone(),
            )
        };

        let model = settings
            .active_profile()
            .default_model
            .clone()
            .unwrap_or_else(|| "gpt-5.5".to_string());

        let mut transcript: Vec<AgentTranscriptItem> = messages
            .iter()
            .filter_map(|message| match message.role {
                WorkflowAuthoringRole::Assistant => Some(AgentTranscriptItem::AssistantMessage {
                    content: message.content.clone(),
                }),
                WorkflowAuthoringRole::User => Some(AgentTranscriptItem::UserMessage {
                    content: message.content.clone(),
                }),
                WorkflowAuthoringRole::Thinking => None,
            })
            .collect();

        let base_context = current_draft
            .as_ref()
            .map(|workflow| serde_json::to_string_pretty(workflow).unwrap_or_default())
            .unwrap_or_default();

        let system_prompt = authoring_system_prompt(project_context.as_ref());
        let output_schema = authoring_finish_output_schema();
        let task_prompt = if base_context.is_empty() {
            "Create the workflow draft incrementally using the authoring tools.".to_string()
        } else {
            format!(
                "Update the workflow draft incrementally using the authoring tools.\n\nCurrent draft JSON:\n{base_context}"
            )
        };

        let (reasoning_effort, reasoning_budget_tokens) =
            provider_reasoning_for_profile(settings.active_profile());

        let mut tool_state = AuthoringToolState::new(current_draft.as_ref(), &model);
        let mut model_attempt = 1u8;
        let mut malformed_submit_retries = 0u8;
        let mut missing_submit_retries = 0u8;
        let mut invalid_draft_retries = 0u8;
        let mut authoring_tool_rounds = 0u8;
        let mut messages = messages;
        let (assistant_message, workflow, validation) = loop {
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
                available_tools: authoring_tool_definitions(),
                transcript: transcript.clone(),
                model_attempt,
                reasoning_effort: reasoning_effort.clone(),
                reasoning_budget_tokens,
                allow_user_input: false,
            };

            let thinking_buffer = Arc::new(Mutex::new(String::new()));
            let sink = AuthoringStreamSink {
                session_id: session_id.to_string(),
                thinking_buffer: Arc::clone(&thinking_buffer),
                on_thinking: &on_thinking,
            };

            match ai.invoke_stream(request, &sink).await {
                Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                    if batch
                        .tool_calls
                        .iter()
                        .any(|call| !is_authoring_tool(&call.name))
                    {
                        return Err(AuthoringError::ModelToolCalls);
                    }
                    if authoring_tool_rounds >= MAX_AUTHORING_TOOL_ROUNDS {
                        return Err(AuthoringError::ToolRoundLimitExceeded(
                            MAX_AUTHORING_TOOL_ROUNDS,
                        ));
                    }
                    authoring_tool_rounds += 1;

                    if let Some(content) = batch.assistant_message.filter(|value| !value.is_empty())
                    {
                        transcript.push(AgentTranscriptItem::AssistantMessage { content });
                    }
                    for call in &batch.tool_calls {
                        transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
                        let result = tool_state.execute(call);
                        transcript.push(AgentTranscriptItem::ToolResult { result });
                    }

                    publish_draft_progress(self, session_id, &tool_state, &on_draft_update);

                    let thinking_text = thinking_buffer
                        .lock()
                        .expect("authoring thinking buffer poisoned")
                        .trim()
                        .to_string();
                    if !thinking_text.is_empty() {
                        on_thinking(WorkflowAuthoringThinkingEvent {
                            session_id: session_id.to_string(),
                            delta: String::new(),
                            finalize: true,
                        });
                        messages.push(WorkflowAuthoringMessage {
                            role: WorkflowAuthoringRole::Thinking,
                            content: thinking_text,
                        });
                    }
                    continue;
                }
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. })) => {
                    let assistant_message = extract_assistant_message(&output);
                    if output_contains_legacy_draft(&output) {
                        match build_workflow_from_output(&output, current_draft.as_ref(), &model) {
                            Ok((workflow, validation)) if validation.valid => {
                                break (assistant_message, workflow, validation)
                            }
                            Ok((workflow, validation)) => {
                                if invalid_draft_retries >= MAX_INVALID_DRAFT_RETRIES {
                                    break (assistant_message, workflow, validation);
                                }
                                invalid_draft_retries += 1;
                                model_attempt += 1;
                                push_invalid_draft_retry(
                                    &mut messages,
                                    &mut transcript,
                                    &assistant_message,
                                    &validation.errors.join("; "),
                                    invalid_draft_retries,
                                    true,
                                );
                            }
                            Err(error) if invalid_draft_retries < MAX_INVALID_DRAFT_RETRIES => {
                                invalid_draft_retries += 1;
                                model_attempt += 1;
                                push_invalid_draft_retry(
                                    &mut messages,
                                    &mut transcript,
                                    &assistant_message,
                                    &error.to_string(),
                                    invalid_draft_retries,
                                    true,
                                );
                            }
                            Err(error) => return Err(error),
                        }
                    } else {
                        match tool_state.materialize_workflow() {
                            Ok((workflow, validation)) if validation.valid => {
                                break (assistant_message, workflow, validation)
                            }
                            Ok((workflow, validation)) => {
                                if invalid_draft_retries >= MAX_INVALID_DRAFT_RETRIES {
                                    break (assistant_message, workflow, validation);
                                }
                                invalid_draft_retries += 1;
                                model_attempt += 1;
                                push_invalid_draft_retry(
                                    &mut messages,
                                    &mut transcript,
                                    &assistant_message,
                                    &validation.errors.join("; "),
                                    invalid_draft_retries,
                                    false,
                                );
                            }
                            Err(error) if invalid_draft_retries < MAX_INVALID_DRAFT_RETRIES => {
                                invalid_draft_retries += 1;
                                model_attempt += 1;
                                push_invalid_draft_retry(
                                    &mut messages,
                                    &mut transcript,
                                    &assistant_message,
                                    &error.to_string(),
                                    invalid_draft_retries,
                                    false,
                                );
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    assistant_message,
                    ..
                })) if model_attempt <= MAX_AUTHORING_CLARIFICATION_RETRIES => {
                    transcript.push(AgentTranscriptItem::AssistantMessage {
                        content: assistant_message,
                    });
                    transcript.push(AgentTranscriptItem::UserMessage {
                        content: AUTHORING_FINISH_REQUIRED_FEEDBACK.to_string(),
                    });
                    model_attempt += 1;
                }
                Ok(AgentTurnOutcome::NeedsUserInput(need)) => {
                    let assistant_message = need.assistant_message;
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
                Err(error)
                    if is_missing_submit_turn(&error)
                        && missing_submit_retries < MAX_MISSING_SUBMIT_TURN_RETRIES =>
                {
                    missing_submit_retries += 1;
                    model_attempt += 1;
                    messages.push(WorkflowAuthoringMessage {
                        role: WorkflowAuthoringRole::Thinking,
                        content: format!(
                            "Model response had no submit output; asking it to call openflow_submit_node_output (attempt {missing_submit_retries}/{MAX_MISSING_SUBMIT_TURN_RETRIES})."
                        ),
                    });
                    transcript.push(AgentTranscriptItem::UserMessage {
                        content: missing_submit_turn_feedback(&error),
                    });
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
            };

            let thinking_text = thinking_buffer
                .lock()
                .expect("authoring thinking buffer poisoned")
                .trim()
                .to_string();
            if !thinking_text.is_empty() {
                on_thinking(WorkflowAuthoringThinkingEvent {
                    session_id: session_id.to_string(),
                    delta: String::new(),
                    finalize: true,
                });
                messages.push(WorkflowAuthoringMessage {
                    role: WorkflowAuthoringRole::Thinking,
                    content: thinking_text,
                });
            }
        };

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

struct AuthoringStreamSink<'a, F> {
    session_id: String,
    thinking_buffer: Arc<Mutex<String>>,
    on_thinking: &'a F,
}

impl<F> AiStreamSink for AuthoringStreamSink<'_, F>
where
    F: Fn(WorkflowAuthoringThinkingEvent) + Send + Sync,
{
    fn on_stream_event(&self, event: AiStreamEvent) {
        let content = match &event {
            AiStreamEvent::ThinkingDelta { content } => {
                if !content.is_empty() {
                    self.thinking_buffer
                        .lock()
                        .expect("authoring thinking buffer poisoned")
                        .push_str(content);
                }
                content.clone()
            }
            AiStreamEvent::AssistantDelta { content } => content.clone(),
        };
        if content.is_empty() {
            return;
        }
        (self.on_thinking)(WorkflowAuthoringThinkingEvent {
            session_id: self.session_id.clone(),
            delta: content,
            finalize: false,
        });
    }
}

const MAX_AUTHORING_CLARIFICATION_RETRIES: u8 = 1;
const MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES: u8 = 3;
const MAX_MISSING_SUBMIT_TURN_RETRIES: u8 = 3;
const MAX_INVALID_DRAFT_RETRIES: u8 = 5;

fn publish_draft_progress<G>(
    service: &WorkflowAuthoringService,
    session_id: &str,
    tool_state: &AuthoringToolState,
    on_draft_update: &G,
) where
    G: Fn(WorkflowAuthoringDraftEvent),
{
    let materialized = tool_state.materialize_workflow().ok();
    let validation = materialized
        .as_ref()
        .map(|(_, validation)| validation.clone())
        .unwrap_or_else(|| tool_state.validation_summary());
    let draft = materialized.map(|(workflow, _)| workflow);
    if let Some(workflow) = draft.clone() {
        let mut sessions = service
            .sessions
            .lock()
            .expect("authoring sessions mutex poisoned");
        if let Some(session) = sessions.get_mut(session_id) {
            session.current_draft = Some(workflow);
        }
    }
    on_draft_update(WorkflowAuthoringDraftEvent {
        session_id: session_id.to_string(),
        draft,
        validation,
    });
}

fn push_invalid_draft_retry(
    messages: &mut Vec<WorkflowAuthoringMessage>,
    transcript: &mut Vec<AgentTranscriptItem>,
    assistant_message: &str,
    error: &str,
    attempt: u8,
    legacy_draft: bool,
) {
    messages.push(WorkflowAuthoringMessage {
        role: WorkflowAuthoringRole::Thinking,
        content: format!(
            "Draft failed validation ({error}); asking the model to fix it (attempt {attempt}/{MAX_INVALID_DRAFT_RETRIES})."
        ),
    });
    transcript.push(AgentTranscriptItem::AssistantMessage {
        content: assistant_message.to_string(),
    });
    let feedback = if legacy_draft {
        format!(
            "Your workflowDraft failed validation: {error}. Fix these issues and call openflow_submit_node_output again with the complete corrected workflowDraft."
        )
    } else {
        format!(
            "Your workflow draft failed validation: {error}. Use the authoring tools to fix the draft, then call openflow_submit_node_output with assistantMessage only."
        )
    };
    transcript.push(AgentTranscriptItem::UserMessage { content: feedback });
}

fn output_contains_legacy_draft(output: &Value) -> bool {
    workflow_draft_value_from_model_output(output).is_ok()
}

fn extract_assistant_message(output: &Value) -> String {
    output
        .get("assistantMessage")
        .or_else(|| output.get("assistant_message"))
        .and_then(|value| value.as_str())
        .unwrap_or("Updated workflow draft.")
        .to_string()
}

/// Parse, materialize, lay out, and validate a workflow draft from model output.
fn build_workflow_from_output(
    output: &Value,
    current_draft: Option<&Workflow>,
    model: &str,
) -> Result<(Workflow, WorkflowAuthoringValidation), AuthoringError> {
    let draft_value = workflow_draft_value_from_model_output(output)?;
    let draft: WorkflowAuthoringDraft = serde_json::from_value(draft_value)
        .map_err(|error| AuthoringError::InvalidDraft(error.to_string()))?;
    let base_id = current_draft.map(|workflow| workflow.id.clone());
    let mut workflow = materialize_authoring_draft(draft, base_id, model);
    layout_workflow_by_layers(&mut workflow)
        .map_err(|error| AuthoringError::LayoutFailed(error.to_string()))?;
    let validation = validate_authoring_workflow(&workflow);
    Ok((workflow, validation))
}

const AUTHORING_FINISH_REQUIRED_FEEDBACK: &str = "Build the workflow with the authoring tools, then call openflow_submit_node_output with assistantMessage only. Do not ask clarifying questions — make reasonable assumptions.";

fn malformed_submit_output_feedback(error: &AgentError) -> String {
    format!(
        "Your openflow_submit_node_output call was invalid ({error}). \
         Call openflow_submit_node_output again with arguments shaped as \
         {{\"output\": {{\"assistantMessage\": \"...\"}}, \"assistant_message\": null}}. \
         Put assistantMessage under \"output\", not at the top level."
    )
}

fn is_missing_submit_turn(error: &AgentError) -> bool {
    matches!(
        error,
        AgentError::Failed(message)
            if message.contains("neither tool calls nor recoverable output")
                || message.contains("did not contain a function call")
    )
}

fn missing_submit_turn_feedback(error: &AgentError) -> String {
    format!(
        "Your last response was rejected ({error}). \
         Build or fix the workflow with the authoring tools, then call openflow_submit_node_output with \
         {{\"output\": {{\"assistantMessage\": \"...\"}}, \"assistant_message\": null}}."
    )
}

fn authoring_system_prompt(project_context: Option<&WorkflowAuthoringProjectContext>) -> String {
    let base = include_str!("prompts/workflow_authoring_system.txt");
    let Some(project) = project_context else {
        return base.to_string();
    };

    let default_execution_cwd = project
        .default_execution_cwd
        .as_deref()
        .filter(|cwd| !cwd.trim().is_empty())
        .unwrap_or(&project.path);
    format!(
        "{base}\n\
         ## Project authoring context\n\n\
         You are creating a workflow for an OpenFlow project. Design the workflow as a \
         project-scoped artifact that will be saved under the project and run from its \
         execution folder.\n\n\
         Project id: {id}\n\
         Project name: {name}\n\
         Project path: {path}\n\
         Default execution cwd: {default_execution_cwd}\n\n\
         Use this context to make repository-aware assumptions. Prefer nodes that can inspect, \
         reason about, and modify files relative to the project's execution cwd when the user's \
         request is about this codebase. Build incrementally with the authoring tools; do not ask \
         follow-up questions.\n\n\
         ## Starting template\n\n\
         This session begins with a preloaded template (clarify → parallel plan/risk → brief). \
         Adapt it with openflow_set_workflow_meta, openflow_update_node, and edge/node tools — do \
         not rebuild from scratch when the template fits.",
        id = project.id,
        name = project.name,
        path = project.path,
    )
}

fn authoring_finish_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "assistantMessage": { "type": "string" }
        },
        "required": ["assistantMessage"]
    })
}
