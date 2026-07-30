use crate::api::{ChatDeleteResult, UserMessageInput};
use crate::chat::{Chat, ChatConfig};
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::RunStoreRoot;
use crate::run::state::WorkflowRunState;
use engine::{HandoffSpec, Node, Workflow, WorkflowId};
use tokio::sync::mpsc::UnboundedReceiver;

use super::{AppBackend, BackendError};

const CHAT_SYSTEM_PROMPT: &str = "\
You are an AI assistant in an ongoing direct conversation. Answer the user's latest message \
naturally and helpfully. After every response, call openflow_request_user_input with your full \
response in assistant_message so the user can send their next message. This call ends the turn; \
it does not require a question. Do not ask a follow-up question unless the user's request cannot \
be completed without their answer. Never set questions in direct chat; use assistant_message \
only. Do not call openflow_submit_node_output unless the user explicitly asks to end the \
conversation.";
const CHAT_TASK_PROMPT: &str = "\
Reply to the latest user message. End the turn after the response; the user can send another \
message without a follow-up prompt.";

fn execution_workflow_for_chat(chat: &Chat) -> Workflow {
    let mut workflow = Workflow::new(&chat.title);
    workflow.id = WorkflowId(chat.id.clone());
    let mut node = Node::agent("Assistant", 80.0, 120.0);
    node.agent.system_prompt = CHAT_SYSTEM_PROMPT.to_string();
    node.agent.task_prompt = CHAT_TASK_PROMPT.to_string();
    node.agent.model = chat.config.model.clone().unwrap_or_default();
    node.agent.handoff = HandoffSpec::Legacy;
    node.agent.auto_start = true;
    node.agent.request_user_input = true;
    node.agent.tools.approval_mode = Some(chat.config.approval_mode);
    node.agent.tools.allow_structured_user_input = false;
    node.agent
        .reasoning_effort
        .clone_from(&chat.config.reasoning_effort);
    node.agent.reasoning_budget_tokens = chat.config.reasoning_budget_tokens;
    workflow.nodes.push(node);
    workflow
}

pub(super) fn refresh_execution_workflow_for_chat(chat: &Chat, workflow: &mut Workflow) -> bool {
    if workflow.id.to_string() != chat.id || workflow.nodes.len() != 1 {
        return false;
    }
    let agent = &mut workflow.nodes[0].agent;
    let changed = agent.system_prompt != CHAT_SYSTEM_PROMPT
        || agent.task_prompt != CHAT_TASK_PROMPT
        || agent.tools.allow_structured_user_input;
    if !changed {
        return false;
    }
    agent.system_prompt = CHAT_SYSTEM_PROMPT.to_string();
    agent.task_prompt = CHAT_TASK_PROMPT.to_string();
    agent.tools.allow_structured_user_input = false;
    true
}

impl AppBackend {
    pub fn create_chat(&self) -> Result<Chat, BackendError> {
        self.chats.create()
    }

    pub fn list_chats(&self) -> Result<Vec<Chat>, BackendError> {
        self.chats.list()
    }

    pub async fn delete_chat(&self, chat_id: &str) -> Result<ChatDeleteResult, BackendError> {
        let chat = self.chats.load_one(chat_id)?;
        let Some(run_id) = chat.run_id.as_deref() else {
            self.chats.delete(chat_id)?;
            return Ok(ChatDeleteResult::Deleted);
        };
        if self.is_run_active().await && self.current_run_id().await.as_deref() == Some(run_id) {
            return Err(BackendError::ActiveChatRun);
        }
        let roots = self.run_roots()?;
        let run = self.run_store.load_record(&roots, run_id)?;
        let quarantine = match run.as_ref() {
            Some((root, _)) => self.run_store.quarantine_run(root, run_id)?,
            None => None,
        };
        if let Err(error) = self.chats.delete(chat_id) {
            if let (Some(path), Some((root, _))) = (quarantine.as_deref(), run.as_ref()) {
                if let Err(restore_error) =
                    self.run_store.restore_quarantined_run(path, root, run_id)
                {
                    log::error!(
                        "failed to restore run {run_id} after chat delete failure: {restore_error}"
                    );
                }
            }
            return Err(error);
        }
        let cleanup_pending = quarantine.as_deref().is_some_and(|path| {
            self.run_store
                .remove_quarantined_run(path)
                .inspect_err(|error| {
                    log::warn!("chat deleted; run {run_id} cleanup remains pending: {error}");
                })
                .is_err()
        });
        Ok(if cleanup_pending {
            ChatDeleteResult::DeletedCleanupPending
        } else {
            ChatDeleteResult::Deleted
        })
    }

    pub fn update_chat_config(
        &self,
        chat_id: &str,
        config: ChatConfig,
    ) -> Result<Chat, BackendError> {
        if let Some(project_id) = config.project_id.as_deref() {
            self.projects
                .load()?
                .iter()
                .find(|project| project.id == project_id)
                .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
        }
        self.chats.update_config(chat_id, config)
    }

