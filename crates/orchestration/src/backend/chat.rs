use crate::chat::{Chat, ChatConfig};
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::RunStoreRoot;
use crate::run::state::WorkflowRunState;
use engine::{Node, Workflow, WorkflowId};
use tokio::sync::mpsc::UnboundedReceiver;

use super::{AppBackend, BackendError};

const CHAT_SYSTEM_PROMPT: &str = "\
You are an AI assistant in an ongoing direct conversation. Answer the user's latest message \
naturally and helpfully. Keep the exchange conversational. After every answer, call \
openflow_request_user_input with your full response in assistant_message so the user can send \
their next message. End assistant_message with one short, direct question. Do not call \
openflow_submit_node_output unless the user explicitly asks to end the conversation.";

fn execution_workflow_for_chat(chat: &Chat) -> Workflow {
    let mut workflow = Workflow::new(&chat.title);
    workflow.id = WorkflowId(chat.id.clone());
    let mut node = Node::agent("Assistant", 80.0, 120.0);
    node.agent.system_prompt = CHAT_SYSTEM_PROMPT.to_string();
    node.agent.task_prompt =
        "Reply to the latest user message, then keep the conversation open.".to_string();
    node.agent.model = chat.config.model.clone().unwrap_or_default();
    node.agent.auto_start = true;
    node.agent.request_user_input = true;
    node.agent.tools.approval_mode = Some(chat.config.approval_mode);
    node.agent
        .reasoning_effort
        .clone_from(&chat.config.reasoning_effort);
    node.agent.reasoning_budget_tokens = chat.config.reasoning_budget_tokens;
    workflow.nodes.push(node);
    workflow
}

impl AppBackend {
    pub fn create_chat(&self) -> Result<Chat, BackendError> {
        self.chats.create()
    }

    pub fn list_chats(&self) -> Result<Vec<Chat>, BackendError> {
        self.chats.list()
    }

    pub fn delete_chat(&self, chat_id: &str) -> Result<(), BackendError> {
        self.chats.delete(chat_id)
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
        let chat = self
            .chats
            .prepare_start(chat_id, first_message.as_deref())?;
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
        let entrypoint = first_message.filter(|message| !message.trim().is_empty());
        let (state, event_rx) = if let Some((run_root, execution_cwd)) = project_context {
            self.start_run_with_root(
                workflow,
                entrypoint.clone(),
                Some(execution_cwd),
                run_root,
                settings,
                transient_api_key,
            )
            .await?
        } else {
            self.start_run(workflow, entrypoint, None, settings, transient_api_key)
                .await?
        };
        let Some(run_id) = state.run_id.clone() else {
            let _ = self.stop_run().await;
            return Err(BackendError::ChatRunMissingId);
        };
        match self
            .chats
            .attach_run_with_title(chat_id, Some(chat.title), run_id)
        {
            Ok(updated) => Ok((updated, state, event_rx)),
            Err(error) => {
                if let Err(stop_error) = self.stop_run().await {
                    log::error!("failed to roll back chat run after attach failure: {stop_error}");
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
}
