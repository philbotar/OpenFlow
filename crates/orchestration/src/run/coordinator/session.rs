//! Run session state and lifecycle helpers.

use crate::error::BackendError;
use crate::run::execution::{
    apply_event_to_run_state, spawn_interactive_workflow_run, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, NodeInterrupts, ProviderContextWindowSizes, ProviderRouter,
    ResumeContinuation,
};
use crate::run::persistence::{
    PendingRunCheckpoint, RunCheckpointPayload, RunRecord, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::prep::prepare_workflow_for_execution_with_profiles;
use crate::run::resources::{BudgetedAiPort, SharedRunResources};
use crate::run::skill_invocation::{
    apply_explicit_skill_invocations, apply_skill_invocations, has_skill_invocations, skill_paths,
    SkillPaths,
};
use crate::run::state::WorkflowRunState;
use crate::settings::model::{merge_preserved_secrets, AppSettings};
use crate::settings::ports::SkillCatalog;
use crate::settings::provider::{
    attach_codex_credential_sink, resolve_provider_config, ProviderEnv,
};
use engine::ports::outbound::AiPort;
use engine::{
    resolve_callable_agent_snapshots, CallableAgent, InteractiveEngineCheckpoint, Node, NodeId,
    NodeRuntimeConfigStore, Workflow,
};
use parking_lot::Mutex as ParkingMutex;
use providers::{create_provider, ProviderId};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub(super) struct PreparedWorkflowRun {
    pub workflow: Workflow,
    pub ai: Box<dyn AiPort>,
    pub agent_snapshots: BTreeMap<String, CallableAgent>,
    pub persisted_settings: AppSettings,
    pub context_window_sizes: ProviderContextWindowSizes,
    pub skill_paths: SkillPaths,
    pub mcp_clients: Option<crate::adapters::mcp::McpRunClients>,
    pub mcp_issues: Vec<crate::adapters::mcp::McpSetupIssue>,
}

pub(super) struct ExecutionResources {
    pub snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    pub lsp_settings: crate::lsp::LspSettings,
    pub pending_engine_reverts: Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>,
    pub node_interrupts: NodeInterrupts,
    pub checkpoint_sink: Arc<ParkingMutex<Option<PendingRunCheckpoint>>>,
    pub runtime_config_store: NodeRuntimeConfigStore,
}

pub(super) struct SpawnRunInput {
    pub metadata: RunLaunchMetadata,
    pub project_id: Option<String>,
    pub attachment_store: Arc<dyn crate::run::ports::RunAttachmentStore>,
    pub resume_checkpoint: Option<InteractiveEngineCheckpoint>,
    pub resume_continuation: Option<ResumeContinuation>,
    pub shared_resources: Arc<SharedRunResources>,
    pub mutation_gate: Option<Arc<tokio::sync::Semaphore>>,
}

#[derive(Clone)]
pub(super) struct RunLaunchMetadata {
    pub entrypoint: Option<String>,
    pub entrypoint_attachments: Vec<engine::ChatAttachmentRef>,
    pub execution_cwd: PathBuf,
    pub artifact_root: PathBuf,
    pub attachment_root: PathBuf,
}

pub(super) struct RunControl {
    pub handle: tokio::task::JoinHandle<()>,
    pub action_tx: UnboundedSender<ExecutionAction>,
    pub cancel_token: CancellationToken,
}

pub(super) struct ActiveRunResources {
    pub resources: ExecutionResources,
    pub control: Option<RunControl>,
}

/// Provider wiring shared by fresh start, in-session continue, and durable resume.
#[allow(
    clippy::too_many_arguments,
    reason = "run preparation composes explicit persistence, provider, agent, and skill ports"
)]
pub(super) fn prepare_workflow_run(
    workflow: Workflow,
    invoked_skill_ids: &[String],
    settings: &AppSettings,
    transient_api_key: Option<&str>,
    agent_store: &dyn crate::agent::AgentStore,
    skill_catalog: &dyn SkillCatalog,
    settings_store: Arc<dyn crate::settings::ports::SettingsStore>,
    env: &ProviderEnv,
    shared_resources: Arc<SharedRunResources>,
) -> Result<PreparedWorkflowRun, BackendError> {
    let persisted_settings = settings_store.load()?;
    let mut provider_settings = settings.clone();
    merge_preserved_secrets(&mut provider_settings, &persisted_settings);
    if let Some(provider_id) = workflow
        .settings
        .provider_id
        .as_ref()
        .filter(|provider_id| !provider_id.trim().is_empty())
    {
        provider_settings.active_provider = ProviderId::from(provider_id.as_str());
    }
    let default_provider_id = provider_settings.active_provider.clone();
    let mut required_provider_ids = BTreeSet::from([default_provider_id.clone()]);
    required_provider_ids.extend(workflow.nodes.iter().filter_map(|node| {
        node.agent
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|provider_id| !provider_id.is_empty())
            .map(ProviderId::from)
    }));
    let mut provider_clients = BTreeMap::new();
    let mut context_window_sizes = BTreeMap::new();
    for provider_id in required_provider_ids {
        let mut selected_settings = provider_settings.clone();
        selected_settings.active_provider = provider_id.clone();
        let selected_transient_key = if provider_id == default_provider_id {
            transient_api_key
        } else {
            None
        };
        let mut provider_config =
            resolve_provider_config(&selected_settings, selected_transient_key, env)?;
        attach_codex_credential_sink(&mut provider_config, Arc::clone(&settings_store));
        context_window_sizes.insert(
            provider_id.to_string(),
            selected_settings
                .active_profile()
                .context_window_sizes
                .clone(),
        );
        provider_clients.insert(provider_id.to_string(), create_provider(provider_config));
    }
    let ai: Box<dyn AiPort> = Box::new(BudgetedAiPort::new(
        Box::new(ProviderRouter::new(
            default_provider_id.to_string(),
            provider_clients,
        )),
        shared_resources,
    ));
    let mut workflow = workflow;
    prepare_workflow_for_execution_with_profiles(
        &mut workflow,
        &default_provider_id,
        &provider_settings.providers,
    )?;
    let agents = agent_store.load()?;
    let mut agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);
    let skills = skill_catalog.discover(&provider_settings.skill_search_paths)?;
    let resolved_skill_paths = skill_paths(&skills);
    if has_skill_invocations(&workflow, &agent_snapshots) {
        apply_skill_invocations(&mut workflow, &mut agent_snapshots, &skills)?;
    }
    apply_explicit_skill_invocations(&mut workflow, invoked_skill_ids, &resolved_skill_paths)?;
    Ok(PreparedWorkflowRun {
        workflow,
        ai,
        agent_snapshots,
        persisted_settings,
        context_window_sizes,
        skill_paths: resolved_skill_paths,
        mcp_clients: None,
        mcp_issues: Vec::new(),
    })
}