    pub async fn start_chat(
        &self,
        chat_id: &str,
        first_message: Option<String>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(Chat, WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.start_chat_with_skill_ids(
            chat_id,
            first_message,
            settings,
            transient_api_key,
            Vec::new(),
        )
        .await
    }

    pub async fn start_chat_with_skill_ids(
        &self,
        chat_id: &str,
        first_message: Option<String>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        invoked_skill_ids: Vec<String>,
    ) -> Result<(Chat, WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.start_chat_with_message_and_skill_ids(
            chat_id,
            first_message.map(UserMessageInput::text),
            settings,
            transient_api_key,
            invoked_skill_ids,
        )
        .await
    }

    pub async fn start_chat_with_message_and_skill_ids(
        &self,
        chat_id: &str,
        first_message: Option<UserMessageInput>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        invoked_skill_ids: Vec<String>,
    ) -> Result<(Chat, WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let title_seed = first_message.as_ref().and_then(|message| {
            (!message.text.trim().is_empty())
                .then_some(message.text.as_str())
                .or_else(|| {
                    message
                        .attachment_source_paths
                        .first()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|name| name.to_str())
                })
        });
        let chat = self.chats.prepare_start(chat_id, title_seed)?;
        let project_context = match chat.config.project_id.as_deref() {
            Some(project_id) => {
                let project = self
                    .projects
                    .load()?
                    .into_iter()
                    .find(|project| project.id == project_id)
                    .ok_or_else(|| BackendError::ProjectNotFound(project_id.to_string()))?;
                let configured = project.default_execution_cwd.trim();
                let execution_cwd = if configured.is_empty() {
                    project.path.clone()
                } else {
                    configured.to_string()
                };
                let run_root = RunStoreRoot {
                    project_id: Some(project.id),
                    root: std::path::Path::new(&project.path)
                        .join(".flow")
                        .join("runs"),
                };
                Some((run_root, execution_cwd))
            }
            None => None,
        };
        let workflow = execution_workflow_for_chat(&chat);
        let entrypoint = first_message.filter(|message| !message.is_empty());
        let run_root_for_cleanup = project_context
            .as_ref()
            .map(|(run_root, _)| run_root.clone())
            .unwrap_or(RunStoreRoot {
                project_id: None,
                root: crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore::app_runs_root(),
            });
        let (state, event_rx) = if let Some((run_root, execution_cwd)) = project_context {
            self.start_run_with_root(
                workflow,
                entrypoint.clone(),
                Some(execution_cwd),
                run_root,
                settings,
                transient_api_key,
                invoked_skill_ids.clone(),
            )
            .await?
        } else {
            self.start_run_with_message_and_skill_ids(
                workflow,
                entrypoint,
                None,
                settings,
                transient_api_key,
                invoked_skill_ids,
            )
            .await?
        };
        let Some(run_id) = state.run_id.clone() else {
            let _ = self.stop_run().await;
            return Err(BackendError::ChatRunMissingId);
        };
        match self
            .chats
            .attach_run_with_title(chat_id, Some(chat.title), run_id.clone())
        {
            Ok(updated) => Ok((updated, state, event_rx)),
            Err(error) => {
                if let Err(stop_error) = self.stop_run().await {
                    log::error!("failed to roll back chat run after attach failure: {stop_error}");
                }
                if let Err(cleanup_error) =
                    self.run_store.remove_run(&run_root_for_cleanup, &run_id)
                {
                    log::error!(
                        "failed to remove run {run_id} after chat attach failure: {cleanup_error}"
                    );
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_workflow_uses_saved_chat_model() {
        let chat = Chat {
            id: "chat-1".to_string(),
            title: "Model selection".to_string(),
            config: ChatConfig {
                model: Some("gpt-5".to_string()),
                ..ChatConfig::default()
            },
            run_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };

        let workflow = execution_workflow_for_chat(&chat);

        assert_eq!(workflow.nodes[0].agent.model, "gpt-5");
        assert!(workflow.nodes[0].agent.auto_start);
    }

    #[test]
    fn execution_workflow_does_not_force_follow_up_questions_or_choices() {
        let chat = Chat {
            id: "chat-1".to_string(),
            title: "Natural conversation".to_string(),
            config: ChatConfig::default(),
            run_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };

        let workflow = execution_workflow_for_chat(&chat);
        let system_prompt = &workflow.nodes[0].agent.system_prompt;

        assert!(
            system_prompt.contains("Do not ask a follow-up question unless"),
            "direct chat must not force a question after every answer"
        );
        assert!(
            system_prompt.contains("Never set questions"),
            "direct chat must not offer structured multiple-choice prompts"
        );
        assert!(
            !workflow.nodes[0].agent.tools.allow_structured_user_input,
            "direct chat must remove structured questions from the model tool schema"
        );
    }

    #[test]
    fn refresh_execution_workflow_updates_saved_chat_prompt_only_once() {
        let chat = Chat {
            id: "chat-1".to_string(),
            title: "Saved conversation".to_string(),
            config: ChatConfig::default(),
            run_id: Some("run-1".to_string()),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        let mut workflow = execution_workflow_for_chat(&chat);
        workflow.nodes[0].agent.system_prompt =
            "Legacy prompt that always requests a questionnaire.".to_string();
        workflow.nodes[0].agent.task_prompt =
            "Reply, then end with one short question.".to_string();
        workflow.nodes[0].agent.tools.allow_structured_user_input = true;

        assert!(refresh_execution_workflow_for_chat(&chat, &mut workflow));
        assert_eq!(workflow.nodes[0].agent.system_prompt, CHAT_SYSTEM_PROMPT);
        assert_eq!(workflow.nodes[0].agent.task_prompt, CHAT_TASK_PROMPT);
        assert!(!workflow.nodes[0].agent.tools.allow_structured_user_input);
        assert!(!refresh_execution_workflow_for_chat(&chat, &mut workflow));
    }
}
