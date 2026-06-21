//! Run session state and lifecycle helpers.

use crate::run::execution::{apply_event_to_run_state, ExecutionEvent};
use crate::run::persistence::{
    PendingRunCheckpoint, RunCheckpointPayload, RunRecord, RunStoreRoot,
};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::WorkflowRunState;
use crate::settings::model::AppSettings;
use crate::settings::provider::ProviderEnv;
use engine::{InteractiveEngineCheckpoint, Workflow};
use parking_lot::Mutex as ParkingMutex;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::run::execution::{ExecutionAction, NodeInterrupts};

/// Clears session-scoped resources when a run becomes inactive.
pub(super) fn finish_run_session(session: &mut RunSession) {
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

pub(super) fn clear_artifact_root(session: &mut RunSession) {
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
pub(super) fn apply_user_stop_to_session(session: &mut RunSession) -> Option<WorkflowRunState> {
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
pub(super) struct RunSession {
    pub(super) workflow: Option<Workflow>,
    pub(super) run_state: Option<WorkflowRunState>,
    pub(super) run_id: Option<String>,
    pub(super) run_root: Option<RunStoreRoot>,
    pub(super) project_id: Option<String>,
    pub(super) execution_cwd: Option<PathBuf>,
    pub(super) entrypoint: Option<String>,
    pub(super) artifact_root: Option<PathBuf>,
    pub(super) engine_checkpoint: Option<InteractiveEngineCheckpoint>,
    pub(super) checkpoint_sink: Option<Arc<ParkingMutex<Option<PendingRunCheckpoint>>>>,
    pub(super) snapshot_store:
        Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    pub(super) lsp_settings: Option<crate::lsp::LspSettings>,
    pub(super) pending_engine_reverts: Option<Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>>,
    pub(super) action_tx: Option<UnboundedSender<ExecutionAction>>,
    pub(super) handle: Option<tokio::task::JoinHandle<()>>,
    pub(super) cancel_token: Option<CancellationToken>,
    pub(super) node_interrupts: Option<NodeInterrupts>,
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
