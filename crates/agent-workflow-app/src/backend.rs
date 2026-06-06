#![allow(
    clippy::assigning_clones,
    clippy::derive_partial_eq_without_eq,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args
)]

use crate::agent_store::{AgentDefinition, FileAgentStore};
use crate::credential_store::CredentialStoreError;
use crate::execution::{
    apply_event_to_run_state, record_user_input, spawn_interactive_workflow_run, ExecutionAction,
    ExecutionEvent,
};
use crate::provider_config::{
    active_provider_env_var, active_provider_label, resolve_provider_config, ProviderConfigError,
    ProviderEnv,
};
use crate::settings_store::{key_ref_for_provider, AppSettings, FileSettingsStore};
use crate::state::WorkflowRunState;
use crate::storage::FileWorkflowStore;
use ai::{AiClient, ProviderId};
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use workflow_core::{
    execution_layers, validate_workflow, Node, NodeId, Workflow, WorkflowValidationError,
};

#[derive(Debug)]
pub struct AppBackend {
    workflow_store: FileWorkflowStore,
    agent_store: FileAgentStore,
    settings_store: FileSettingsStore,
    env: ProviderEnv,
    runtime: tokio::runtime::Runtime,
    run_session: Mutex<RunSession>,
}

#[derive(Debug)]
struct RunSession {
    workflow: Option<Workflow>,
    run_state: Option<WorkflowRunState>,
    action_tx: Option<UnboundedSender<ExecutionAction>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Credential(#[from] CredentialStoreError),
    #[error(transparent)]
    Validation(#[from] WorkflowValidationError),
    #[error(transparent)]
    ProviderConfig(#[from] ProviderConfigError),
    #[error("workflow {0} not found")]
    WorkflowNotFound(String),
    #[error("workflow run is not active")]
    NoActiveRun,
    #[error("workflow run is not awaiting input")]
    NoAwaitingInput,
    #[error("workflow run has no pending tool approval")]
    NoPendingApproval,
    #[error("expected input for {expected}, got {received}")]
    WrongAwaitingNode { expected: NodeId, received: NodeId },
    #[error("expected approval {expected}, got {received}")]
    WrongApprovalId { expected: String, received: String },
    #[error("workflow run channel closed")]
    RunChannelClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowListItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReadiness {
    pub ready: bool,
    pub provider: String,
    pub message: String,
    pub env_var: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowValidationSummary {
    pub layer_count: usize,
    pub layers: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinitionSummary {
    pub id: String,
    pub name: String,
    pub model: String,
}
impl AppBackend {
    #[must_use]
    pub fn new(
        workflow_store: FileWorkflowStore,
        agent_store: FileAgentStore,
        settings_store: FileSettingsStore,
        env: ProviderEnv,
        runtime: tokio::runtime::Runtime,
    ) -> Self {
        Self {
            workflow_store,
            agent_store,
            settings_store,
            env,
            runtime,
            run_session: Mutex::new(RunSession {
                workflow: None,
                run_state: None,
                action_tx: None,
                handle: None,
            }),
        }
    }

    #[must_use]
    pub fn with_default_paths() -> Self {
        Self::new(
            FileWorkflowStore::new(FileWorkflowStore::default_path()),
            FileAgentStore::new(FileAgentStore::default_path()),
            FileSettingsStore::new(FileSettingsStore::default_path()),
            ProviderEnv::from_system(),
            tokio::runtime::Runtime::new().expect("failed to create tokio runtime"),
        )
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be read.
    pub fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        Ok(self
            .workflow_store
            .load()?
            .into_iter()
            .map(|workflow| WorkflowListItem {
                id: workflow.id.to_string(),
                name: workflow.name,
            })
            .collect())
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be read.
    pub fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        self.workflow_store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be read or the workflow does not exist.
    pub fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflow_store
            .load()?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written.
    pub fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        let mut workflows = self.workflow_store.load()?;
        let workflow = default_workflow(name.as_str());
        workflows.push(workflow.clone());
        self.workflow_store.save(&workflows)?;
        Ok(workflow)
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written.
    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        let mut workflows = self.workflow_store.load()?;
        if let Some(existing) = workflows.iter_mut().find(|item| item.id == workflow.id) {
            *existing = workflow.clone();
        } else {
            workflows.push(workflow.clone());
        }
        self.workflow_store.save(&workflows)?;
        Ok(workflow)
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written.
    pub fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        self.workflow_store
            .save(workflows)
            .map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the workflow store cannot be written or the workflow does not exist.
    pub fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        let mut workflows = self.workflow_store.load()?;
        let workflow = workflows
            .iter_mut()
            .find(|item| item.id == workflow_id)
            .ok_or_else(|| BackendError::WorkflowNotFound(workflow_id.to_string()))?;
        workflow.name = name.clone();
        self.workflow_store.save(&workflows)?;
        Ok(WorkflowListItem {
            id: workflow_id.to_string(),
            name,
        })
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read.
    pub fn load_agents(&self) -> Result<Vec<AgentDefinition>, BackendError> {
        self.agent_store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn save_agents(&self, agents: &[AgentDefinition]) -> Result<(), BackendError> {
        self.agent_store.save(agents).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn create_agent_definition(&self, name: String) -> Result<AgentDefinition, BackendError> {
        let mut agents = self.agent_store.load()?;
        let agent = AgentDefinition::new(name);
        agents.push(agent.clone());
        self.agent_store.save(&agents)?;
        Ok(agent)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read or the selected agent does not exist.
    pub fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        let default_name = format!("Agent {}", index + 1);
        let Some(agent_id) = agent_id else {
            return Ok(Node::agent(default_name, x, y));
        };

        let agents = self.agent_store.load()?;
        let agent = agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| {
                BackendError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("agent {agent_id} not found"),
                ))
            })?;

        let label = if agent.name.trim().is_empty() {
            default_name
        } else {
            agent.name.clone()
        };
        let mut node = Node::agent(label, x, y);
        node.agent.system_prompt = agent.system_prompt.clone();
        node.agent.task_prompt = agent.task_prompt.clone();
        node.agent.model = agent.model.clone();
        node.agent.output_schema = agent.output_schema.clone();
        node.agent.auto_start = agent.auto_start;
        node.agent.tools = agent.tools.clone();

        Ok(node)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read.
    pub fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        Ok(self
            .agent_store
            .load()?
            .into_iter()
            .map(|agent| AgentDefinitionSummary {
                id: agent.id,
                name: agent.name,
                model: agent.model,
            })
            .collect())
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read.
    pub fn load_settings(&self) -> Result<AppSettings, BackendError> {
        self.settings_store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the settings file cannot be written.
    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.settings_store
            .save(settings)
            .map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the credential store cannot be read.
    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        let provider_id = ProviderId::from(provider_id);
        Ok(self
            .settings_store
            .credential_store()
            .get(&key_ref_for_provider(&provider_id))?)
    }

    /// # Errors
    /// Returns an error if the credential store cannot be written.
    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        let provider_id = ProviderId::from(provider_id);
        let key_ref = key_ref_for_provider(&provider_id);
        let api_key = api_key.trim();
        if api_key.is_empty() {
            self.settings_store.credential_store().delete(&key_ref)?;
        } else {
            self.settings_store
                .credential_store()
                .set(&key_ref, api_key)?;
        }
        Ok(())
    }

    /// # Errors
    /// Returns an error if the credential store cannot delete the key.
    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        let provider_id = ProviderId::from(provider_id);
        self.settings_store
            .credential_store()
            .delete(&key_ref_for_provider(&provider_id))?;
        Ok(())
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        match resolve_provider_config(
            settings,
            transient_api_key,
            &self.env,
            self.settings_store.credential_store(),
        ) {
            Ok(_) => ProviderReadiness {
                ready: true,
                provider: active_provider_label(settings),
                message: "Ready".to_string(),
                env_var: active_provider_env_var(settings)
                    .unwrap_or_default()
                    .to_string(),
            },
            Err(ProviderConfigError::MissingApiKey { provider, env_var }) => ProviderReadiness {
                ready: false,
                provider,
                message: format!("API key missing (set it in Settings or {env_var})"),
                env_var,
            },
            Err(error) => ProviderReadiness {
                ready: false,
                provider: active_provider_label(settings),
                message: error.to_string(),
                env_var: active_provider_env_var(settings)
                    .unwrap_or_default()
                    .to_string(),
            },
        }
    }

    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        validate_workflow(workflow)?;
        let layers = execution_layers(workflow)?;
        Ok(WorkflowValidationSummary {
            layer_count: layers.len(),
            layers: layers
                .into_iter()
                .map(|layer| layer.into_iter().map(|id| id.to_string()).collect())
                .collect(),
        })
    }

    /// # Errors
    /// Returns an error if the workflow fails validation or provider configuration fails.
    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        validate_workflow(&workflow)?;
        let provider_config = resolve_provider_config(
            settings,
            transient_api_key,
            &self.env,
            self.settings_store.credential_store(),
        )?;
        let ai = AiClient::with_config(provider_config);

        let (handle, event_rx, action_tx) =
            spawn_interactive_workflow_run(&self.runtime, workflow.clone(), entrypoint, ai);

        let mut session = self.run_session.lock().await;
        if let Some(existing) = session.handle.take() {
            existing.abort();
        }
        let initial_state = WorkflowRunState::running_for_workflow(&workflow);
        session.workflow = Some(workflow);
        session.run_state = Some(initial_state.clone());
        session.action_tx = Some(action_tx);
        session.handle = Some(handle);
        Ok((initial_state, event_rx))
    }

    /// # Errors
    /// Returns an error if there is no active run.
    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.run_session.lock().await;
        let workflow = session.workflow.clone().ok_or(BackendError::NoActiveRun)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        apply_event_to_run_state(&workflow, run_state, event);
        let finished = !run_state.active;
        let snapshot = run_state.clone();
        if finished {
            session.action_tx = None;
            session.handle = None;
        }
        Ok(snapshot)
    }

    /// # Errors
    /// Returns an error if there is no active run or the wrong node is selected.
    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.run_session.lock().await;
        let expected = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .awaiting_node_id
            .clone()
            .ok_or(BackendError::NoAwaitingInput)?;
        if expected != node_id {
            return Err(BackendError::WrongAwaitingNode {
                expected,
                received: NodeId(node_id.to_string()),
            });
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ProvideInput(text.clone()))
            .map_err(|_| BackendError::RunChannelClosed)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        record_user_input(run_state, node_id, text);
        Ok(run_state.clone())
    }

    /// # Errors
    /// Returns an error if there is no active run or the wrong approval is selected.
    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.run_session.lock().await;
        let expected = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .pending_approvals
            .first()
            .cloned()
            .ok_or(BackendError::NoPendingApproval)?;
        if expected.approval_id != approval_id {
            return Err(BackendError::WrongApprovalId {
                expected: expected.approval_id,
                received: approval_id.to_string(),
            });
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ResolveApproval {
                approval_id: approval_id.to_string(),
                allow,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        run_state.pending_approvals.clear();
        Ok(run_state.clone())
    }

    /// Returns an error because conversational paused nodes advance via `submit_user_input`.
    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        let session = self.run_session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        if run_state.awaiting_node_id.is_some() {
            return Err(BackendError::NoAwaitingInput);
        }
        Err(BackendError::NoAwaitingInput)
    }

    #[must_use]
    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.run_session.lock().await.run_state.clone()
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        let mut session = self.run_session.lock().await;
        let workflow = session.workflow.clone();
        let run_state = session.run_state.as_mut();
        match (workflow, run_state) {
            (Some(workflow), Some(run_state)) => {
                let mut cleared = WorkflowRunState::idle_for_workflow(&workflow);
                cleared.chat_logs = run_state.chat_logs.clone();
                cleared.outputs = run_state.outputs.clone();
                *run_state = cleared;
                Ok(Some(run_state.clone()))
            }
            _ => Ok(None),
        }
    }
}