pub(super) async fn resolve_mcp_context_for_run(
    prepared: &mut PreparedWorkflowRun,
    execution_cwd: &std::path::Path,
    project_root: Option<&std::path::Path>,
) {
    if prepared.workflow.nodes.iter().all(|node| {
        node.agent.mcp_resources.is_empty()
            && node.agent.mcp_prompts.is_empty()
            && node.agent.mcp_context_snapshots.is_empty()
    }) {
        return;
    }
    let effective_servers = crate::adapters::mcp::effective_mcp_servers(
        &prepared.persisted_settings.mcp,
        execution_cwd,
    );
    let effective_mcp = crate::settings::model::McpSettings {
        servers: effective_servers,
        discover_external: prepared.persisted_settings.mcp.discover_external,
        disabled_discovered_ids: prepared
            .persisted_settings
            .mcp
            .disabled_discovered_ids
            .clone(),
        registry_base_url: prepared.persisted_settings.mcp.registry_base_url.clone(),
    };
    let (clients, issues) =
        crate::adapters::mcp::McpRunClients::connect_for_run(&effective_mcp, project_root).await;
    clients
        .resolve_workflow_context(&mut prepared.workflow)
        .await;
    prepared.mcp_clients = Some(clients);
    prepared.mcp_issues = issues;
}

pub(crate) fn fresh_execution_resources(persisted_settings: &AppSettings) -> ExecutionResources {
    ExecutionResources {
        snapshot_store: Arc::new(
            crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
        ),
        lsp_settings: persisted_settings.lsp.runtime(),
        pending_engine_reverts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        node_interrupts: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
        checkpoint_sink: Arc::new(ParkingMutex::new(None)),
        runtime_config_store: engine::new_runtime_config_store(),
    }
}

