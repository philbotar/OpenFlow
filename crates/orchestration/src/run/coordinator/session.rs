//! Run session state and lifecycle helpers.

use crate::error::BackendError;
use crate::run::execution::{
    apply_event_to_run_state, spawn_interactive_workflow_run, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, NodeInterrupts,
};
use crate::run::persistence::{
    PendingRunCheckpoint, RunCheckpointPayload, RunRecord, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::prep::prepare_workflow_for_execution;
use crate::run::state::WorkflowRunState;
use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::provider::{resolve_provider_config, ProviderEnv};
use engine::ports::outbound::AiPort;
use engine::{
    resolve_callable_agent_snapshots, CallableAgent, InteractiveEngineCheckpoint, Workflow,
};
use parking_lot::Mutex as ParkingMutex;
use providers::{create_provider, ProviderId};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

pub(super) struct PreparedWorkflowRun {
    pub workflow: Workflow,
    pub ai: Box<dyn AiPort>,
    pub agent_snapshots: BTreeMap<String, CallableAgent>,
    pub persisted_settings: AppSettings,
    pub context_window_sizes: BTreeMap<String, u32>,
}

pub(super) struct ExecutionResources {
    pub snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    pub lsp_settings: crate::lsp::LspSettings,
    pub pending_engine_reverts: Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>,
    pub node_interrupts: NodeInterrupts,
    pub checkpoint_sink: Arc<ParkingMutex<Option<PendingRunCheckpoint>>>,
}

pub(super) struct SpawnRunInput {
    pub entrypoint: Option<String>,
    pub execution_cwd: PathBuf,
    pub project_id: Option<String>,
    pub artifact_root: PathBuf,
    pub resume_checkpoint: Option<InteractiveEngineCheckpoint>,
}

pub(super) struct SpawnedRun {
    pub event_rx: Option<UnboundedReceiver<ExecutionEvent>>,
    pub handle: tokio::task::JoinHandle<()>,
    pub action_tx: UnboundedSender<ExecutionAction>,
    pub cancel_token: CancellationToken,
}

/// Provider wiring shared by fresh start, in-session continue, and durable resume.
pub(super) fn prepare_workflow_run(
    workflow: Workflow,
    settings: &AppSettings,
    transient_api_key: Option<&str>,
    agent_store: &dyn crate::agent::ports::AgentStore,
    settings_store: &dyn crate::settings::ports::SettingsStore,
    env: &ProviderEnv,
) -> Result<PreparedWorkflowRun, BackendError> {
    let persisted_settings = settings_store.load()?;
    let mut provider_settings = settings.clone();
    merge_preserved_api_keys(&mut provider_settings, &persisted_settings);
    if let Some(provider_id) = workflow
        .settings
        .provider_id
        .as_ref()
        .filter(|provider_id| !provider_id.trim().is_empty())
    {
        provider_settings.active_provider = ProviderId::from(provider_id.as_str());
    }
    let provider_config = resolve_provider_config(&provider_settings, transient_api_key, env)?;
    let ai = create_provider(provider_config);
    let mut workflow = workflow;
    prepare_workflow_for_execution(&mut workflow, Some(provider_settings.active_profile()));
    let agents = agent_store.load()?;
    let agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);
    Ok(PreparedWorkflowRun {
        workflow,
        ai,
        agent_snapshots,
        persisted_settings,
        context_window_sizes: provider_settings
            .active_profile()
            .context_window_sizes
            .clone(),
    })
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
    }
}

pub(super) fn spawn_prepared_run(
    runtime_handle: &tokio::runtime::Handle,
    prepared: PreparedWorkflowRun,
    input: SpawnRunInput,
    resources: &ExecutionResources,
) -> SpawnedRun {
    let (handle, event_rx, action_tx, cancel_token, _) = spawn_interactive_workflow_run(
        runtime_handle,
        InteractiveWorkflowRunParams {
            workflow: prepared.workflow.clone(),
            entrypoint: input.entrypoint,
            execution_cwd: input.execution_cwd.clone(),
            project_repository_root: crate::run::execution::project_repository_root(
                input.project_id.as_deref(),
                &input.execution_cwd,
            ),
            artifact_root: input.artifact_root.clone(),
            resume_checkpoint: input.resume_checkpoint,
            checkpoint_sink: resources.checkpoint_sink.clone(),
            ai: prepared.ai,
            agent_snapshots: prepared.agent_snapshots,
            snapshot_store: resources.snapshot_store.clone(),
            lsp: resources.lsp_settings.clone(),
            pending_engine_reverts: resources.pending_engine_reverts.clone(),
            node_interrupts: resources.node_interrupts.clone(),
            context_window_sizes: prepared.context_window_sizes,
            mcp: prepared.persisted_settings.mcp.clone(),
        },
    );
    SpawnedRun {
        event_rx: Some(event_rx),
        handle,
        action_tx,
        cancel_token,
    }
}

