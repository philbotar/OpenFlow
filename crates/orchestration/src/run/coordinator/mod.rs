use crate::api::FileEditPreview;
use crate::error::BackendError;
#[cfg(test)]
use crate::run::execution::NodeInterrupts;
use crate::run::execution::{
    apply_event_to_run_state, initial_engine_checkpoint,
    record_entrypoint_message_with_attachments, resolve_execution_cwd, ExecutionAction,
    ExecutionEvent, ResumeContinuation,
};
use crate::run::persistence::{
    run_name, workflow_hash, PendingRunCheckpoint, RunCheckpointReason, RunRecord, RunStatus,
    RunStoreRoot,
};
use crate::run::ports::{RunAttachmentStore, RunCheckpointStore};
use crate::run::resources::SharedRunResources;
use crate::run::skill_invocation::skill_prompt_for_ids;
use crate::run::state::{AgentStatus, WorkflowRunState};
#[cfg(test)]
use crate::settings::model::AppSettings;
use crate::tools::edit::preview::preview_file_edit;
use chrono::Utc;
#[cfg(test)]
use engine::Workflow;
use engine::{
    apply_runtime_patch_to_agent, execution_layers, upsert_runtime_patch, validate_workflow, NodeId,
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

use checkpoint::{
    load_replay_projection, persist_pending_checkpoint, projection_ready_for_checkpoint,
};
use session::{
    apply_user_stop_to_session, clear_artifact_root, finalize_run_launch, finish_run_session,
    fresh_execution_resources, prepare_workflow_run, require_action_tx, require_active_run_state,
    require_node_mut, require_run_state, require_run_state_mut, require_workflow_mut,
    resolve_mcp_context_for_run, ExecutionResources, RunControl, RunLaunchMetadata, RunLaunchTail,
    RunSession, SpawnRunInput, TerminationMode,
};

pub struct RunCoordinator {
    runtime_handle: tokio::runtime::Handle,
    attachment_store: Arc<dyn RunAttachmentStore>,
    shared_resources: Arc<SharedRunResources>,
    session: Mutex<RunSession>,
}

#[cfg(test)]
mod tests;

impl RunCoordinator {
    #[must_use]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self::with_attachment_store(
            runtime_handle,
            Arc::new(
                crate::adapters::storage::run_attachment_store::FileRunAttachmentStore::default(),
            ),
        )
    }

    #[must_use]
    pub fn with_attachment_store(
        runtime_handle: tokio::runtime::Handle,
        attachment_store: Arc<dyn RunAttachmentStore>,
    ) -> Self {
        Self::with_shared_resources(
            runtime_handle,
            attachment_store,
            Arc::new(SharedRunResources::default()),
        )
    }

    #[must_use]
    pub(crate) fn with_shared_resources(
        runtime_handle: tokio::runtime::Handle,
        attachment_store: Arc<dyn RunAttachmentStore>,
        shared_resources: Arc<SharedRunResources>,
    ) -> Self {
        Self {
            runtime_handle,
            attachment_store,
            shared_resources,
            session: Mutex::new(RunSession {
                workflow: None,
                run_state: None,
                run_id: None,
                run_root: None,
                project_id: None,
                skill_paths: Default::default(),
                execution_cwd: None,
                entrypoint: None,
                entrypoint_attachments: Vec::new(),
                artifact_root: None,
                attachment_root: None,
                generation: 0,
                engine_checkpoint: None,
                active: None,
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
            invoked_skill_ids,
            entrypoint,
            execution_cwd,
            run_root,
            settings,
            transient_api_key,
            agent_store,
            skill_catalog,
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

        let mut prepared = prepare_workflow_run(
            workflow,
            &invoked_skill_ids,
            settings,
            transient_api_key,
            agent_store,
            skill_catalog,
            settings_store,
            env,
            Arc::clone(&self.shared_resources),
        )?;
        let mcp_project_root = run_root.project_id.as_ref().map(|_| resolved_cwd.as_path());
        resolve_mcp_context_for_run(&mut prepared, &resolved_cwd, mcp_project_root).await;
        validate_workflow(&prepared.workflow)?;
        let workflow = prepared.workflow.clone();
        let mutation_gate = self
            .shared_resources
            .mutation_gate_for_workflow(&workflow, &resolved_cwd);
        let run_entrypoint = entrypoint.filter(|message| !message.is_empty());
        let entrypoint_text = run_entrypoint
            .as_ref()
            .map(|message| message.text.clone())
            .filter(|text| !text.trim().is_empty());

        self.terminate_active_run(TerminationMode::Replaced).await;
        {
            let mut session = self.session.lock().await;
            session.engine_checkpoint = None;
        }

        let run_id = Uuid::new_v4().to_string();
        let run_dir = run_store.run_dir(&run_root, &run_id);
        let artifact_root = run_dir.join("artifacts");
        let attachment_root = run_dir.join("attachments");
        let now_ms = Utc::now().timestamp_millis();
        let run_record = RunRecord {
            run_id: run_id.clone(),
            name: Some(run_name(&workflow.name, entrypoint_text.as_deref())),
            workflow_id: workflow.id.to_string(),
            workflow_name: workflow.name.clone(),
            workflow_hash: workflow_hash(&workflow),
            workflow_snapshot: workflow.clone(),
            project_id: run_root.project_id.clone(),
            execution_cwd: resolved_cwd.display().to_string(),
            artifact_root: artifact_root.display().to_string(),
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
            status: RunStatus::Running,
        };
        run_store.create_run(&run_root, &run_record)?;
        let attachment_source_paths = run_entrypoint
            .as_ref()
            .map(|message| {
                message
                    .attachment_source_paths
                    .iter()
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let attachment_store = Arc::clone(&self.attachment_store);
        let attachment_root_for_ingest = attachment_root.clone();
        let entrypoint_attachments = match self
            .runtime_handle
            .spawn_blocking(move || {
                attachment_store.ingest_batch(&attachment_root_for_ingest, &attachment_source_paths)
            })
            .await
        {
            Ok(Ok(attachments)) => attachments,
            Ok(Err(error)) => {
                let _ = run_store.remove_run(&run_root, &run_id);
                return Err(error.into());
            }
            Err(error) => {
                let _ = run_store.remove_run(&run_root, &run_id);
                return Err(BackendError::PreviewFailed(error.to_string()));
            }
        };

        let mut initial_state = WorkflowRunState::running_for_workflow(&workflow);
        initial_state.run_id = Some(run_id.clone());
        initial_state.execution_cwd = Some(resolved_cwd.display().to_string());
        initial_state.project_id = run_root.project_id.clone();
        initial_state.waiting_reason = mutation_gate
            .as_ref()
            .is_some_and(|gate| gate.available_permits() == 0)
            .then(|| "Waiting for another run using this workspace".to_string());
        if entrypoint_text.is_some() || !entrypoint_attachments.is_empty() {
            if let Some(root_id) = execution_layers(&workflow)
                .ok()
                .and_then(|layers| layers.first().and_then(|layer| layer.first()).cloned())
            {
                record_entrypoint_message_with_attachments(
                    &mut initial_state,
                    &root_id.0,
                    entrypoint_text.clone().unwrap_or_default(),
                    entrypoint_attachments.clone(),
                );
            }
        }
        let project_repository_root = run_root
            .project_id
            .as_ref()
            .map(|_| resolved_cwd.display().to_string());
        let initial_checkpoint = match initial_engine_checkpoint(
            workflow.clone(),
            entrypoint_text.clone(),
            entrypoint_attachments.clone(),
            project_repository_root,
        ) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                let _ = run_store.remove_run(&run_root, &run_id);
                return Err(error.into());
            }
        };
        if let Err(error) = persist_pending_checkpoint(
            run_store,
            &run_root,
            &run_id,
            &initial_state,
            PendingRunCheckpoint {
                reason: RunCheckpointReason::Started,
                engine: initial_checkpoint,
            },
        ) {
            let _ = run_store.remove_run(&run_root, &run_id);
            return Err(error);
        }

        let resources = fresh_execution_resources(&prepared.persisted_settings);
        let initial_state_for_session = initial_state.clone();
        finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    metadata: RunLaunchMetadata {
                        entrypoint: entrypoint_text,
                        entrypoint_attachments,
                        execution_cwd: resolved_cwd,
                        artifact_root,
                        attachment_root,
                    },
                    project_id: run_root.project_id.clone(),
                    attachment_store: Arc::clone(&self.attachment_store),
                    resume_checkpoint: None,
                    resume_continuation: None,
                    shared_resources: Arc::clone(&self.shared_resources),
                    mutation_gate,
                },
                resources,
            },
            |session| {
                session.run_id = Some(run_id.clone());
                session.run_root = Some(run_root);
                session.project_id = run_record.project_id.clone();
                session.run_state = Some(initial_state_for_session.clone());
                Ok(initial_state_for_session)
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
            mut workflow,
            invoked_skill_ids,
            entrypoint,
            settings,
            transient_api_key,
            agent_store,
            skill_catalog,
            settings_store,
            env,
            ..
        } = params;

        validate_workflow(&workflow)?;
        let (
            checkpoint,
            artifact_root,
            attachment_root,
            execution_cwd,
            snapshot_store,
            lsp_settings,
            pending_engine_reverts,
            project_id,
            frozen_workflow,
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
                    .attachment_root
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .execution_cwd
                    .clone()
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .active
                    .as_ref()
                    .map(|active| Arc::clone(&active.resources.snapshot_store))
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .active
                    .as_ref()
                    .map(|active| active.resources.lsp_settings.clone())
                    .ok_or(BackendError::NoContinuableRun)?,
                session
                    .active
                    .as_ref()
                    .map(|active| Arc::clone(&active.resources.pending_engine_reverts))
                    .ok_or(BackendError::NoContinuableRun)?,
                session.project_id.clone(),
                session.workflow.clone(),
            )
        };
        if let Some(frozen_workflow) = frozen_workflow {
            for node in &mut workflow.nodes {
                if let Some(frozen_node) = frozen_workflow
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == node.id)
                {
                    node.agent
                        .mcp_resources
                        .clone_from(&frozen_node.agent.mcp_resources);
                    node.agent
                        .mcp_prompts
                        .clone_from(&frozen_node.agent.mcp_prompts);
                    node.agent
                        .mcp_context_snapshots
                        .clone_from(&frozen_node.agent.mcp_context_snapshots);
                }
            }
        }
        engine::validate_checkpoint_against_workflow(&workflow, &checkpoint)
            .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

        let prepared = prepare_workflow_run(
            workflow,
            &invoked_skill_ids,
            settings,
            transient_api_key,
            agent_store,
            skill_catalog,
            settings_store,
            env,
            Arc::clone(&self.shared_resources),
        )?;
        let mutation_gate = self
            .shared_resources
            .mutation_gate_for_workflow(&prepared.workflow, &execution_cwd);
        let continued_workflow_id = prepared.workflow.id.to_string();
        self.terminate_active_run(TerminationMode::Replaced).await;

        let resources = ExecutionResources {
            snapshot_store,
            lsp_settings,
            pending_engine_reverts,
            node_interrupts: Arc::new(parking_lot::Mutex::new(std::collections::BTreeMap::new())),
            checkpoint_sink: Arc::new(parking_lot::Mutex::new(None)),
            runtime_config_store: engine::new_runtime_config_store(),
        };
        let entrypoint_text = entrypoint
            .map(|message| message.text)
            .filter(|text| !text.trim().is_empty());
        let entrypoint_attachments = Vec::new();
        let continued_execution_cwd = execution_cwd.display().to_string();
        let continued_project_id = project_id.clone();
        finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    metadata: RunLaunchMetadata {
                        entrypoint: entrypoint_text,
                        entrypoint_attachments,
                        execution_cwd,
                        artifact_root,
                        attachment_root,
                    },
                    project_id: project_id.clone(),
                    attachment_store: Arc::clone(&self.attachment_store),
                    resume_checkpoint: Some(checkpoint),
                    resume_continuation: None,
                    shared_resources: Arc::clone(&self.shared_resources),
                    mutation_gate: mutation_gate.clone(),
                },
                resources,
            },
            |session| {
                let run_id = session.run_id.clone();
                let run_state = session
                    .run_state
                    .as_mut()
                    .ok_or(BackendError::NoContinuableRun)?;
                run_state.active = true;
                run_state.workflow_id = Some(continued_workflow_id);
                run_state.run_id = run_id;
                run_state.execution_cwd = Some(continued_execution_cwd);
                run_state.project_id = continued_project_id;
                run_state.waiting_reason = mutation_gate
                    .as_ref()
                    .is_some_and(|gate| gate.available_permits() == 0)
                    .then(|| "Waiting for another run using this workspace".to_string());
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
        if let Some(interrupts) = session
            .active
            .as_ref()
            .map(|active| &active.resources.node_interrupts)
        {
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
        if let Some(store) = session
            .active
            .as_ref()
            .map(|active| &active.resources.runtime_config_store)
        {
            upsert_runtime_patch(store, node_id_key, &patch);
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
            match (
                session
                    .active
                    .as_ref()
                    .and_then(|active| active.control.as_ref()),
                &session.run_state,
                &session.workflow,
            ) {
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

    /// Stops the active run and durably records its resumable user-stop checkpoint.
    ///
    /// # Errors
    /// Returns an error when stopping or checkpoint persistence fails.
    pub async fn stop_run_and_persist(
        &self,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        let snapshot = self.stop_run().await?;
        let pending_persist = {
            let mut session = self.session.lock().await;
            let pending = session
                .active
                .as_ref()
                .and_then(|active| active.resources.checkpoint_sink.lock().take());
            let engine = pending
                .map(|checkpoint| checkpoint.engine)
                .or_else(|| session.engine_checkpoint.clone());
            if let Some(engine) = engine.as_ref() {
                session.engine_checkpoint = Some(engine.clone());
            }
            match (session.run_root.clone(), session.run_id.clone(), engine) {
                (Some(root), Some(run_id), Some(engine)) => Some((
                    root,
                    run_id,
                    PendingRunCheckpoint {
                        reason: RunCheckpointReason::UserStopped,
                        engine,
                    },
                )),
                _ => None,
            }
        };
        if let Some((root, run_id, pending)) = pending_persist {
            persist_pending_checkpoint(run_store, &root, &run_id, &snapshot, pending)?;
        }
        Ok(snapshot)
    }

    async fn terminate_active_run(&self, mode: TerminationMode) -> Option<WorkflowRunState> {
        let control = {
            let mut session = self.session.lock().await;
            let active = session.active.as_mut()?;
            let control = active.control.take()?;
            session.generation = session.generation.wrapping_add(1);
            control
        };
        let RunControl {
            handle,
            action_tx,
            cancel_token,
        } = control;

        let _ = action_tx.send(ExecutionAction::Stop);
        cancel_token.cancel();

        let mut handle = handle;
        match tokio::time::timeout(Duration::from_secs(2), &mut handle).await {
            Ok(_) => {}
            Err(_) => {
                handle.abort();
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
        let (snapshot, pending_persist, stopped_checkpoint, finished) = {
            let mut session = self.session.lock().await;
            let workflow = session.workflow.clone().ok_or(BackendError::NoActiveRun)?;
            let run_state = require_run_state_mut(&mut session)?;

            if !run_state.active {
                return Ok(run_state.clone());
            }

            apply_event_to_run_state(&workflow, run_state, event);
            let finished = !run_state.active;
            let snapshot = run_state.clone();
            let pending_checkpoint = session.active.as_ref().and_then(|active| {
                let sink = &active.resources.checkpoint_sink;
                let mut pending = sink.lock();
                pending
                    .as_ref()
                    .is_some_and(|checkpoint| {
                        projection_ready_for_checkpoint(checkpoint, &snapshot)
                    })
                    .then(|| pending.take())
                    .flatten()
            });
            let stopped_checkpoint = pending_checkpoint
                .as_ref()
                .filter(|pending| pending.reason == RunCheckpointReason::UserStopped)
                .map(|pending| pending.engine.clone());
            if let Some(checkpoint) = stopped_checkpoint.as_ref() {
                session.engine_checkpoint = Some(checkpoint.clone());
            }
            let pending_persist = match (
                session.run_root.clone(),
                session.run_id.clone(),
                pending_checkpoint,
            ) {
                (Some(root), Some(run_id), Some(pending)) => Some((root, run_id, pending)),
                _ => None,
            };
            (snapshot, pending_persist, stopped_checkpoint, finished)
        };

        if let Some((root, run_id, pending)) = pending_persist {
            persist_pending_checkpoint(run_store, &root, &run_id, &snapshot, pending)?;
        }
        if finished {
            let mut session = self.session.lock().await;
            finish_run_session(&mut session);
            if let Some(checkpoint) = stopped_checkpoint {
                session.engine_checkpoint = Some(checkpoint);
            }
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
        self.resume_durable_run_with_continuation(params, None)
            .await
    }

    /// # Errors
    /// Returns an error when the checkpoint cannot accept the continuation or provider
    /// configuration fails.
    pub async fn resume_durable_run_with_continuation(
        &self,
        params: DurableResumeParams<'_>,
        continuation: Option<crate::api::DurableRunContinuationInput>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let workflow = params.record.workflow_snapshot.clone();
        if workflow_hash(&workflow) != params.record.workflow_hash {
            return Err(BackendError::RunWorkflowChanged(
                params.run_id.to_string(),
                workflow.id.to_string(),
            ));
        }
        engine::validate_checkpoint_against_workflow(&workflow, &params.checkpoint.engine)
            .map_err(|error| BackendError::CheckpointIncompatible(error.to_string()))?;

        let prepared = prepare_workflow_run(
            workflow,
            &[],
            params.settings,
            params.transient_api_key,
            params.agent_store,
            params.skill_catalog,
            params.settings_store,
            params.env,
            Arc::clone(&self.shared_resources),
        )?;
        let attachment_root = params
            .run_store
            .run_dir(&params.root, params.run_id)
            .join("attachments");
        let (mut resume_continuation, attachment_source_paths) = continuation
            .map(|continuation| {
                let node_id = NodeId(continuation.node_id);
                let checkpoint = &params.checkpoint.engine;
                if !checkpoint.awaiting_nodes.contains(&node_id)
                    && !checkpoint.interrupted_nodes.contains(&node_id)
                    && !checkpoint.failed_nodes.contains_key(&node_id)
                {
                    return Err(BackendError::NodeNotRetryable(node_id.to_string()));
                }
                let skill_prompt = skill_prompt_for_ids(
                    &continuation.invoked_skill_ids,
                    &prepared.skill_paths,
                    &format!("saved run continuation for node {node_id:?}"),
                )?;
                Ok((
                    ResumeContinuation {
                        node_id,
                        text: continuation.text,
                        attachments: Vec::new(),
                        skill_prompt,
                    },
                    continuation
                        .attachment_source_paths
                        .into_iter()
                        .map(PathBuf::from)
                        .collect::<Vec<_>>(),
                ))
            })
            .transpose()?
            .map_or((None, Vec::new()), |(continuation, source_paths)| {
                (Some(continuation), source_paths)
            });
        let attachments = if attachment_source_paths.is_empty() {
            Vec::new()
        } else {
            let attachment_store = Arc::clone(&self.attachment_store);
            let attachment_root_for_ingest = attachment_root.clone();
            self.runtime_handle
                .spawn_blocking(move || {
                    attachment_store
                        .ingest_batch(&attachment_root_for_ingest, &attachment_source_paths)
                })
                .await
                .map_err(|error| BackendError::PreviewFailed(error.to_string()))??
        };
        if let Some(continuation) = resume_continuation.as_mut() {
            continuation.attachments.clone_from(&attachments);
        }
        let engine_checkpoint = params.checkpoint.engine;

        self.terminate_active_run(TerminationMode::Replaced).await;

        let resources = fresh_execution_resources(&prepared.persisted_settings);
        let artifact_root = PathBuf::from(&params.record.artifact_root);
        let execution_cwd = PathBuf::from(&params.record.execution_cwd);
        let mutation_gate = self
            .shared_resources
            .mutation_gate_for_workflow(&prepared.workflow, &execution_cwd);
        let resumed_workflow_id = prepared.workflow.id.to_string();
        let run_root = params.root.clone();
        let run_id = params.run_id.to_string();
        let project_id = params.record.project_id.clone();
        let mut resumed_state = params.checkpoint.projection;
        resumed_state.active = true;
        resumed_state.workflow_id = Some(resumed_workflow_id);
        resumed_state.run_id = Some(run_id.clone());
        resumed_state.execution_cwd = Some(execution_cwd.display().to_string());
        resumed_state.project_id.clone_from(&project_id);
        resumed_state.waiting_reason = mutation_gate
            .as_ref()
            .is_some_and(|gate| gate.available_permits() == 0)
            .then(|| "Waiting for another run using this workspace".to_string());
        if let Some(continuation) = &resume_continuation {
            crate::run::execution::record_user_input_with_attachments(
                &mut resumed_state,
                &continuation.node_id.0,
                continuation.text.clone(),
                continuation.attachments.clone(),
            );
        }

        let launch_result = finalize_run_launch(
            &self.runtime_handle,
            &self.session,
            prepared,
            RunLaunchTail {
                spawn_input: SpawnRunInput {
                    metadata: RunLaunchMetadata {
                        entrypoint: None,
                        entrypoint_attachments: Vec::new(),
                        execution_cwd: execution_cwd.clone(),
                        artifact_root,
                        attachment_root: attachment_root.clone(),
                    },
                    project_id: project_id.clone(),
                    attachment_store: Arc::clone(&self.attachment_store),
                    resume_checkpoint: Some(engine_checkpoint),
                    resume_continuation,
                    shared_resources: Arc::clone(&self.shared_resources),
                    mutation_gate,
                },
                resources,
            },
            |session| {
                session.run_state = Some(resumed_state.clone());
                session.run_id = Some(run_id.clone());
                session.run_root = Some(run_root.clone());
                session.project_id = project_id;
                Ok(resumed_state.clone())
            },
        )
        .await;
        let (state, event_rx) = match launch_result {
            Ok(launched) => launched,
            Err(error) => {
                remove_ingested_attachments(
                    &self.runtime_handle,
                    Arc::clone(&self.attachment_store),
                    Some(attachment_root),
                    attachments,
                )
                .await;
                return Err(error);
            }
        };
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
        self.submit_user_input_with_skill_ids(node_id, text, &[])
            .await
    }

    pub async fn submit_user_input_with_skill_ids(
        &self,
        node_id: &str,
        text: String,
        skill_ids: &[String],
    ) -> Result<WorkflowRunState, BackendError> {
        self.submit_user_message_with_skill_ids(
            node_id,
            crate::api::UserMessageInput::text(text),
            skill_ids,
        )
        .await
    }

    pub async fn submit_user_message_with_skill_ids(
        &self,
        node_id: &str,
        message: crate::api::UserMessageInput,
        skill_ids: &[String],
    ) -> Result<WorkflowRunState, BackendError> {
        let node_id_key = NodeId(node_id.to_string());
        let (run_id, attachment_root, generation, action_tx, skill_prompt) = {
            let session = self.session.lock().await;
            validate_chat_target(&session, &node_id_key)?;
            let skill_prompt = skill_prompt_for_ids(
                skill_ids,
                &session.skill_paths,
                &format!("chat input for node {:?}", node_id_key),
            )?;
            (
                session.run_id.clone(),
                session.attachment_root.clone(),
                session.generation,
                require_action_tx(&session)?.clone(),
                skill_prompt,
            )
        };

        let source_paths = message
            .attachment_source_paths
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let attachments = if source_paths.is_empty() {
            Vec::new()
        } else {
            let attachment_root_for_ingest =
                attachment_root.clone().ok_or(BackendError::NoActiveRun)?;
            let attachment_store = Arc::clone(&self.attachment_store);
            self.runtime_handle
                .spawn_blocking(move || {
                    attachment_store.ingest_batch(&attachment_root_for_ingest, &source_paths)
                })
                .await
                .map_err(|error| BackendError::PreviewFailed(error.to_string()))??
        };

        let mut session = self.session.lock().await;
        let still_current = session.generation == generation
            && session.run_id == run_id
            && session
                .active
                .as_ref()
                .and_then(|active| active.control.as_ref())
                .map(|control| &control.action_tx)
                .is_some_and(|current| current.same_channel(&action_tx));
        let validation = if still_current {
            validate_chat_target(&session, &node_id_key)
        } else {
            Err(BackendError::NoActiveRun)
        };
        if let Err(error) = validation {
            drop(session);
            remove_ingested_attachments(
                &self.runtime_handle,
                Arc::clone(&self.attachment_store),
                attachment_root.clone(),
                attachments,
            )
            .await;
            return Err(error);
        }
        if action_tx
            .send(ExecutionAction::ProvideInput {
                node_id: node_id_key.clone(),
                text: message.text.clone(),
                attachments: attachments.clone(),
                skill_prompt,
            })
            .is_err()
        {
            drop(session);
            remove_ingested_attachments(
                &self.runtime_handle,
                Arc::clone(&self.attachment_store),
                attachment_root,
                attachments,
            )
            .await;
            return Err(BackendError::RunChannelClosed);
        }
        // The accepted request is no longer actionable. Project it immediately
        // so duplicate sends cannot race the drive loop.
        let run_state = require_run_state_mut(&mut session)?;
        run_state.structured_input_by_node.remove(&node_id_key);
        crate::run::execution::record_user_input_with_attachments(
            run_state,
            node_id,
            message.text,
            attachments,
        );
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

    /// # Errors
    /// Returns an error if no matching server-to-client MCP req is pending.
    pub async fn resolve_mcp_client_request(
        &self,
        request_id: &str,
        decision: crate::mcp::client_capabilities::McpClientRequestDecision,
    ) -> Result<WorkflowRunState, BackendError> {
        let session = self.session.lock().await;
        let run_state = require_run_state(&session)?;
        if !run_state
            .pending_mcp_client_requests
            .iter()
            .any(|pending| pending.request_id == request_id)
        {
            return Err(if run_state.pending_mcp_client_requests.is_empty() {
                BackendError::NoPendingMcpClientRequest
            } else {
                BackendError::WrongMcpClientRequestId {
                    expected: run_state.pending_mcp_client_requests[0].request_id.clone(),
                    received: request_id.to_string(),
                }
            });
        }
        require_action_tx(&session)?
            .send(ExecutionAction::ResolveMcpClientRequest {
                request_id: request_id.to_string(),
                decision,
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
            .active
            .as_ref()
            .map(|active| Arc::clone(&active.resources.snapshot_store))
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
            let pending_engine_reverts = session
                .active
                .as_ref()
                .map(|active| Arc::clone(&active.resources.pending_engine_reverts));
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
        if session.run_state.as_ref().is_some_and(|state| state.active)
            || session
                .active
                .as_ref()
                .is_some_and(|active| active.control.is_some())
        {
            return Err(BackendError::ActiveRun);
        }
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
        if let Some(active) = session.active.as_mut() {
            active.resources.checkpoint_sink = Arc::new(parking_lot::Mutex::new(None));
        }
        Ok(snapshot)
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "coordinator tests seed varied session shapes")]
    pub(crate) async fn test_seed_full(&self, seed: TestSessionSeed) {
        let mut session = self.session.lock().await;
        let has_resources = seed.checkpoint_sink.is_some()
            || seed.snapshot_store.is_some()
            || seed.lsp_settings.is_some()
            || seed.pending_engine_reverts.is_some()
            || seed.node_interrupts.is_some()
            || seed.runtime_config_store.is_some();
        let has_control =
            seed.action_tx.is_some() || seed.handle.is_some() || seed.cancel_token.is_some();
        let active = if has_resources || has_control {
            let mut resources = fresh_execution_resources(&AppSettings::default());
            if let Some(checkpoint_sink) = seed.checkpoint_sink {
                resources.checkpoint_sink = checkpoint_sink;
            }
            if let Some(snapshot_store) = seed.snapshot_store {
                resources.snapshot_store = snapshot_store;
            }
            if let Some(lsp_settings) = seed.lsp_settings {
                resources.lsp_settings = lsp_settings;
            }
            if let Some(pending_engine_reverts) = seed.pending_engine_reverts {
                resources.pending_engine_reverts = pending_engine_reverts;
            }
            if let Some(node_interrupts) = seed.node_interrupts {
                resources.node_interrupts = node_interrupts;
            }
            if let Some(runtime_config_store) = seed.runtime_config_store {
                resources.runtime_config_store = runtime_config_store;
            }
            let control = if has_control {
                let action_tx = seed.action_tx.unwrap_or_else(|| {
                    let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
                    action_tx
                });
                Some(RunControl {
                    action_tx,
                    handle: seed.handle.unwrap_or_else(|| tokio::spawn(async {})),
                    cancel_token: seed.cancel_token.unwrap_or_default(),
                })
            } else {
                None
            };
            Some(session::ActiveRunResources { resources, control })
        } else {
            None
        };
        let attachment_root = seed
            .artifact_root
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(|run_dir| run_dir.join("attachments"));
        session.run_id = seed.run_id.or_else(|| seed.run_state.run_id.clone());
        session.run_root = seed.run_root;
        session.project_id = seed.project_id;
        session.workflow = Some(seed.workflow);
        session.run_state = Some(seed.run_state);
        session.execution_cwd = seed.execution_cwd;
        session.entrypoint = seed.entrypoint;
        session.artifact_root = seed.artifact_root;
        session.attachment_root = attachment_root;
        session.engine_checkpoint = seed.engine_checkpoint;
        session.active = active;
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

fn validate_chat_target(session: &RunSession, node_id: &NodeId) -> Result<(), BackendError> {
    let run_state = require_run_state(session)?;
    if run_state.awaiting_node_ids.contains(node_id)
        || run_state.awaiting_node_id.as_ref() == Some(node_id)
        || matches!(
            run_state.status_by_node.get(node_id),
            Some(AgentStatus::Failed | AgentStatus::Interrupted)
        )
    {
        return Ok(());
    }
    let accepts_live_message = matches!(
        run_state.status_by_node.get(node_id),
        Some(AgentStatus::Started | AgentStatus::RunningTool)
    ) && session.workflow.as_ref().is_some_and(|workflow| {
        workflow
            .nodes
            .iter()
            .find(|node| node.id == *node_id)
            .is_some_and(|node| node.agent.request_user_input || node.agent.conversation_mode)
    });
    if accepts_live_message {
        return Ok(());
    }
    let expected = run_state
        .awaiting_node_id
        .clone()
        .or_else(|| run_state.awaiting_node_ids.first().cloned())
        .ok_or(BackendError::NoAwaitingInput)?;
    Err(BackendError::WrongAwaitingNode {
        expected,
        received: node_id.clone(),
    })
}

async fn remove_ingested_attachments(
    runtime_handle: &tokio::runtime::Handle,
    attachment_store: Arc<dyn RunAttachmentStore>,
    attachment_root: Option<PathBuf>,
    attachments: Vec<engine::ChatAttachmentRef>,
) {
    let Some(attachment_root) = attachment_root else {
        return;
    };
    if attachments.is_empty() {
        return;
    }
    match runtime_handle
        .spawn_blocking(move || attachment_store.remove_batch(&attachment_root, &attachments))
        .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::warn!("failed to roll back attachments: {error}"),
        Err(error) => log::warn!("failed to join attachment rollback task: {error}"),
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
