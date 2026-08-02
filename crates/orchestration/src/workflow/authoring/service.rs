use crate::api::{
    WorkflowAuthoringDraftEvent, WorkflowAuthoringMessage, WorkflowAuthoringRole,
    WorkflowAuthoringStartResult, WorkflowAuthoringThinkingEvent, WorkflowAuthoringTurnResult,
    WorkflowAuthoringValidation,
};
use crate::run::prep::provider_reasoning_for_profile;
use crate::settings::model::AppSettings;
use crate::tool::ProjectReadTools;
use crate::workflow::authoring::tools::{
    authoring_tool_definitions, is_authoring_tool, AuthoringToolState, MAX_AUTHORING_TOOL_ROUNDS,
};
use crate::workflow::authoring::{
    default_authoring_template_workflow, layout_workflow_by_layers, materialize_authoring_draft,
    validate_authoring_workflow, workflow_draft_value_from_model_output, AuthoringError,
    WorkflowAuthoringDraft,
};
use engine::{
    AgentError, AgentMessageTurn, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
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
                    attachments: Vec::new(),
                }),
                WorkflowAuthoringRole::Thinking => None,
            })
            .collect();

        let base_context = current_draft
            .as_ref()
            .map(|workflow| serde_json::to_string_pretty(workflow).unwrap_or_default())
            .unwrap_or_default();

        let system_prompt = authoring_system_prompt(project_context.as_ref());
        let authoring_change_requested = explicit_workflow_change_request(&user_message);
        let output_schema = authoring_finish_output_schema();
        let project_read_tools = project_context
            .as_ref()
            .map(|project| {
                let cwd = project
                    .default_execution_cwd
                    .as_deref()
                    .filter(|cwd| !cwd.trim().is_empty())
                    .unwrap_or(&project.path);
                ProjectReadTools::new(cwd)
                    .map_err(|error| AuthoringError::ProjectReadTools(error.to_string()))
            })
            .transpose()?;
        let mut available_tools = if authoring_change_requested {
            authoring_tool_definitions()
        } else {
            Vec::new()
        };
        if project_read_tools.is_some() {
            available_tools.extend(ProjectReadTools::definitions());
        }
        let task_prompt = if base_context.is_empty() {
            "Continue the conversation. Answer the user's message directly. Only propose a workflow draft if the user explicitly asks for a workflow change.".to_string()
        } else {
            format!(
                "Continue the conversation. Answer the user's message directly unless the user explicitly asks for a workflow change. Treat the current draft as a proposal; do not change it for an informational question.\n\nCurrent draft JSON:\n{base_context}"
            )
        };

        let (reasoning_effort, reasoning_budget_tokens) =
            provider_reasoning_for_profile(settings.active_profile());

        let mut tool_state = AuthoringToolState::new(current_draft.as_ref(), &model);
        let mut model_attempt = 1u8;
        let mut malformed_submit_retries = 0u8;
        let mut missing_submit_retries = 0u8;
        let mut missing_draft_retries = 0u8;
        let mut mixed_tool_turn_retries = 0u8;
        let mut invalid_draft_retries = 0u8;
        let mut authoring_tool_rounds = 0u8;
        let mut messages = messages;
        let mut draft_changed = false;
        let (assistant_message, workflow, validation) = loop {
            let request = AgentRequest {
                workflow_id: WorkflowId::from("workflow-authoring"),
                node_id: NodeId::from("authoring"),
                node_label: "Workflow authoring".to_string(),
                model: model.clone(),
                provider_id: None,
                max_output_tokens: None,
                system_messages: vec![system_prompt.clone()],
                task_prompt: task_prompt.clone(),
                input: json!({ "userMessage": user_message }),
                output_schema: output_schema.clone(),
                tool_config: Default::default(),
                available_tools: available_tools.clone(),
                transcript: transcript.clone(),
                entrypoint_attachments: Vec::new(),
                resolved_attachments: Default::default(),
                model_attempt,
                reasoning_effort: reasoning_effort.clone(),
                reasoning_budget_tokens,
                fast_mode: false,
                allow_user_input: false,
                conversation_mode: true,
                tool_access_policy: engine::ToolAccessPolicy::Execution,
            };

            let thinking_buffer = Arc::new(Mutex::new(String::new()));
            let sink = AuthoringStreamSink {
                session_id: session_id.to_string(),
                thinking_buffer: Arc::clone(&thinking_buffer),
                on_thinking: &on_thinking,
            };

            match ai.invoke_stream(request, &sink).await {
                Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                    if batch.tool_calls.iter().any(|call| {
                        !is_authoring_tool(&call.name)
                            && !project_read_tools
                                .as_ref()
                                .is_some_and(|tools| tools.handles(&call.name))
                    }) {
                        return Err(AuthoringError::ModelToolCalls);
                    }
                    if authoring_tool_rounds >= MAX_AUTHORING_TOOL_ROUNDS {
                        return Err(AuthoringError::ToolRoundLimitExceeded(
                            MAX_AUTHORING_TOOL_ROUNDS,
                        ));
                    }
                    authoring_tool_rounds += 1;
                    malformed_submit_retries = 0;
                    missing_submit_retries = 0;

                    for reasoning in &batch.reasoning {
                        transcript.push(AgentTranscriptItem::Reasoning {
                            reasoning: reasoning.clone(),
                        });
                    }
                    if let Some(content) = batch.assistant_message.filter(|value| !value.is_empty())
                    {
                        transcript.push(AgentTranscriptItem::AssistantMessage { content });
                    }
                    let draft_before_tools = tool_state.snapshot();
                    for call in &batch.tool_calls {
                        transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
                        let result = if is_authoring_tool(&call.name) {
                            tool_state.execute(call)
                        } else {
                            project_read_tools
                                .as_ref()
                                .expect("project read tool calls were validated")
                                .execute(call)
                                .await
                        };
                        transcript.push(AgentTranscriptItem::ToolResult { result });
                    }

                    if draft_before_tools != tool_state.snapshot() {
                        draft_changed = true;
                        missing_draft_retries = 0;
                    }

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
                        if !authoring_change_requested {
                            let (workflow, validation) =
                                existing_draft_state(current_draft.clone());
                            break (assistant_message, workflow, validation);
                        }
                        draft_changed = true;
                        match build_workflow_from_output(&output, current_draft.as_ref(), &model) {
                            Ok((workflow, validation)) if validation.valid => {
                                break (assistant_message, Some(workflow), validation)
                            }
                            Ok((workflow, validation)) => {
                                if invalid_draft_retries >= MAX_INVALID_DRAFT_RETRIES {
                                    break (assistant_message, Some(workflow), validation);
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
                                if authoring_change_requested && !draft_changed {
                                    if missing_draft_retries < MAX_MISSING_DRAFT_RETRIES {
                                        missing_draft_retries += 1;
                                        model_attempt += 1;
                                        push_missing_draft_retry(
                                            &mut messages,
                                            &mut transcript,
                                            &assistant_message,
                                            missing_draft_retries,
                                        );
                                        continue;
                                    }
                                    break (
                                        proposal_not_created_message(&assistant_message),
                                        Some(workflow),
                                        validation,
                                    );
                                }
                                break (assistant_message, Some(workflow), validation);
                            }
                            Ok((workflow, validation)) => {
                                if invalid_draft_retries >= MAX_INVALID_DRAFT_RETRIES {
                                    break (assistant_message, Some(workflow), validation);
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
                Ok(AgentTurnOutcome::Message(AgentMessageTurn {
                    assistant_message, ..
                })) => {
                    if authoring_change_requested
                        && !draft_changed
                        && claims_workflow_proposal_ready(&assistant_message)
                    {
                        if missing_draft_retries < MAX_MISSING_DRAFT_RETRIES {
                            missing_draft_retries += 1;
                            model_attempt += 1;
                            push_missing_draft_retry(
                                &mut messages,
                                &mut transcript,
                                &assistant_message,
                                missing_draft_retries,
                            );
                            continue;
                        }
                        let (workflow, validation) = existing_draft_state(current_draft.clone());
                        break (
                            proposal_not_created_message(&assistant_message),
                            workflow,
                            validation,
                        );
                    }
                    let (workflow, validation) = existing_draft_state(current_draft.clone());
                    break (assistant_message, workflow, validation);
                }
                Ok(AgentTurnOutcome::NeedsUserInput(need)) => {
                    let assistant_message = need.assistant_message;
                    let (draft, validation) = existing_draft_state(current_draft.clone());
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
                        draft,
                        validation,
                        messages,
                        draft_changed: false,
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
                        attachments: Vec::new(),
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
                        attachments: Vec::new(),
                    });
                }
                Err(error)
                    if error.is_mixed_tool_turn()
                        && mixed_tool_turn_retries < MAX_MIXED_TOOL_TURN_RETRIES =>
                {
                    mixed_tool_turn_retries += 1;
                    model_attempt = model_attempt.saturating_add(1);
                    transcript.push(AgentTranscriptItem::UserMessage {
                        content: mixed_tool_turn_feedback(&error),
                        attachments: Vec::new(),
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
            session.current_draft = workflow.clone();
        }

        if draft_changed {
            if let Some(workflow) = workflow.clone() {
                on_draft_update(WorkflowAuthoringDraftEvent {
                    session_id: session_id.to_string(),
                    draft: Some(workflow),
                    validation: validation.clone(),
                });
            }
        }

        Ok(WorkflowAuthoringTurnResult {
            session_id: session_id.to_string(),
            assistant_message,
            draft: workflow,
            validation,
            messages,
            draft_changed,
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
            AiStreamEvent::OutputRepairStarted { .. }
            | AiStreamEvent::OutputRepairSucceeded { .. }
            | AiStreamEvent::OutputRepairFailed { .. } => String::new(),
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

const MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES: u8 = 3;
const MAX_MISSING_SUBMIT_TURN_RETRIES: u8 = 3;
const MAX_MISSING_DRAFT_RETRIES: u8 = 2;
const MAX_MIXED_TOOL_TURN_RETRIES: u8 = 3;
const MAX_INVALID_DRAFT_RETRIES: u8 = 5;

fn mixed_tool_turn_feedback(error: &AgentError) -> String {
    let tool_names = error.mixed_tool_names().unwrap_or("unknown tools");
    format!(
        "Your last response mixed finish/submit tools and authoring tools ({tool_names}) and was rejected; no calls from that response were executed. Call openflow_submit_node_output alone when the draft is complete, or call one or more authoring tools (openflow_set_workflow_meta, openflow_add_node, openflow_update_node, openflow_add_edge, openflow_remove_node, openflow_remove_edge) without submit in the same batch."
    )
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
    transcript.push(AgentTranscriptItem::UserMessage {
        content: feedback,
        attachments: Vec::new(),
    });
}

fn push_missing_draft_retry(
    messages: &mut Vec<WorkflowAuthoringMessage>,
    transcript: &mut Vec<AgentTranscriptItem>,
    assistant_message: &str,
    attempt: u8,
) {
    messages.push(WorkflowAuthoringMessage {
        role: WorkflowAuthoringRole::Thinking,
        content: format!(
            "The model described a proposal without changing the draft; asking it to create the draft (attempt {attempt}/{MAX_MISSING_DRAFT_RETRIES})."
        ),
    });
    transcript.push(AgentTranscriptItem::AssistantMessage {
        content: assistant_message.to_string(),
    });
    transcript.push(AgentTranscriptItem::UserMessage {
        content: "You described a workflow proposal without changing the workflow draft. Do not claim that a proposal is ready yet. Use the authoring tools to create or edit the requested workflow, then call openflow_submit_node_output with assistantMessage only. If the request is unclear, ask a clarification question instead of submitting a proposal.".to_string(),
        attachments: Vec::new(),
    });
}

fn claims_workflow_proposal_ready(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let claims_proposal = ["proposal", "propose", "proposed"]
        .iter()
        .any(|term| message.contains(term));
    let asks_acceptance = ["review", "accept", "not saved", "ready"]
        .iter()
        .any(|term| message.contains(term));
    claims_proposal && asks_acceptance
}

fn proposal_not_created_message(message: &str) -> String {
    format!(
        "{message}\n\nI haven't changed the workflow draft yet, so there is nothing to accept. Tell me what to add or edit and I'll prepare a proposal."
    )
}

fn output_contains_legacy_draft(output: &Value) -> bool {
    workflow_draft_value_from_model_output(output).is_ok()
}

fn existing_draft_state(
    draft: Option<Workflow>,
) -> (Option<Workflow>, WorkflowAuthoringValidation) {
    match draft {
        Some(workflow) => {
            let validation = validate_authoring_workflow(&workflow);
            (Some(workflow), validation)
        }
        None => (
            None,
            WorkflowAuthoringValidation {
                valid: false,
                errors: vec!["No workflow draft exists yet".to_string()],
                warnings: Vec::new(),
                dag: None,
            },
        ),
    }
}

pub(super) fn explicit_workflow_change_request(message: &str) -> bool {
    const ACTIONS: &[&str] = &[
        "add",
        "apply",
        "build",
        "change",
        "configure",
        "create",
        "delete",
        "design",
        "edit",
        "make",
        "modify",
        "remove",
        "rename",
        "replace",
        "revise",
        "update",
    ];
    const TARGETS: &[&str] = &[
        "agent",
        "agents",
        "draft",
        "edge",
        "edges",
        "graph",
        "handoff",
        "node",
        "nodes",
        "pipeline",
        "prompt",
        "prompts",
        "workflow",
        "workflows",
    ];
    let words = message
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let has_action = words.iter().any(|word| ACTIONS.contains(&word.as_str()));
    let has_target = words.iter().any(|word| TARGETS.contains(&word.as_str()));
    let starts_workflow_creation = matches!(
        words.first().map(String::as_str),
        Some("build" | "create" | "design" | "make")
    );
    if !has_action || (!has_target && !starts_workflow_creation) {
        return false;
    }

    let first = words.first().map(String::as_str);
    if matches!(
        first,
        Some("how" | "what" | "why" | "when" | "where" | "tell" | "explain")
    ) || words
        .iter()
        .any(|word| matches!(word.as_str(), "explain" | "understand" | "learn" | "know"))
    {
        return false;
    }

    let direct_request = matches!(first, Some(word) if ACTIONS.contains(&word))
        || words.iter().any(|word| word == "please")
        || words.windows(2).any(|window| window == ["let", "us"])
        || words.windows(2).any(|window| window == ["help", "me"])
        || words.windows(2).any(|window| window == ["i", "want"])
        || words.windows(2).any(|window| window == ["i", "need"])
        || words
            .windows(3)
            .any(|window| window == ["i", "would", "like"])
        || matches!(first, Some("can" | "could" | "would") if words.get(1).is_some_and(|word| matches!(word.as_str(), "you" | "we" | "us")));

    direct_request
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
         Read-only project tools are available in this conversation: read, search, and find. \
         Use them to inspect relevant files before designing repository-specific nodes. All paths \
         must be relative to the project's execution cwd. These tools cannot modify files.\n\n\
         Use this context to make repository-aware assumptions when the user explicitly asks for \
         a repository workflow change. Prefer nodes that can inspect, reason about, and modify \
         files relative to the project's execution cwd. For informational questions, answer in \
         chat without changing the draft.\n\n\
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
