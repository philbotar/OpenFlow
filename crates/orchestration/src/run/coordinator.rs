use crate::api::FileEditPreview;
use crate::error::BackendError;
use crate::run::execution::{
    apply_event_to_run_state, new_artifact_root, record_entrypoint_message, record_user_input,
    resolve_execution_cwd, spawn_interactive_workflow_run, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, NodeInterrupts,
};
use crate::run::reasoning_defaults::apply_provider_reasoning_defaults;
use crate::run::state::{AgentStatus, WorkflowRunState};
use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::provider::{resolve_provider_config, ProviderEnv};
use crate::tools::edit::preview::preview_file_edit;
use engine::resolve_callable_agent_snapshots;
use engine::{execution_layers, validate_workflow, InteractiveEngineCheckpoint, NodeId, Workflow};
use parking_lot::Mutex as ParkingMutex;
use providers::{create_provider, ProviderId};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Clears session-scoped resources when a run becomes inactive.
fn finish_run_session(session: &mut RunSession) {
    session.snapshot_store = None;
    session.lsp_settings = None;
    session.pending_engine_reverts = None;
    session.action_tx = None;
    session.handle = None;
    session.cancel_token = None;
    session.node_interrupts = None;
    session.checkpoint_sink = None;
    if session.engine_checkpoint.is_none() {
        clear_artifact_root(session);
    }
}

fn clear_artifact_root(session: &mut RunSession) {
    let Some(path) = session.artifact_root.take() else {
        return;
    };
    if let Err(error) = fs::remove_dir_all(&path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!("failed to remove artifact root {}: {error}", path.display());
        }
    }
}

#[derive(Debug)]
struct RunSession {
    workflow: Option<Workflow>,
    run_state: Option<WorkflowRunState>,
    execution_cwd: Option<PathBuf>,
    entrypoint: Option<String>,
    artifact_root: Option<PathBuf>,
    engine_checkpoint: Option<InteractiveEngineCheckpoint>,
    checkpoint_sink: Option<Arc<ParkingMutex<Option<InteractiveEngineCheckpoint>>>>,
    snapshot_store: Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    lsp_settings: Option<crate::lsp::LspSettings>,
    pending_engine_reverts: Option<Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>>,
    action_tx: Option<UnboundedSender<ExecutionAction>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    cancel_token: Option<CancellationToken>,
    node_interrupts: Option<NodeInterrupts>,
}

enum TerminationMode {
    Replaced,
    UserStop,
}

pub struct RunStartParams<'a> {
    pub workflow: Workflow,
    pub entrypoint: Option<String>,
    pub execution_cwd: Option<String>,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a dyn crate::agent::ports::AgentStore,
    pub settings_store: &'a dyn crate::settings::ports::SettingsStore,
    pub env: &'a ProviderEnv,
}