pub(super) fn spawn_prepared_run(
    runtime_handle: &tokio::runtime::Handle,
    mut prepared: PreparedWorkflowRun,
    input: SpawnRunInput,
    resources: &ExecutionResources,
) -> (RunControl, UnboundedReceiver<ExecutionEvent>) {
    let SpawnRunInput {
        metadata,
        project_id,
        attachment_store,
        resume_checkpoint,
        resume_continuation,
        shared_resources,
        mutation_gate,
    } = input;
    let (handle, event_rx, action_tx, cancel_token, _) = spawn_interactive_workflow_run(
        runtime_handle,
        InteractiveWorkflowRunParams {
            workflow: prepared.workflow.clone(),
            entrypoint: metadata.entrypoint,
            entrypoint_attachments: metadata.entrypoint_attachments,
            execution_cwd: metadata.execution_cwd.clone(),
            project_repository_root: crate::run::execution::project_repository_root(
                project_id.as_deref(),
                &metadata.execution_cwd,
            ),
            artifact_root: metadata.artifact_root,
            attachment_root: metadata.attachment_root,
            attachment_store,
            resume_checkpoint,
            resume_continuation,
            checkpoint_sink: resources.checkpoint_sink.clone(),
            ai: prepared.ai,
            agent_snapshots: prepared.agent_snapshots,
            snapshot_store: resources.snapshot_store.clone(),
            lsp: resources.lsp_settings.clone(),
            pending_engine_reverts: resources.pending_engine_reverts.clone(),
            node_interrupts: resources.node_interrupts.clone(),
            context_window_sizes: prepared.context_window_sizes,
            mcp: prepared.persisted_settings.mcp.clone(),
            prepared_mcp: prepared
                .mcp_clients
                .take()
                .map(|clients| (clients, std::mem::take(&mut prepared.mcp_issues))),
            search: prepared.persisted_settings.search.clone(),
            runtime_config_store: resources.runtime_config_store.clone(),
            tool_budget: shared_resources.tool_budget(),
            mutation_gate,
        },
    );
    (
        RunControl {
            handle,
            action_tx,
            cancel_token,
        },
        event_rx,
    )
}

pub(super) struct RunLaunchTail {
    pub spawn_input: SpawnRunInput,
    pub resources: ExecutionResources,
}

/// Shared tail for fresh start, in-session continue, and durable resume launches.
pub(super) async fn finalize_run_launch(
    runtime_handle: &tokio::runtime::Handle,
    session: &Mutex<RunSession>,
    prepared: PreparedWorkflowRun,
    tail: RunLaunchTail,
    configure_session: impl FnOnce(&mut RunSession) -> Result<WorkflowRunState, BackendError>,
) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
    let workflow = prepared.workflow.clone();
    let skill_paths = prepared.skill_paths.clone();
    let RunLaunchTail {
        spawn_input,
        resources,
    } = tail;
    let metadata = spawn_input.metadata.clone();
    let (spawned, event_rx) = spawn_prepared_run(runtime_handle, prepared, spawn_input, &resources);
    let mut session_guard = session.lock().await;
    let initial_state = configure_session(&mut session_guard)?;
    attach_execution_handles(
        &mut session_guard,
        workflow,
        metadata,
        resources,
        skill_paths,
        spawned,
    );
    Ok((initial_state, event_rx))
}

#[allow(
    clippy::too_many_arguments,
    reason = "single session mutation installs one complete run launch atomically"
)]
pub(super) fn attach_execution_handles(
    session: &mut RunSession,
    workflow: Workflow,
    metadata: RunLaunchMetadata,
    resources: ExecutionResources,
    skill_paths: SkillPaths,
    control: RunControl,
) {
    session.workflow = Some(workflow);
    session.skill_paths = skill_paths;
    session.entrypoint = metadata.entrypoint;
    session.entrypoint_attachments = metadata.entrypoint_attachments;
    session.execution_cwd = Some(metadata.execution_cwd);
    session.artifact_root = Some(metadata.artifact_root);
    session.attachment_root = Some(metadata.attachment_root);
    session.generation = session.generation.wrapping_add(1);
    session.engine_checkpoint = None;
    session.active = Some(ActiveRunResources {
        resources,
        control: Some(control),
    });
}

/// Clears session-scoped resources when a run becomes inactive.
pub(crate) fn finish_run_session(session: &mut RunSession) {
    session.generation = session.generation.wrapping_add(1);
    session.active = None;
    session.engine_checkpoint = None;
    session.skill_paths.clear();
}