pub(super) fn attach_execution_handles(
    session: &mut RunSession,
    workflow: Workflow,
    entrypoint: Option<String>,
    execution_cwd: PathBuf,
    artifact_root: PathBuf,
    resources: ExecutionResources,
    spawned: SpawnedRun,
) {
    let SpawnedRun {
        event_rx: _,
        handle,
        action_tx,
        cancel_token,
    } = spawned;
    let ExecutionResources {
        snapshot_store,
        lsp_settings,
        pending_engine_reverts,
        node_interrupts,
        checkpoint_sink,
    } = resources;
    session.workflow = Some(workflow);
    session.entrypoint = entrypoint;
    session.execution_cwd = Some(execution_cwd);
    session.artifact_root = Some(artifact_root);
    session.engine_checkpoint = None;
    session.checkpoint_sink = Some(checkpoint_sink);
    session.snapshot_store = Some(snapshot_store);
    session.lsp_settings = Some(lsp_settings);
    session.pending_engine_reverts = Some(pending_engine_reverts);
    session.action_tx = Some(action_tx);
    session.handle = Some(handle);
    session.cancel_token = Some(cancel_token);
    session.node_interrupts = Some(node_interrupts);
}

/// Clears session-scoped resources when a run becomes inactive.
pub(crate) fn finish_run_session(session: &mut RunSession) {
    session.snapshot_store = None;
    session.lsp_settings = None;
    session.pending_engine_reverts = None;
    session.action_tx = None;
    session.handle = None;
    session.cancel_token = None;
    session.node_interrupts = None;
    session.checkpoint_sink = None;
    session.engine_checkpoint = None;
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
        .checkpoint_sink
        .as_ref()
        .and_then(|sink| sink.lock().take());
    if let Some(checkpoint) = captured_checkpoint {
        session.engine_checkpoint = Some(checkpoint.engine);
    }
    session.checkpoint_sink = None;
    let workflow = session.workflow.clone()?;
    let run_state = session.run_state.as_mut()?;
    if run_state.active {
        apply_event_to_run_state(&workflow, run_state, ExecutionEvent::Aborted);
    }
    Some(run_state.clone())
}

#[derive(Debug)]
pub(crate) struct RunSession {
    pub(crate) workflow: Option<Workflow>,
    pub(crate) run_state: Option<WorkflowRunState>,
    pub(crate) run_id: Option<String>,
    pub(crate) run_root: Option<RunStoreRoot>,
    pub(crate) project_id: Option<String>,
    pub(crate) execution_cwd: Option<PathBuf>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) artifact_root: Option<PathBuf>,
    pub(crate) engine_checkpoint: Option<InteractiveEngineCheckpoint>,
    pub(crate) checkpoint_sink: Option<Arc<ParkingMutex<Option<PendingRunCheckpoint>>>>,
    pub(crate) snapshot_store:
        Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    pub(crate) lsp_settings: Option<crate::lsp::LspSettings>,
    pub(crate) pending_engine_reverts: Option<Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>>,
    pub(crate) action_tx: Option<UnboundedSender<ExecutionAction>>,
    pub(crate) handle: Option<tokio::task::JoinHandle<()>>,
    pub(crate) cancel_token: Option<CancellationToken>,
    pub(crate) node_interrupts: Option<NodeInterrupts>,
}

pub(super) enum TerminationMode {
    Replaced,
    UserStop,
}

pub struct RunStartParams<'a> {
    pub workflow: Workflow,
    pub entrypoint: Option<String>,
    pub execution_cwd: Option<String>,
    pub run_root: RunStoreRoot,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::ports::AgentStore,
    pub settings_store: &'a dyn crate::settings::ports::SettingsStore,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}

pub struct DurableResumeParams<'a> {
    pub run_id: &'a str,
    pub workflow: Workflow,
    pub root: RunStoreRoot,
    pub record: RunRecord,
    pub checkpoint: RunCheckpointPayload,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::ports::AgentStore,
    pub settings_store: &'a dyn crate::settings::ports::SettingsStore,
    pub run_store: &'a dyn RunCheckpointStore,
    pub env: &'a ProviderEnv,
}
