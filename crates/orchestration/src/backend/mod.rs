use crate::adapters::storage::agent_store::FileAgentStore;
use crate::agent::ports::AgentStore;
use crate::agent_library::AgentLibrary;
use crate::execution::ExecutionEvent;
use crate::flow_store::FileProjectWorkflowStore;
use crate::project::ports::{Project, ProjectStore};
use crate::project_registry::ProjectRegistry;
use crate::project_store::FileProjectStore;
use crate::run_coordinator::{RunCoordinator, RunStartParams};
use crate::settings::model::AppSettings;
use crate::settings::ports::{SettingsStore, SkillCatalog, SkillSummary};
use crate::settings::provider::ProviderEnv;
use crate::settings_facade::SettingsFacade;
use crate::settings_store::FileSettingsStore;
use crate::skill_store::FileSkillCatalog;
use crate::state::WorkflowRunState;
use crate::storage::FileWorkflowStore;
use crate::workflow::ports::{ProjectWorkflowStore, WorkflowStore};
use crate::workflow_catalog::WorkflowCatalog;
use engine::{CallableAgent, Node, Workflow};
use tokio::sync::mpsc::UnboundedReceiver;

pub use crate::api::{
    AgentDefinitionSummary, FileEditPreview, ProviderReadiness, WorkflowListItem,
    WorkflowValidationSummary,
};
pub use crate::error::BackendError;

pub struct AppBackendDeps {
    pub workflow_store: Box<dyn WorkflowStore>,
    pub project_workflow_store: Box<dyn ProjectWorkflowStore>,
    pub agent_store: Box<dyn AgentStore>,
    pub project_store: Box<dyn ProjectStore>,
    pub settings_store: Box<dyn SettingsStore>,
    pub skill_catalog: Box<dyn SkillCatalog>,
    pub env: ProviderEnv,
    pub runtime: tokio::runtime::Runtime,
}

pub struct AppBackend {
    workflows: WorkflowCatalog,
    agents: AgentLibrary,
    projects: ProjectRegistry,
    settings: SettingsFacade,
    runs: RunCoordinator,
}

impl AppBackend {
    #[must_use]
    pub fn new(deps: AppBackendDeps) -> Self {
        Self {
            workflows: WorkflowCatalog::new(deps.workflow_store, deps.project_workflow_store),
            agents: AgentLibrary::new(deps.agent_store),
            projects: ProjectRegistry::new(deps.project_store),
            settings: SettingsFacade::new(deps.settings_store, deps.skill_catalog, deps.env),
            runs: RunCoordinator::new(deps.runtime),
        }
    }