pub(crate) fn clear_artifact_root(session: &mut RunSession) {
    let Some(path) = session.artifact_root.take() else {
        return;
    };
    if let Err(error) = fs::remove_dir_all(&path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("failed to remove artifact root {}: {error}", path.display());
        }
    }
}

/// Marks the in-session run as user-stopped and captures a resume checkpoint when present.
pub(crate) fn apply_user_stop_to_session(session: &mut RunSession) -> Option<WorkflowRunState> {
    let captured_checkpoint = session
        .active
        .as_ref()
        .and_then(|active| active.resources.checkpoint_sink.lock().clone());
    if let Some(checkpoint) = captured_checkpoint {
        session.engine_checkpoint = Some(checkpoint.engine);
    }
    if let Some(active) = session.active.as_mut() {
        active.resources.checkpoint_sink = Arc::new(ParkingMutex::new(None));
    }
    let workflow = session.workflow.clone()?;
    let run_state = session.run_state.as_mut()?;
    if run_state.active {
        apply_event_to_run_state(&workflow, run_state, ExecutionEvent::Aborted);
    }
    Some(run_state.clone())
}

pub(super) fn require_run_state(session: &RunSession) -> Result<&WorkflowRunState, BackendError> {
    session.run_state.as_ref().ok_or(BackendError::NoActiveRun)
}

pub(super) fn require_active_run_state(
    session: &RunSession,
) -> Result<&WorkflowRunState, BackendError> {
    let run_state = require_run_state(session)?;
    if run_state.active {
        Ok(run_state)
    } else {
        Err(BackendError::NoActiveRun)
    }
}

pub(super) fn require_run_state_mut(
    session: &mut RunSession,
) -> Result<&mut WorkflowRunState, BackendError> {
    session.run_state.as_mut().ok_or(BackendError::NoActiveRun)
}

pub(super) fn require_workflow_mut(
    session: &mut RunSession,
) -> Result<&mut Workflow, BackendError> {
    session.workflow.as_mut().ok_or(BackendError::NoActiveRun)
}

pub(super) fn require_action_tx(
    session: &RunSession,
) -> Result<&UnboundedSender<ExecutionAction>, BackendError> {
    session
        .active
        .as_ref()
        .and_then(|active| active.control.as_ref())
        .map(|control| &control.action_tx)
        .ok_or(BackendError::NoActiveRun)
}

pub(super) fn require_node_mut<'a>(
    workflow: &'a mut Workflow,
    node_id: &str,
) -> Result<&'a mut Node, BackendError> {
    let node_id_key = NodeId(node_id.to_string());
    workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id_key)
        .ok_or_else(|| BackendError::NodeNotFoundInRun(node_id.to_string()))
}

pub(crate) struct RunSession {
    pub(crate) workflow: Option<Workflow>,
    pub(crate) run_state: Option<WorkflowRunState>,
    pub(crate) run_id: Option<String>,
    pub(crate) run_root: Option<RunStoreRoot>,
    pub(crate) project_id: Option<String>,
    pub(crate) skill_paths: SkillPaths,
    pub(crate) execution_cwd: Option<PathBuf>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) entrypoint_attachments: Vec<engine::ChatAttachmentRef>,
    pub(crate) artifact_root: Option<PathBuf>,
    pub(crate) attachment_root: Option<PathBuf>,
    pub(crate) generation: u64,
    pub(crate) engine_checkpoint: Option<InteractiveEngineCheckpoint>,
    pub(crate) active: Option<ActiveRunResources>,
}

pub(super) enum TerminationMode {
    Replaced,
    UserStop,
}

pub struct RunStartParams<'a> {
    pub workflow: Workflow,
    pub invoked_skill_ids: Vec<String>,
    pub entrypoint: Option<crate::api::UserMessageInput>,
    pub execution_cwd: Option<String>,
    pub run_root: RunStoreRoot,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::AgentStore,
    pub skill_catalog: &'a dyn SkillCatalog,
    pub settings_store: Arc<dyn crate::settings::ports::SettingsStore>,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}

pub struct DurableResumeParams<'a> {
    pub run_id: &'a str,
    pub root: RunStoreRoot,
    pub record: RunRecord,
    pub checkpoint: RunCheckpointPayload,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::AgentStore,
    pub skill_catalog: &'a dyn SkillCatalog,
    pub settings_store: Arc<dyn crate::settings::ports::SettingsStore>,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}
