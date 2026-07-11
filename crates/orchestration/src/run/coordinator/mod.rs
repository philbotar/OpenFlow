use crate::api::FileEditPreview;
use crate::error::BackendError;
#[cfg(test)]
use crate::run::execution::NodeInterrupts;
use crate::run::execution::{
    apply_event_to_run_state, record_entrypoint_message, resolve_execution_cwd,
    should_record_entrypoint_in_chat, ExecutionAction, ExecutionEvent,
};
use crate::run::persistence::{workflow_hash, RunRecord, RunStatus, RunStoreRoot};
use crate::run::ports::RunCheckpointStore;
use crate::run::state::{AgentStatus, WorkflowRunState};
use crate::tools::edit::preview::preview_file_edit;
use chrono::Utc;
#[cfg(test)]
use engine::Workflow;
use engine::{
    apply_runtime_patch_to_agent, execution_layers, upsert_runtime_patch, validate_workflow,
    ChatMessage, ChatRole, NodeId,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
#[cfg(test)]
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use uuid::Uuid;

mod checkpoint;
mod session;

pub use session::{DurableResumeParams, RunStartParams};

use checkpoint::{load_replay_projection, persist_pending_checkpoint};
use session::{
    apply_user_stop_to_session, clear_artifact_root, finalize_run_launch, finish_run_session,
    fresh_execution_resources, prepare_workflow_run, require_action_tx, require_active_run_state,
    require_node_mut, require_run_state, require_run_state_mut, require_workflow_mut,
    ExecutionResources, RunLaunchTail, RunSession, SpawnRunInput, TerminationMode,
};

pub struct RunCoordinator {
    runtime_handle: tokio::runtime::Handle,
    session: Mutex<RunSession>,
}

#[cfg(test)]
mod tests;

impl RunCoordinator {
    #[must_use]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            runtime_handle,
            session: Mutex::new(RunSession {
                workflow: None,
                run_state: None,
                run_id: None,
                run_root: None,
                project_id: None,
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
                runtime_config_store: None,
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
            run_root,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            run_store,
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

        let prepared = prepare_workflow_run(
            workflow,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            env,
        )?;
        let workflow = prepared.workflow.clone();

        self.terminate_active_run(TerminationMode::Replaced).await;
        {
            let mut session = self.session.lock().await;
            session.engine_checkpoint = None;
        }

        let run_id = Uuid::new_v4().to_string();
        let artifact_root = run_store.run_dir(&run_root, &run_id).join("artifacts");
        let now_ms = Utc::now().timestamp_millis();
        let run_record = RunRecord {
            run_id: run_id.clone(),
            workflow_id: workflow.id.to_string(),
            workflow_name: workflow.name.clone(),
            workflow_hash: workflow_hash(&workflow),
            project_id: run_root.project_id.clone(),
            execution_cwd: resolved_cwd.display().to_string(),
            artifact_root: artifact_root.display().to_string(),
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
            status: RunStatus::Running,
        };
        run_store.create_run(&run_root, &run_record)?;

        let resources = fresh_execution_resources(&prepared.persisted_settings);
        let entrypoint_for_chat = entrypoint.clone();
        finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    entrypoint: entrypoint.clone(),
                    execution_cwd: resolved_cwd.clone(),
                    project_id: run_root.project_id.clone(),
                    artifact_root: artifact_root.clone(),
                    resume_checkpoint: None,
                },
                resources,
                entrypoint,
                execution_cwd: resolved_cwd,
                artifact_root,
            },
            |session| {
                session.run_id = Some(run_id.clone());
                session.run_root = Some(run_root);
                session.project_id = run_record.project_id.clone();
                let mut initial_state = WorkflowRunState::running_for_workflow(&workflow);
                initial_state.run_id = session.run_id.clone();
                if let Some(text) = entrypoint_for_chat.filter(|t| !t.trim().is_empty()) {
                    if let Ok(layers) = execution_layers(&workflow) {
                        if let Some(root_id) = layers.first().and_then(|layer| layer.first()) {
                            if should_record_entrypoint_in_chat(&workflow, root_id) {
                                record_entrypoint_message(&mut initial_state, &root_id.0, text);
                            }
                        }
                    }
                }
                session.run_state = Some(initial_state.clone());
                Ok(initial_state)
            },
        )
        .await
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
            project_id,
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
                session.project_id.clone(),
            )
        };
        engine::validate_checkpoint_against_workflow(&workflow, &checkpoint)
            .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

        let prepared = prepare_workflow_run(
            workflow,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            env,
        )?;
        self.terminate_active_run(TerminationMode::Replaced).await;

        let resources = ExecutionResources {
            snapshot_store,
            lsp_settings,
            pending_engine_reverts,
            node_interrupts: Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::new())),
            checkpoint_sink: Arc::new(parking_lot::Mutex::new(None)),
            runtime_config_store: engine::new_runtime_config_store(),
        };
        finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    entrypoint: entrypoint.clone(),
                    execution_cwd: execution_cwd.clone(),
                    project_id,
                    artifact_root: artifact_root.clone(),
                    resume_checkpoint: Some(checkpoint),
                },
                resources,
                entrypoint,
                execution_cwd,
                artifact_root,
            },
            |session| {
                let run_id = session.run_id.clone();
                let run_state = session
                    .run_state
                    .as_mut()
                    .ok_or(BackendError::NoContinuableRun)?;
                run_state.active = true;
                run_state.run_id = run_id;
                Ok(run_state.clone())
            },
        )
        .await
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
        let run_state = require_run_state(&session)?;
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
        let run_state = require_run_state(&session)?;
        let node_id_key = NodeId(node_id.to_string());
        let status = run_state
            .status_by_node
            .get(&node_id_key)
            .copied()
            .unwrap_or(AgentStatus::Idle);
        if !matches!(status, AgentStatus::Failed | AgentStatus::Interrupted) {
            return Err(BackendError::NodeNotRetryable(node_id.to_string()));
        }
        require_action_tx(&session)?
            .send(ExecutionAction::RetryNode {
                node_id: node_id_key,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        Ok(run_state.clone())
    }

    /// Update per-node tool approval or reasoning settings for the active run.
    ///
    /// # Errors
    /// Returns an error when there is no active run or the node is unknown.
    pub async fn update_node_runtime_config(
        &self,
        node_id: &str,
        patch: engine::NodeRuntimeConfigPatch,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let snapshot = require_active_run_state(&session)?.clone();
        let node_id_key = NodeId(node_id.to_string());
        {
            let node = require_node_mut(require_workflow_mut(&mut session)?, node_id)?;
            apply_runtime_patch_to_agent(&mut node.agent, &patch);
        }
        if let Some(store) = session.runtime_config_store.as_ref() {
            upsert_runtime_patch(store, node_id_key, patch);
        }
        Ok(snapshot)
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

        let mut session = self.session.lock().await;
        if session.run_state.as_ref().is_some_and(|state| state.active) {
            if let Some(snapshot) = apply_user_stop_to_session(&mut session) {
                return Ok(snapshot);
            }
        }
        match (session.run_state.clone(), session.workflow.clone()) {
            (Some(state), _) => Ok(state),
            (None, Some(workflow)) => Ok(WorkflowRunState::idle_for_workflow(&workflow)),
            (None, None) => Err(BackendError::NoActiveRun),
        }
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
            return apply_user_stop_to_session(&mut session);
        }
        None
    }

    /// # Errors
    /// Returns an error if there is no active run.
    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        let (snapshot, pending_persist, finished) = {
            let mut session = self.session.lock().await;
            let workflow = session.workflow.clone().ok_or(BackendError::NoActiveRun)?;
            let run_state = require_run_state_mut(&mut session)?;

            if !run_state.active {
                return Ok(run_state.clone());
            }

            apply_event_to_run_state(&workflow, run_state, event);
            let finished = !run_state.active;
            let snapshot = run_state.clone();
            let pending_checkpoint = session
                .checkpoint_sink
                .as_ref()
                .and_then(|sink| sink.lock().take());
            let pending_persist = match (
                session.run_root.clone(),
                session.run_id.clone(),
                pending_checkpoint,
            ) {
                (Some(root), Some(run_id), Some(pending)) => Some((root, run_id, pending)),
                _ => None,
            };
            (snapshot, pending_persist, finished)
        };

        if let Some((root, run_id, pending)) = pending_persist {
            persist_pending_checkpoint(run_store, &root, &run_id, &snapshot, pending)?;
        }
        if finished {
            let mut session = self.session.lock().await;
            finish_run_session(&mut session);
        }
        Ok(snapshot)
    }

    pub fn list_runs(
        &self,
        run_store: &dyn RunCheckpointStore,
        roots: &[RunStoreRoot],
        workflow_id: Option<&str>,
    ) -> Result<Vec<crate::run::persistence::RunSummary>, BackendError> {
        Ok(run_store.list_runs(roots, workflow_id)?)
    }

    pub fn replay_run(
        &self,
        run_store: &dyn RunCheckpointStore,
        roots: &[RunStoreRoot],
        run_id: &str,
    ) -> Result<WorkflowRunState, BackendError> {
        load_replay_projection(run_store, roots, run_id)
    }

    /// # Errors
    /// Returns an error when the workflow changed or provider configuration fails.
    pub async fn resume_durable_run(
        &self,
        params: DurableResumeParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        if workflow_hash(&params.workflow) != params.record.workflow_hash {
            return Err(BackendError::RunWorkflowChanged(
                params.run_id.to_string(),
                params.workflow.id.to_string(),
            ));
        }
        engine::validate_checkpoint_against_workflow(&params.workflow, &params.checkpoint.engine)
            .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

        let prepared = prepare_workflow_run(
            params.workflow,
            params.settings,
            params.transient_api_key,
            params.agent_store,
            params.settings_store,
            params.env,
        )?;
        let engine_checkpoint = params.checkpoint.engine;

        self.terminate_active_run(TerminationMode::Replaced).await;

        let resources = fresh_execution_resources(&prepared.persisted_settings);
        let artifact_root = PathBuf::from(&params.record.artifact_root);
        let execution_cwd = PathBuf::from(&params.record.execution_cwd);
        let run_root = params.root.clone();
        let run_id = params.run_id.to_string();
        let project_id = params.record.project_id.clone();
        let mut resumed_state = params.checkpoint.projection;
        resumed_state.active = true;
        resumed_state.run_id = Some(run_id.clone());

        let (state, event_rx) = finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    entrypoint: None,
                    execution_cwd: execution_cwd.clone(),
                    project_id: project_id.clone(),
                    artifact_root: artifact_root.clone(),
                    resume_checkpoint: Some(engine_checkpoint),
                },
                resources,
                entrypoint: None,
                execution_cwd,
                artifact_root,
            },
            |session| {
                session.run_state = Some(resumed_state.clone());
                session.run_id = Some(run_id.clone());
                session.run_root = Some(run_root.clone());
                session.project_id = project_id;
                Ok(resumed_state.clone())
            },
        )
        .await?;
        params.run_store.update_status(
            &run_root,
            &run_id,
            RunStatus::Running,
            Utc::now().timestamp_millis(),
        )?;
        Ok((state, event_rx))
    }

    /// # Errors
    /// Returns an error if there is no active run or the wrong node is selected.
    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let run_state = require_run_state(&session)?;
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
        require_action_tx(&session)?
            .send(ExecutionAction::ProvideInput {
                node_id: node_id_key.clone(),
                text: text.clone(),
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        // ponytail: chat-only optimistic append; awaiting/status follow execution events
        let run_state = require_run_state_mut(&mut session)?;
        run_state
            .chat_logs
            .entry(node_id_key)
            .or_default()
            .push(ChatMessage::text(ChatRole::User, text));
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
        let session = self.session.lock().await;
        let run_state = require_run_state(&session)?;
        if !run_state
            .pending_approvals
            .iter()
            .any(|pending| pending.approval_id == approval_id)
        {
            return Err(if run_state.pending_approvals.is_empty() {
                BackendError::NoPendingApproval
            } else {
                BackendError::WrongApprovalId {
                    expected: run_state.pending_approvals[0].approval_id.clone(),
                    received: approval_id.to_string(),
                }
            });
        }
        require_action_tx(&session)?
            .send(ExecutionAction::ResolveApproval {
                approval_id: approval_id.to_string(),
                allow,
                reason,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        Ok(run_state.clone())
    }

    #[must_use]
    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.session.lock().await.run_state.clone()
    }

    #[must_use]
    pub async fn current_run_id(&self) -> Option<String> {
        self.session.lock().await.run_id.clone()
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
        let run_state = require_run_state(&session)?;
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
            let run_state = require_run_state(&session)?;
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
        let run_state = require_run_state_mut(&mut session)?;
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
    #[allow(dead_code, reason = "coordinator tests seed varied session shapes")]
    pub(crate) async fn test_seed_full(&self, seed: TestSessionSeed) {
        let mut session = self.session.lock().await;
        session.run_id = seed.run_id.or_else(|| seed.run_state.run_id.clone());
        session.run_root = seed.run_root;
        session.project_id = seed.project_id;
        session.workflow = Some(seed.workflow);
        session.run_state = Some(seed.run_state);
        session.action_tx = seed.action_tx;
        session.execution_cwd = seed.execution_cwd;
        session.entrypoint = seed.entrypoint;
        session.artifact_root = seed.artifact_root;
        session.engine_checkpoint = seed.engine_checkpoint;
        session.checkpoint_sink = seed.checkpoint_sink;
        session.snapshot_store = seed.snapshot_store;
        session.lsp_settings = seed.lsp_settings;
        session.pending_engine_reverts = seed.pending_engine_reverts;
        session.node_interrupts = seed.node_interrupts;
        session.runtime_config_store = seed.runtime_config_store;
        session.cancel_token = seed.cancel_token;
        session.handle = seed.handle;
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "used by orchestration integration tests")]
    pub(crate) async fn test_seed_session(
        &self,
        workflow: Workflow,
        run_state: WorkflowRunState,
        action_tx: UnboundedSender<ExecutionAction>,
    ) {
        self.test_seed_full(TestSessionSeed {
            workflow,
            run_state,
            action_tx: Some(action_tx),
            run_id: None,
            run_root: None,
            project_id: None,
            execution_cwd: None,
            entrypoint: None,
            artifact_root: None,
            engine_checkpoint: None,
            checkpoint_sink: None,
            snapshot_store: None,
            lsp_settings: None,
            pending_engine_reverts: None,
            node_interrupts: None,
            runtime_config_store: None,
            cancel_token: None,
            handle: None,
        })
        .await;
    }
}

#[cfg(test)]
pub(crate) struct TestSessionSeed {
    pub workflow: Workflow,
    pub run_state: WorkflowRunState,
    pub action_tx: Option<UnboundedSender<ExecutionAction>>,
    pub run_id: Option<String>,
    pub run_root: Option<RunStoreRoot>,
    pub project_id: Option<String>,
    pub execution_cwd: Option<PathBuf>,
    pub entrypoint: Option<String>,
    pub artifact_root: Option<PathBuf>,
    pub engine_checkpoint: Option<engine::InteractiveEngineCheckpoint>,
    pub checkpoint_sink:
        Option<Arc<parking_lot::Mutex<Option<crate::run::persistence::PendingRunCheckpoint>>>>,
    pub snapshot_store: Option<Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>>,
    pub lsp_settings: Option<crate::lsp::LspSettings>,
    pub pending_engine_reverts: Option<Arc<parking_lot::Mutex<Vec<engine::EditBatch>>>>,
    pub node_interrupts: Option<NodeInterrupts>,
    pub runtime_config_store: Option<engine::NodeRuntimeConfigStore>,
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    pub handle: Option<tokio::task::JoinHandle<()>>,
}