fn default_workflow(name: &str) -> Workflow {
    let mut workflow = Workflow::new(name);
    workflow.nodes.push(Node::agent("Idea", 80.0, 120.0));
    workflow
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::credential_store::CredentialStore;
    use crate::settings_store::{ProviderProfile, ProviderTransport};
    use tempfile::tempdir;

    fn backend() -> (AppBackend, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let credential_store = CredentialStore::in_memory();
        let backend = AppBackend::new(
            FileWorkflowStore::new(dir.path().join("workflows.json")),
            FileAgentStore::new(dir.path().join("agents.json")),
            FileSettingsStore::with_credential_store(
                dir.path().join("settings.json"),
                credential_store,
            ),
            ProviderEnv::from_pairs([
                ("OPENAI_API_KEY", "openai-key"),
                ("OPENAI_COMPATIBLE_API_KEY", "compatible-key"),
            ]),
            tokio::runtime::Runtime::new().expect("runtime"),
        );
        (backend, dir)
    }

    #[test]
    fn create_and_load_workflow_round_trips() {
        let (backend, _dir) = backend();
        let workflow = backend
            .create_workflow("Workflow 1".to_string())
            .expect("create workflow");

        let items = backend.list_workflows().expect("list workflows");
        let loaded = backend.load_workflow(&workflow.id).expect("load workflow");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Workflow 1");
        assert_eq!(loaded.id, workflow.id);
        assert_eq!(loaded.nodes.len(), 1);
    }

    #[test]
    fn save_workflows_overwrites_store() {
        let (backend, _dir) = backend();
        let first = backend
            .create_workflow("One".to_string())
            .expect("create first workflow");
        let second = backend
            .create_workflow("Two".to_string())
            .expect("create second workflow");

        backend
            .save_workflows(std::slice::from_ref(&first))
            .expect("save workflows");

        let items = backend.list_workflows().expect("list workflows");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id.to_string());
        assert_eq!(
            backend
                .load_workflow(&second.id)
                .expect_err("missing second workflow")
                .to_string(),
            format!("workflow {} not found", second.id)
        );
    }

    #[test]
    fn create_and_load_agents_round_trip() {
        let (backend, _dir) = backend();
        let agent = backend
            .create_agent_definition("Research Agent".to_string())
            .expect("create agent");

        let items = backend.list_agents().expect("list agents");
        let loaded = backend.load_agents().expect("load agents");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Research Agent");
        assert_eq!(loaded, vec![agent]);
    }

    #[test]
    fn save_agents_overwrites_store() {
        let (backend, _dir) = backend();
        let first = backend
            .create_agent_definition("One".to_string())
            .expect("create first agent");
        backend
            .create_agent_definition("Two".to_string())
            .expect("create second agent");

        backend
            .save_agents(std::slice::from_ref(&first))
            .expect("save agents");

        let items = backend.list_agents().expect("list agents");
        let loaded = backend.load_agents().expect("load agents");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id);
        assert_eq!(loaded, vec![first]);
    }
    #[test]
    fn create_agent_node_without_template_uses_default_node() {
        let (backend, _dir) = backend();

        let node = backend
            .create_agent_node(2, 32.0, 48.0, None)
            .expect("create default node");

        assert_eq!(node.label, "Agent 3");
        assert_eq!(node.position.x, 32.0);
        assert_eq!(node.position.y, 48.0);
        assert_eq!(node.agent, workflow_core::AgentNodeConfig::default());
    }

    #[test]
    fn create_agent_node_from_template_id_copies_agent_config() {
        let (backend, _dir) = backend();
        let mut agent = backend
            .create_agent_definition("Research Agent".to_string())
            .expect("create agent");
        agent.system_prompt = "system".to_string();
        agent.task_prompt = "task".to_string();
        agent.model = "gpt-template".to_string();
        agent.output_schema =
            serde_json::json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } });
        agent.auto_start = false;
        agent.tools.catalog.tools = vec![workflow_core::ToolRef {
            name: "search".to_string(),
        }];
        agent.tools.max_tool_rounds = 7;
        backend
            .save_agents(std::slice::from_ref(&agent))
            .expect("save agent");

        let node = backend
            .create_agent_node(0, 12.0, 24.0, Some(&agent.id))
            .expect("create templated node");

        assert_eq!(node.label, "Research Agent");
        assert_eq!(node.position.x, 12.0);
        assert_eq!(node.position.y, 24.0);
        assert_eq!(node.agent.system_prompt, "system");
        assert_eq!(node.agent.task_prompt, "task");
        assert_eq!(node.agent.model, "gpt-template");
        assert_eq!(
            node.agent.output_schema,
            serde_json::json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } })
        );
        assert!(!node.agent.auto_start);
        assert_eq!(
            node.agent.tools.catalog.tools,
            vec![workflow_core::ToolRef {
                name: "search".to_string(),
            }]
        );
        assert_eq!(node.agent.tools.max_tool_rounds, 7);
    }

    #[test]
    fn provider_readiness_reports_missing_key() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("custom_openai_compatible"),
            ..AppSettings::default()
        };
        settings.providers.insert(
            ProviderId::from("custom_openai_compatible"),
            ProviderProfile {
                transport: ProviderTransport::ChatCompletions,
                ..ProviderProfile::compatible_default()
            },
        );

        let credential_store = CredentialStore::in_memory();
        let readiness = AppBackend::new(
            FileWorkflowStore::new("/tmp/unused-workflows.json"),
            FileAgentStore::new("/tmp/unused-agents.json"),
            FileSettingsStore::with_credential_store("/tmp/unused-settings.json", credential_store),
            ProviderEnv::default(),
            tokio::runtime::Runtime::new().expect("runtime"),
        )
        .resolve_provider_readiness(&settings, None);

        assert!(!readiness.ready);
        assert_eq!(readiness.env_var, "OPENAI_COMPATIBLE_API_KEY");
    }

    #[test]
    fn start_run_returns_initial_state_and_manual_events() {
        let (backend, _dir) = backend();
        backend.runtime.block_on(async {
            let mut workflow = Workflow::new("Manual run");
            let mut node = Node::agent("Review", 0.0, 0.0);
            node.id = NodeId("review".to_string());
            node.agent.auto_start = false;
            workflow.nodes = vec![node];

            let (initial_state, mut event_rx) = backend
                .start_run(workflow, None, &AppSettings::default(), None)
                .await
                .expect("start run");

            assert!(initial_state.active);
            assert!(initial_state.awaiting_node_id.is_none());

            let first = event_rx.recv().await.expect("queued event");
            let second = event_rx.recv().await.expect("awaiting event");
            assert!(matches!(
                first,
                ExecutionEvent::NodeQueued { ref node_id, ref label }
                    if node_id == "review" && label == "Review"
            ));
            assert!(matches!(
                second,
                ExecutionEvent::NodeAwaitingInput { ref node_id, ref label, is_initial: true, .. }
                    if node_id == "review" && label == "Review"
            ));

            let handle = {
                let mut session = backend.run_session.lock().await;
                session.handle.take()
            };
            if let Some(handle) = handle {
                handle.abort();
            }
        });
    }

    #[test]
    fn submit_user_input_updates_snapshot_and_sends_action() {
        let (backend, _dir) = backend();
        backend.runtime.block_on(async {
            let workflow = default_workflow("Workflow");
            let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
            {
                let mut session = backend.run_session.lock().await;
                let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
                run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
                session.workflow = Some(workflow);
                session.run_state = Some(run_state);
                session.action_tx = Some(action_tx);
            }

            let run_state = backend
                .submit_user_input("idea", "Continue with approvals".to_string())
                .await
                .expect("submit input");

            assert!(run_state.awaiting_node_id.is_none());
            assert_eq!(
                run_state
                    .chat_logs
                    .get(&NodeId("idea".to_string()))
                    .unwrap()
                    .last()
                    .unwrap()
                    .content,
                "Continue with approvals"
            );
            match action_rx.recv().await.expect("action") {
                ExecutionAction::ProvideInput(text) => {
                    assert_eq!(text, "Continue with approvals");
                }
                ExecutionAction::ResolveApproval { .. } => {
                    panic!("unexpected approval action");
                }
            }
        });
    }

    #[test]
    fn submit_tool_approval_updates_snapshot_and_sends_action() {
        let (backend, _dir) = backend();
        backend.runtime.block_on(async {
            let workflow = default_workflow("Workflow");
            let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
            {
                let mut session = backend.run_session.lock().await;
                let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
                run_state.pending_approvals = vec![workflow_core::PendingToolApproval {
                    approval_id: "approval-1".to_string(),
                    node_id: "idea".to_string(),
                    node_label: "Idea".to_string(),
                    tool_call: workflow_core::ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: serde_json::json!({ "path": "README.md" }),
                        intent: None,
                    },
                    tier: workflow_core::ToolTier::Read,
                }];
                session.workflow = Some(workflow);
                session.run_state = Some(run_state);
                session.action_tx = Some(action_tx);
            }

            let run_state = backend
                .submit_tool_approval("approval-1", true)
                .await
                .expect("submit approval");

            assert!(run_state.pending_approvals.is_empty());
            match action_rx.recv().await.expect("action") {
                ExecutionAction::ResolveApproval { approval_id, allow } => {
                    assert_eq!(approval_id, "approval-1");
                    assert!(allow);
                }
                ExecutionAction::ProvideInput(_) => {
                    panic!("unexpected input action");
                }
            }
        });
    }
}