    #[must_use]
    pub fn with_default_paths() -> Self {
        Self::new(AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(FileWorkflowStore::default_path())),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(FileAgentStore::default_path())),
            project_store: Box::new(FileProjectStore::new(FileProjectStore::default_path())),
            settings_store: Box::new(FileSettingsStore::new(FileSettingsStore::default_path())),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_system(),
            runtime: tokio::runtime::Runtime::new().expect("failed to create tokio runtime"),
        })
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowListItem>, BackendError> {
        self.workflows.list(&self.projects)
    }

    pub fn load_all_workflows(&self) -> Result<Vec<Workflow>, BackendError> {
        self.workflows.load_all(&self.projects)
    }

    pub fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, BackendError> {
        self.workflows.load_one(&self.projects, workflow_id)
    }

    pub fn create_workflow(&self, name: String) -> Result<Workflow, BackendError> {
        self.workflows.create(name)
    }

    pub fn save_workflow(&self, workflow: Workflow) -> Result<Workflow, BackendError> {
        self.workflows.save_one(&self.projects, workflow)
    }

    pub fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), BackendError> {
        self.workflows.save_all(&self.projects, workflows)
    }

    pub fn rename_workflow(
        &self,
        workflow_id: &str,
        name: String,
    ) -> Result<WorkflowListItem, BackendError> {
        self.workflows.rename(&self.projects, workflow_id, name)
    }

    pub fn load_agents(&self) -> Result<Vec<CallableAgent>, BackendError> {
        self.agents.load()
    }

    pub fn save_agents(&self, agents: &[CallableAgent]) -> Result<(), BackendError> {
        self.agents.save(agents)
    }

    pub fn create_agent_definition(&self, name: String) -> Result<CallableAgent, BackendError> {
        self.agents.create(name)
    }

    pub fn create_agent_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        self.agents.create_node(index, x, y, agent_id)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        self.agents.list()
    }

    pub fn list_skills(&self) -> Result<Vec<SkillSummary>, BackendError> {
        self.settings.list_skills()
    }

    pub fn load_settings(&self) -> Result<AppSettings, BackendError> {
        self.settings.load()
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.settings.save(settings)
    }

    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        self.settings.load_provider_api_key(provider_id)
    }

    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        self.settings.save_provider_api_key(provider_id, api_key)
    }

    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        self.settings.delete_provider_api_key(provider_id)
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        self.settings
            .resolve_provider_readiness(settings, transient_api_key)
    }

    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        self.settings.validate_workflow(workflow)
    }

    pub fn load_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.load()
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        self.projects.list()
    }

    pub fn save_projects(&self, projects: &[Project]) -> Result<(), BackendError> {
        self.projects.save(projects)
    }

    pub fn create_project_from_directory(&self, path: String) -> Result<Project, BackendError> {
        self.projects.create_from_directory(path)
    }

    pub fn assign_workflow_to_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.workflows
            .assign_to_project(&self.projects, project_id, workflow_id)
    }

    pub fn unassign_workflow_from_project(
        &self,
        project_id: &str,
        workflow_id: &str,
    ) -> Result<Vec<Project>, BackendError> {
        self.workflows
            .unassign_from_project(&self.projects, project_id, workflow_id)
    }

    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        execution_cwd: Option<String>,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                env: self.settings.env(),
            })
            .await
    }

    /// Stops the active workflow run cooperatively.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no run session to stop.
    pub async fn stop_run(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.stop_run().await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.runs.is_run_active().await
    }

    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.apply_execution_event(event).await
    }

    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.submit_user_input(node_id, text).await
    }

    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.submit_tool_approval(approval_id, allow).await
    }

    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.complete_manual_node().await
    }

    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.runs.get_run_state().await
    }

    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.runs
            .preview_file_edit(approval_id, tool_name, arguments)
            .await
    }

    pub async fn git_diff_file(&self, path: String) -> Result<String, BackendError> {
        self.runs.git_diff_file(path).await
    }

    pub async fn revert_edit_batch(
        &self,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.revert_edit_batch(batch_id).await
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        self.runs.clear_run_trace().await
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::execution::{ExecutionAction, ExecutionEvent};
    use crate::settings::model::{ProviderProfile, ProviderTransport};
    use crate::workflow_catalog::default_workflow;
    use engine::{Node, NodeId};
    use providers::ProviderId;
    use tempfile::tempdir;

    fn backend() -> (AppBackend, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let backend = AppBackend::new(AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(dir.path().join("workflows.json"))),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(dir.path().join("agents.json"))),
            project_store: Box::new(FileProjectStore::new(dir.path().join("projects.json"))),
            settings_store: Box::new(FileSettingsStore::new(dir.path().join("settings.json"))),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_pairs([
                ("OPENAI_API_KEY", "openai-key"),
                ("OPENAI_COMPATIBLE_API_KEY", "compatible-key"),
            ]),
            runtime: tokio::runtime::Runtime::new().expect("runtime"),
        });
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
        assert_eq!(node.agent, engine::AgentNodeConfig::default());
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
        agent.tools.catalog.tools = vec![engine::ToolRef {
            name: "search".to_string(),
            tier: Some(engine::ToolTier::Read),
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
            vec![engine::ToolRef {
                name: "search".to_string(),
                tier: Some(engine::ToolTier::Read),
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

        let readiness = AppBackend::new(AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new("/tmp/unused-workflows.json")),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new("/tmp/unused-agents.json")),
            project_store: Box::new(FileProjectStore::new("/tmp/unused-projects.json")),
            settings_store: Box::new(FileSettingsStore::new("/tmp/unused-settings.json")),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::default(),
            runtime: tokio::runtime::Runtime::new().expect("runtime"),
        })
        .resolve_provider_readiness(&settings, None);

        assert!(!readiness.ready);
        assert_eq!(readiness.env_var, "OPENAI_COMPATIBLE_API_KEY");
    }

    #[test]
    fn start_run_returns_initial_state_and_manual_events() {
        let (backend, _dir) = backend();
        backend.runs.runtime().block_on(async {
            let mut workflow = Workflow::new("Manual run");
            let mut node = Node::agent("Review", 0.0, 0.0);
            node.id = NodeId("review".to_string());
            node.agent.auto_start = false;
            workflow.nodes = vec![node];

            let (initial_state, mut event_rx) = backend
                .start_run(workflow, None, None, &AppSettings::default(), None)
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

            let stopped = backend.stop_run().await.expect("stop run");
            assert!(!stopped.active);
            assert!(stopped.last_error.is_none());
        });
    }

    #[test]
    fn stop_run_is_idempotent_when_inactive() {
        let (backend, _dir) = backend();
        backend.runs.runtime().block_on(async {
            let workflow = default_workflow("Workflow");
            let run_state = WorkflowRunState::idle_for_workflow(&workflow);
            backend
                .runs
                .test_seed_session(workflow.clone(), run_state.clone(), {
                    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                    tx
                })
                .await;

            let snapshot = backend.stop_run().await.expect("stop inactive run");
            assert!(!snapshot.active);
        });
    }

    #[test]
    fn submit_user_input_updates_snapshot_and_sends_action() {
        let (backend, _dir) = backend();
        backend.runs.runtime().block_on(async {
            let workflow = default_workflow("Workflow");
            let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
            let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
            run_state.awaiting_node_id = Some(NodeId("idea".to_string()));
            backend
                .runs
                .test_seed_session(workflow, run_state, action_tx)
                .await;

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
                ExecutionAction::Stop => panic!("unexpected stop action"),
            }
        });
    }

    #[test]
    fn submit_tool_approval_updates_snapshot_and_sends_action() {
        let (backend, _dir) = backend();
        backend.runs.runtime().block_on(async {
            let workflow = default_workflow("Workflow");
            let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
            let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
            run_state.pending_approvals = vec![engine::PendingToolApproval {
                approval_id: "approval-1".to_string(),
                node_id: "idea".to_string(),
                node_label: "Idea".to_string(),
                tool_call: engine::ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({ "path": "README.md" }),
                    intent: None,
                },
                tier: engine::ToolTier::Read,
            }];
            backend
                .runs
                .test_seed_session(workflow, run_state, action_tx)
                .await;

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
                ExecutionAction::Stop => panic!("unexpected stop action"),
            }
        });
    }

    #[test]
    fn assign_workflow_to_project_round_trips() {
        let (backend, _dir) = backend();
        let workflow = backend
            .create_workflow("Flow".to_string())
            .expect("create workflow");
        let project = backend
            .create_project_from_directory(std::env::temp_dir().to_string_lossy().into_owned())
            .expect("create project");

        let projects = backend
            .assign_workflow_to_project(&project.id, &workflow.id.to_string())
            .expect("assign workflow");

        assert_eq!(projects[0].workflow_ids, vec![workflow.id.to_string()]);
        let loaded = backend.load_projects().expect("load projects");
        assert_eq!(loaded[0].workflow_ids, vec![workflow.id.to_string()]);
    }
}