#[derive(Debug)]
pub struct RunCoordinator {
    runtime_handle: tokio::runtime::Handle,
    session: Mutex<RunSession>,
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod coordinator_tests;

impl RunCoordinator {
    #[must_use]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            runtime_handle,
            session: Mutex::new(RunSession {
                workflow: None,
                run_state: None,
                execution_cwd: None,
                entrypoint: None,
                artifact_root: None,
                engine_checkpoint: None,
                checkpoint_sink: None,
                snapshot_store: None,
                lsp_settings: None,
                pending_engine_reverts: None,
                action_tx: None,
                handle: None,
                cancel_token: None,
                node_interrupts: None,
            }),
        }
    }

    /// # Errors
    /// Returns an error if the workflow fails validation or provider configuration fails.
    pub async fn start_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let RunStartParams {
            workflow,
            entrypoint,
            execution_cwd,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            env,
        } = params;

        validate_workflow(&workflow)?;
        let cwd_arg = execution_cwd.clone();
        let resolved_cwd = self
            .runtime_handle
            .spawn_blocking(move || resolve_execution_cwd(cwd_arg.as_deref()))
            .await
            .map_err(|error| BackendError::PreviewFailed(error.to_string()))?
            .map_err(BackendError::InvalidExecutionCwd)?;
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
        apply_provider_reasoning_defaults(&mut workflow, provider_settings.active_profile());

        let agents = agent_store.load()?;
        let agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);

        self.terminate_active_run(TerminationMode::Replaced).await;
        {
            let mut session = self.session.lock().await;
            session.engine_checkpoint = None;
            clear_artifact_root(&mut session);
        }

        let snapshot_store =
            Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new());
        let lsp_settings = crate::lsp::LspSettings::from_persisted(&persisted_settings.lsp);
        let pending_engine_reverts = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let node_interrupts: NodeInterrupts =
            Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::new()));
        let artifact_root = new_artifact_root();
        let checkpoint_sink = Arc::new(ParkingMutex::new(None));
        let (handle, event_rx, action_tx, cancel_token, _) = spawn_interactive_workflow_run(
            &self.runtime_handle,
            InteractiveWorkflowRunParams {
                workflow: workflow.clone(),
                entrypoint: entrypoint.clone(),
                execution_cwd: resolved_cwd.clone(),
                artifact_root: artifact_root.clone(),
                resume_checkpoint: None,
                checkpoint_sink: checkpoint_sink.clone(),
                ai,
                agent_snapshots,
                snapshot_store: snapshot_store.clone(),
                lsp: lsp_settings.clone(),
                pending_engine_reverts: pending_engine_reverts.clone(),
                node_interrupts: node_interrupts.clone(),
            },
        );

        let mut session = self.session.lock().await;
        let mut initial_state = WorkflowRunState::running_for_workflow(&workflow);
        if let Some(text) = entrypoint.clone().filter(|t| !t.trim().is_empty()) {
            if let Ok(layers) = execution_layers(&workflow) {
                if let Some(root_id) = layers.first().and_then(|layer| layer.first()) {
                    record_entrypoint_message(&mut initial_state, &root_id.0, text);
                }
            }
        }
        session.workflow = Some(workflow);
        session.run_state = Some(initial_state.clone());
        session.execution_cwd = Some(resolved_cwd);
        session.entrypoint = entrypoint;
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
        Ok((initial_state, event_rx))
    }

    /// Resume a stopped run from the latest in-session checkpoint.
    ///
    /// # Errors
    /// Returns an error when there is no continuable run or provider configuration fails.
    pub async fn continue_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let RunStartParams {
            workflow,
            entrypoint,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            env,
            ..
        } = params;

        validate_workflow(&workflow)?;
        let (
            checkpoint,
            artifact_root,
            execution_cwd,
            snapshot_store,
            lsp_settings,
            pending_engine_reverts,
        ) = {
            let session = self.session.lock().await;
            if session.run_state.as_ref().is_some_and(|state| state.active) {
                return Err(BackendError::NoContinuableRun);
            }
            let checkpoint = session
                .engine_checkpoint
                .clone()
                .ok_or(BackendError::NoContinuableRun)?;
            if checkpoint.workflow_id != workflow.id {
                return Err(BackendError::CheckpointWorkflowMismatch);
            }
            (
                checkpoint,
                session
                    .artifact_root
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .execution_cwd
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .snapshot_store
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .lsp_settings
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .pending_engine_reverts
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
            )
        };
        engine::validate_checkpoint_against_workflow(&workflow, &checkpoint)
            .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

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
        apply_provider_reasoning_defaults(&mut workflow, provider_settings.active_profile());

        let agents = agent_store.load()?;
        let agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);

        self.terminate_active_run(TerminationMode::Replaced).await;

        let node_interrupts: NodeInterrupts =
            Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::new()));
        let checkpoint_sink = Arc::new(ParkingMutex::new(None));
        let (handle, event_rx, action_tx, cancel_token, _) = spawn_interactive_workflow_run(
            &self.runtime_handle,
            InteractiveWorkflowRunParams {
                workflow: workflow.clone(),
                entrypoint: entrypoint.clone(),
                execution_cwd: execution_cwd.clone(),
                artifact_root: artifact_root.clone(),
                resume_checkpoint: Some(checkpoint),
                checkpoint_sink: checkpoint_sink.clone(),
                ai,
                agent_snapshots,
                snapshot_store: snapshot_store.clone(),
                lsp: lsp_settings.clone(),
                pending_engine_reverts: pending_engine_reverts.clone(),
                node_interrupts: node_interrupts.clone(),
            },
        );

        let mut session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoContinuableRun)?;
        run_state.active = true;
        let resumed_state = run_state.clone();
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
        Ok((resumed_state, event_rx))
    }

    #[must_use]
    pub async fn is_run_continuable(&self) -> bool {
        let session = self.session.lock().await;
        session.engine_checkpoint.is_some()
            && session
                .run_state
                .as_ref()
                .is_some_and(|state| !state.active)
    }

    /// Cancel the in-flight AI invocation for a running node without stopping the run.
    ///
    /// # Errors
    /// Returns an error when there is no active run or the node is not interruptible.
    pub async fn interrupt_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        let session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        let node_id_key = NodeId(node_id.to_string());
        let status = run_state
            .status_by_node
            .get(&node_id_key)
            .copied()
            .unwrap_or(AgentStatus::Idle);
        if !matches!(status, AgentStatus::Started | AgentStatus::RunningTool) {
            return Err(BackendError::NodeNotInterruptible(node_id.to_string()));
        }
        if let Some(interrupts) = &session.node_interrupts {
            if let Some((_, token)) = interrupts.lock().get(&node_id_key) {
                token.cancel();
            }
        }
        Ok(run_state.clone())
    }

    /// Retry a failed or interrupted node, preserving its transcript.
    ///
    /// # Errors
    /// Returns an error when there is no active run or the node is not retryable.
    pub async fn retry_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        let session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        let node_id_key = NodeId(node_id.to_string());
        let status = run_state
            .status_by_node
            .get(&node_id_key)
            .copied()
            .unwrap_or(AgentStatus::Idle);
        if !matches!(status, AgentStatus::Failed | AgentStatus::Interrupted) {
            return Err(BackendError::NodeNotRetryable(node_id.to_string()));
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::RetryNode {
                node_id: node_id_key,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        Ok(run_state.clone())
    }

    /// Stops the active workflow run cooperatively.
    ///
    /// # Errors
    ///
    /// Returns an error only if the stop signal cannot be sent on the run channel.
    pub async fn stop_run(&self) -> Result<WorkflowRunState, BackendError> {
        let should_terminate = {
            let session = self.session.lock().await;
            match (&session.handle, &session.run_state, &session.workflow) {
                (Some(_), Some(run_state), _) if run_state.active => true,
                (None, Some(run_state), _) if run_state.active => false,
                (_, Some(run_state), _) => return Ok(run_state.clone()),
                (Some(_), None, _) => true,
                (None, None, Some(workflow)) => {
                    return Ok(WorkflowRunState::idle_for_workflow(workflow));
                }
                (None, None, None) => return Err(BackendError::NoActiveRun),
            }
        };

        if should_terminate {
            if let Some(snapshot) = self.terminate_active_run(TerminationMode::UserStop).await {
                return Ok(snapshot);
            }
        }

        let session = self.session.lock().await;
        if let Some(run_state) = session.run_state.clone() {
            if !run_state.active {
                return Ok(run_state);
            }
        }
        if let Some(workflow) = session.workflow.clone() {
            return Ok(WorkflowRunState::idle_for_workflow(&workflow));
        }
        Err(BackendError::NoActiveRun)
    }

    async fn terminate_active_run(&self, mode: TerminationMode) -> Option<WorkflowRunState> {
        let (handle, action_tx, cancel_token) = {
            let mut session = self.session.lock().await;
            if session.handle.is_none() && session.cancel_token.is_none() {
                return None;
            }
            (
                session.handle.take(),
                session.action_tx.take(),
                session.cancel_token.take(),
            )
        };

        if let Some(tx) = action_tx {
            let _ = tx.send(ExecutionAction::Stop);
        }
        if let Some(token) = cancel_token {
            token.cancel();
        }

        if let Some(mut handle) = handle {
            match tokio::time::timeout(Duration::from_secs(2), &mut handle).await {
                Ok(_) => {}
                Err(_) => {
                    handle.abort();
                }
            }
        }

        if matches!(mode, TerminationMode::UserStop) {
            let mut session = self.session.lock().await;
            let captured_checkpoint = session
                .checkpoint_sink
                .as_ref()
                .and_then(|sink| sink.lock().take());
            if let Some(checkpoint) = captured_checkpoint {
                session.engine_checkpoint = Some(checkpoint);
            }
            session.checkpoint_sink = None;
            let workflow = session.workflow.clone()?;
            let run_state = session.run_state.as_mut()?;
            if run_state.active {
                apply_event_to_run_state(&workflow, run_state, ExecutionEvent::Aborted);
            }
            return Some(run_state.clone());
        }
        None
    }

    /// # Errors
    /// Returns an error if there is no active run.
    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let workflow = session.workflow.clone().ok_or(BackendError::NoActiveRun)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        apply_event_to_run_state(&workflow, run_state, event);
        let finished = !run_state.active;
        let snapshot = run_state.clone();
        if finished {
            finish_run_session(&mut session);
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
        let mut session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        let node_id_key = NodeId(node_id.to_string());
        if !run_state.awaiting_node_ids.contains(&node_id_key)
            && run_state.awaiting_node_id.as_ref() != Some(&node_id_key)
        {
            let expected = run_state
                .awaiting_node_id
                .clone()
                .or_else(|| run_state.awaiting_node_ids.first().cloned())
                .ok_or(BackendError::NoAwaitingInput)?;
            return Err(BackendError::WrongAwaitingNode {
                expected,
                received: node_id_key,
            });
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ProvideInput {
                node_id: node_id_key,
                text: text.clone(),
            })
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
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        let expected = run_state
            .pending_approvals
            .iter()
            .find(|pending| pending.approval_id == approval_id)
            .cloned()
            .ok_or_else(|| {
                if run_state.pending_approvals.is_empty() {
                    BackendError::NoPendingApproval
                } else {
                    BackendError::WrongApprovalId {
                        expected: run_state.pending_approvals[0].approval_id.clone(),
                        received: approval_id.to_string(),
                    }
                }
            })?;
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ResolveApproval {
                approval_id: approval_id.to_string(),
                allow,
                reason,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        run_state
            .pending_approvals
            .retain(|pending| pending.approval_id != expected.approval_id);
        Ok(run_state.clone())
    }

    /// Returns an error because conversational paused nodes advance via `submit_user_input`.
    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        let session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        if run_state.awaiting_node_id.is_some() || !run_state.awaiting_node_ids.is_empty() {
            return Err(BackendError::NoAwaitingInput);
        }
        Err(BackendError::NoAwaitingInput)
    }

    #[must_use]
    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.session.lock().await.run_state.clone()
    }

    /// Dry-run a write-tier tool call and return numbered diffs for approval UI.
    ///
    /// # Errors
    /// Returns an error when there is no active run or preview computation fails.
    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        _arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        let session = self.session.lock().await;
        let cwd = session
            .execution_cwd
            .clone()
            .ok_or(BackendError::NoActiveRun)?;
        let snapshot_store = session
            .snapshot_store
            .clone()
            .ok_or(BackendError::NoActiveRun)?;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        let pending = run_state
            .pending_approvals
            .iter()
            .find(|pending| pending.approval_id == approval_id)
            .ok_or_else(|| {
                if run_state.pending_approvals.is_empty() {
                    BackendError::NoPendingApproval
                } else {
                    BackendError::WrongApprovalId {
                        expected: run_state.pending_approvals[0].approval_id.clone(),
                        received: approval_id.to_string(),
                    }
                }
            })?;
        if pending.tool_call.name != tool_name {
            return Err(BackendError::PreviewFailed(
                "preview does not match the pending tool approval".to_string(),
            ));
        }
        // Use the pending approval's stored arguments — UI round-trips can change JSON shape.
        let tool_name_for_task = pending.tool_call.name.clone();
        let preview_arguments = pending.tool_call.arguments.clone();
        self.runtime_handle
            .spawn_blocking(move || {
                preview_file_edit(cwd, &tool_name_for_task, &preview_arguments, snapshot_store)
            })
            .await
            .map_err(|error| BackendError::PreviewFailed(error.to_string()))?
            .map_err(BackendError::PreviewFailed)
    }

    /// Return `git diff` for a file under the active run's execution folder.
    pub async fn git_diff_file(&self, path: String) -> Result<String, BackendError> {
        let cwd = self
            .session
            .lock()
            .await
            .execution_cwd
            .clone()
            .ok_or(BackendError::NoExecutionCwd)?;
        self.runtime_handle
            .spawn_blocking(move || crate::git::diff_file(&cwd, &path))
            .await
            .map_err(|error| BackendError::GitFailed(error.to_string()))?
            .map_err(|error| BackendError::GitFailed(error.to_string()))
    }

    /// Restore files from a recorded edit batch and update run state.
    pub async fn revert_edit_batch(
        &self,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        let (cwd, batch, pending_engine_reverts) = {
            let session = self.session.lock().await;
            let cwd = session
                .execution_cwd
                .clone()
                .ok_or(BackendError::NoExecutionCwd)?;
            let run_state = session
                .run_state
                .as_ref()
                .ok_or(BackendError::NoActiveRun)?;
            let batch = run_state
                .edit_batches
                .iter()
                .find(|batch| batch.batch_id == batch_id)
                .cloned()
                .ok_or_else(|| BackendError::EditBatchNotFound(batch_id.clone()))?;
            let pending_engine_reverts = session.pending_engine_reverts.clone();
            (cwd, batch, pending_engine_reverts)
        };

        let batch_for_revert = batch.clone();
        self.runtime_handle
            .spawn_blocking(move || {
                crate::tools::edit::batch::revert_edit_batch(&cwd, &batch_for_revert)
            })
            .await
            .map_err(|error| BackendError::GitFailed(error.to_string()))?
            .map_err(BackendError::GitFailed)?;

        let batch_node_id = batch.node_id.clone();
        if let Some(pending) = pending_engine_reverts {
            pending.lock().push(batch);
        }

        let mut session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        run_state
            .changed_files
            .retain(|record| record.batch_id.as_deref() != Some(batch_id.as_str()));
        if let Some(records) = run_state.changed_files_by_node.get_mut(&batch_node_id) {
            records.retain(|record| record.batch_id.as_deref() != Some(batch_id.as_str()));
        }
        run_state
            .edit_batches
            .retain(|entry| entry.batch_id != batch_id);
        Ok(run_state.clone())
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.session
            .lock()
            .await
            .run_state
            .as_ref()
            .is_some_and(|state| state.active)
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        let mut session = self.session.lock().await;
        let workflow = session.workflow.clone();
        let run_state = session.run_state.as_mut();
        let snapshot = match (workflow, run_state) {
            (Some(workflow), Some(run_state)) => {
                let mut cleared = WorkflowRunState::idle_for_workflow(&workflow);
                cleared.chat_logs = run_state.chat_logs.clone();
                cleared.outputs = run_state.outputs.clone();
                *run_state = cleared;
                Some(run_state.clone())
            }
            _ => None,
        };
        session.engine_checkpoint = None;
        clear_artifact_root(&mut session);
        session.checkpoint_sink = None;
        Ok(snapshot)
    }

    #[cfg(test)]
    #[must_use]
    #[allow(dead_code, reason = "used by orchestration integration tests")]
    pub(crate) fn runtime_handle(&self) -> &tokio::runtime::Handle {
        &self.runtime_handle
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "used by orchestration integration tests")]
    pub(crate) async fn test_seed_session(
        &self,
        workflow: Workflow,
        run_state: WorkflowRunState,
        action_tx: UnboundedSender<ExecutionAction>,
    ) {
        let mut session = self.session.lock().await;
        session.workflow = Some(workflow);
        session.run_state = Some(run_state);
        session.action_tx = Some(action_tx);
    }
}
