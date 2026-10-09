use crate::api::{AttachmentPreviewPayload, StagedAttachmentPayload, UserMessageInput};
use crate::run::coordinator::{DurableResumeParams, RunStartParams};
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::{workflow_hash, RunStoreRoot};
use crate::run::state::WorkflowRunState;
use base64::Engine as _;
use engine::Workflow;
use tokio::sync::mpsc::UnboundedReceiver;

use super::{AppBackend, BackendError, FileEditPreview, ScheduledRunCandidate};

/// Refresh fields that are explicitly mutable while preserving the prepared run snapshot.
pub(super) fn refresh_runtime_config_from_workflow(
    current: &Workflow,
    snapshot: &mut Workflow,
) -> bool {
    if current.id != snapshot.id {
        return false;
    }

    let mut changed = false;
    for snapshot_node in &mut snapshot.nodes {
        let Some(current_node) = current
            .nodes
            .iter()
            .find(|node| node.id == snapshot_node.id)
        else {
            continue;
        };
        let snapshot_agent = &mut snapshot_node.agent;
        let current_agent = &current_node.agent;
        if snapshot_agent.model != current_agent.model {
            snapshot_agent.model.clone_from(&current_agent.model);
            changed = true;
        }
        if snapshot_agent.reasoning_effort != current_agent.reasoning_effort {
            snapshot_agent
                .reasoning_effort
                .clone_from(&current_agent.reasoning_effort);
            changed = true;
        }
        if snapshot_agent.reasoning_budget_tokens != current_agent.reasoning_budget_tokens {
            snapshot_agent.reasoning_budget_tokens = current_agent.reasoning_budget_tokens;
            changed = true;
        }
        if snapshot_agent.tools.approval_mode != current_agent.tools.approval_mode {
            snapshot_agent.tools.approval_mode = current_agent.tools.approval_mode;
            changed = true;
        }
    }
    if snapshot.settings.fast_mode != current.settings.fast_mode {
        snapshot.settings.fast_mode = current.settings.fast_mode;
        changed = true;
    }
    changed
}

/// Defines a pair of `AppBackend` methods that differ only in run targeting:
/// `$active` addresses the active run (`$run == None`), `$targeted` takes a
/// leading `run_id: &str` (`$run == Some(run_id)`). Both share `$body`.
macro_rules! run_scoped_pair {
    (
        $(#[$meta:meta])*
        $active:ident / $targeted:ident ($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty;
        |$this:ident, $run:ident| $body:expr
    ) => {
        $(#[$meta])*
        pub async fn $active(&self $(, $arg: $ty)*) -> $ret {
            let ($this, $run): (&Self, Option<&str>) = (self, None);
            $body
        }

        $(#[$meta])*
        pub async fn $targeted(&self, run_id: &str $(, $arg: $ty)*) -> $ret {
            let ($this, $run): (&Self, Option<&str>) = (self, Some(run_id));
            $body
        }
    };
}

impl AppBackend {
    pub(super) fn run_roots(&self) -> Result<Vec<RunStoreRoot>, BackendError> {
        let mut roots = vec![RunStoreRoot {
            project_id: None,
            root: self.app_runs_root.clone(),
        }];
        for project in self.projects.load()? {
            roots.push(RunStoreRoot {
                project_id: Some(project.id),
                root: std::path::Path::new(&project.path)
                    .join(".flow")
                    .join("runs"),
            });
        }
        Ok(roots)
    }

    pub fn list_runs(
        &self,
        workflow_id: Option<&str>,
    ) -> Result<Vec<crate::run::persistence::RunSummary>, BackendError> {
        let roots = self.run_roots()?;
        self.runs
            .list_runs(self.run_store.as_ref(), &roots, workflow_id)
    }

    pub fn replay_run(&self, run_id: &str) -> Result<WorkflowRunState, BackendError> {
        let roots = self.run_roots()?;
        let (root, _) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let mut checkpoint = self
            .run_store
            .load_latest_checkpoint(&root, run_id)?
            .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
        if self
            .chats
            .list()?
            .iter()
            .any(|chat| chat.run_id.as_deref() == Some(run_id))
        {
            checkpoint.repair_direct_chat_projection();
        }
        Ok(checkpoint.projection.into_replay_projection())
    }

    pub async fn load_chat_attachment_preview(
        &self,
        run_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentPreviewPayload, BackendError> {
        let roots = self.run_roots()?;
        let (root, _) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let current_attachment = if self.is_run_active_for(run_id).await {
            self.get_run_state_for(run_id).await.and_then(|state| {
                state
                    .chat_logs
                    .values()
                    .flatten()
                    .flat_map(|message| message.attachments.iter())
                    .find(|attachment| attachment.id == attachment_id)
                    .cloned()
            })
        } else {
            None
        };
        let attachment = match current_attachment {
            Some(attachment) => attachment,
            None => self
                .run_store
                .load_latest_checkpoint(&root, run_id)?
                .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?
                .projection
                .chat_logs
                .values()
                .flatten()
                .flat_map(|message| message.attachments.iter())
                .find(|attachment| attachment.id == attachment_id)
                .cloned()
                .ok_or_else(|| crate::run::ports::AttachmentError::Corrupt {
                    file_name: attachment_id.to_string(),
                })?,
        };
        let attachment_root = self.run_store.run_dir(&root, run_id).join("attachments");
        let preview = self
            .attachment_store
            .preview(&attachment_root, &attachment)?;
        Ok(AttachmentPreviewPayload {
            media_type: preview.media_type,
            data_base64: base64::engine::general_purpose::STANDARD.encode(preview.bytes),
        })
    }

    pub async fn load_file_change_diff(
        &self,
        run_id: &str,
        artifact_id: &str,
    ) -> Result<String, BackendError> {
        let roots = self.run_roots()?;
        let (root, _) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let current_state = if self.is_run_active_for(run_id).await {
            self.get_run_state_for(run_id).await
        } else {
            None
        };
        let state = match current_state {
            Some(state) => state,
            None => {
                self.run_store
                    .load_latest_checkpoint(&root, run_id)?
                    .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?
                    .projection
            }
        };
        let belongs_to_run = state
            .changed_files_by_node
            .values()
            .flatten()
            .chain(state.changed_files.iter())
            .any(|change| change.diff_artifact_id.as_deref() == Some(artifact_id));
        let valid_artifact_id = uuid::Uuid::parse_str(artifact_id)
            .is_ok_and(|parsed| parsed.to_string() == artifact_id);
        if !belongs_to_run || !valid_artifact_id {
            return Err(BackendError::FileChangeDiffNotFound {
                run_id: run_id.to_string(),
                artifact_id: artifact_id.to_string(),
            });
        }

        let path = self
            .run_store
            .run_dir(&root, run_id)
            .join("artifacts")
            .join(format!("{artifact_id}-file-diff.txt"));
        tokio::fs::read_to_string(path).await.map_err(|error| {
            BackendError::FileChangeDiffUnavailable {
                run_id: run_id.to_string(),
                artifact_id: artifact_id.to_string(),
                error: error.to_string(),
            }
        })
    }

    pub fn stage_chat_attachment(
        &self,
        file_name: &str,
        data_base64: &str,
    ) -> Result<StagedAttachmentPayload, BackendError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .map_err(|_| crate::run::ports::AttachmentError::TypeMismatch {
                file_name: file_name.to_string(),
            })?;
        let staged = self.attachment_store.stage(file_name, &bytes)?;
        Ok(StagedAttachmentPayload {
            token: staged.token,
            file_name: staged.file_name,
            size_bytes: staged.size_bytes,
            kind: staged.kind,
        })
    }

    pub fn remove_staged_chat_attachment(&self, token: &str) -> Result<(), BackendError> {
        Ok(self.attachment_store.remove_staged(token)?)
    }

    pub async fn resume_durable_run(
        &self,
        run_id: &str,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String), BackendError> {
        self.resume_durable_run_with_continuation(run_id, settings, transient_api_key, None, None)
            .await
    }

    pub async fn resume_durable_run_with_continuation(
        &self,
        run_id: &str,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        continuation: Option<crate::api::DurableRunContinuationInput>,
        runtime_workflow: Option<Workflow>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String), BackendError> {
        let roots = self.run_roots()?;
        let (root, mut record) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let direct_chat = self
            .chats
            .list()?
            .into_iter()
            .find(|chat| chat.run_id.as_deref() == Some(run_id));
        let runtime_config_changed = if let Some(chat) = direct_chat.as_ref() {
            super::chat::refresh_execution_workflow_for_chat(chat, &mut record.workflow_snapshot)
        } else if let Some(current) = runtime_workflow.as_ref() {
            if current.id.to_string() != record.workflow_id {
                return Err(BackendError::RunWorkflowChanged(
                    run_id.to_string(),
                    record.workflow_id.clone(),
                ));
            }
            refresh_runtime_config_from_workflow(current, &mut record.workflow_snapshot)
        } else {
            false
        };
        if runtime_config_changed {
            record.workflow_hash = workflow_hash(&record.workflow_snapshot);
            self.run_store.update_record(&root, &record)?;
        }
        let workflow_name = record.workflow_name.clone();
        let mut checkpoint = self
            .run_store
            .load_latest_checkpoint(&root, run_id)?
            .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
        if direct_chat.is_some() {
            checkpoint.discard_structured_user_input();
            checkpoint.repair_direct_chat_projection();
        }
        let (state, event_rx) = self
            .runs
            .resume_durable_run_with_continuation(
                DurableResumeParams {
                    run_id,
                    root,
                    record,
                    checkpoint,
                    settings,
                    transient_api_key,
                    agent_store: self.agents.store(),
                    skill_catalog: self.settings.skill_catalog(),
                    settings_store: self.settings.store_arc(),
                    run_store: self.run_store.as_ref(),
                    env: self.settings.env(),
                },
                continuation,
            )
            .await
            .map_err(|error| self.backend_err(error))?;
        Ok((state, event_rx, workflow_name))
    }

    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        project_id: Option<&str>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.start_run_with_skill_ids(
            workflow,
            entrypoint,
            project_id,
            settings,
            transient_api_key,
            Vec::new(),
        )
        .await
    }

    pub async fn start_run_with_skill_ids(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        project_id: Option<&str>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        invoked_skill_ids: Vec<String>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.start_run_with_message_and_skill_ids(
            workflow,
            entrypoint.map(UserMessageInput::text),
            project_id,
            settings,
            transient_api_key,
            invoked_skill_ids,
        )
        .await
    }

    pub async fn start_run_with_message_and_skill_ids(
        &self,
        workflow: Workflow,
        message: Option<UserMessageInput>,
        project_id: Option<&str>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        invoked_skill_ids: Vec<String>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let workspace = self.workspace_for_workflow(&workflow.id, project_id)?;
        self.start_run_with_root(
            workflow,
            message,
            Some(workspace.execution_cwd),
            workspace.run_root,
            settings,
            transient_api_key,
            invoked_skill_ids,
        )
        .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "internal launch seam keeps run root, auth, settings, and skill inputs explicit"
    )]
    pub(super) async fn start_run_with_root(
        &self,
        workflow: Workflow,
        message: Option<UserMessageInput>,
        execution_cwd: Option<String>,
        run_root: RunStoreRoot,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
        invoked_skill_ids: Vec<String>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        self.runs
            .start_run(RunStartParams {
                workflow,
                invoked_skill_ids,
                entrypoint: message,
                execution_cwd,
                run_root,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                skill_catalog: self.settings.skill_catalog(),
                settings_store: self.settings.store_arc(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))
    }

    run_scoped_pair! {
        stop_run / stop_run_for() -> Result<WorkflowRunState, BackendError>;
        |this, run| this.runs.stop_run_for(run, this.run_store.as_ref()).await
    }

    pub async fn stop_all_runs(&self) {
        self.runs
            .stop_all_and_persist(self.run_store.as_ref())
            .await;
    }

    run_scoped_pair! {
        continue_run / continue_run_for(
            workflow: Workflow,
            entrypoint: Option<String>,
            settings: &crate::settings::model::AppSettings,
            transient_api_key: Option<&str>,
        ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError>;
        |this, run| this
            .runs
            .continue_run_for(
                run,
                RunStartParams {
                    workflow,
                    invoked_skill_ids: Vec::new(),
                    entrypoint: entrypoint.map(UserMessageInput::text),
                    execution_cwd: None,
                    run_root: RunStoreRoot {
                        project_id: None,
                        root: this.app_runs_root.clone(),
                    },
                    settings,
                    transient_api_key,
                    agent_store: this.agents.store(),
                    skill_catalog: this.settings.skill_catalog(),
                    settings_store: this.settings.store_arc(),
                    run_store: this.run_store.as_ref(),
                    env: this.settings.env(),
                },
            )
            .await
            .map_err(|error| this.backend_err(error))
    }

    run_scoped_pair! {
        #[must_use]
        is_run_continuable / is_run_continuable_for() -> bool;
        |this, run| this.runs.is_run_continuable_for(run).await
    }

    run_scoped_pair! {
        interrupt_node / interrupt_node_for(node_id: &str) -> Result<WorkflowRunState, BackendError>;
        |this, run| this.runs.interrupt_node_for(run, node_id).await
    }

    run_scoped_pair! {
        retry_node / retry_node_for(node_id: &str) -> Result<WorkflowRunState, BackendError>;
        |this, run| this.runs.retry_node_for(run, node_id).await
    }

    run_scoped_pair! {
        update_node_runtime_config / update_node_runtime_config_for(
            node_id: &str,
            update: crate::api::NodeRuntimeConfigUpdate,
        ) -> Result<WorkflowRunState, BackendError>;
        |this, run| this
            .runs
            .update_node_runtime_config_for(run, node_id, update.into_patch())
            .await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.runs.is_run_active().await
    }

    #[must_use]
    pub async fn is_run_active_for(&self, run_id: &str) -> bool {
        self.runs.is_run_active_for(run_id).await
    }

    run_scoped_pair! {
        apply_execution_event / apply_execution_event_for(event: ExecutionEvent)
            -> Result<WorkflowRunState, BackendError>;
        |this, run| this
            .runs
            .apply_execution_event_for(run, event, this.run_store.as_ref())
            .await
    }

    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.submit_user_input_with_skill_ids(node_id, text, Vec::new())
            .await
    }

    pub async fn submit_user_input_with_skill_ids(
        &self,
        node_id: &str,
        text: String,
        invoked_skill_ids: Vec<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.submit_user_message_with_skill_ids(
            node_id,
            UserMessageInput::text(text),
            invoked_skill_ids,
        )
        .await
    }

    pub async fn submit_user_message_with_skill_ids(
        &self,
        node_id: &str,
        message: UserMessageInput,
        invoked_skill_ids: Vec<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_user_message_with_skill_ids_for(None, node_id, message, &invoked_skill_ids)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn submit_user_message_with_skill_ids_for(
        &self,
        run_id: &str,
        node_id: &str,
        message: UserMessageInput,
        invoked_skill_ids: Vec<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_user_message_with_skill_ids_for(
                Some(run_id),
                node_id,
                message,
                &invoked_skill_ids,
            )
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_tool_approval_for(None, approval_id, allow, reason)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn submit_tool_approval_for(
        &self,
        run_id: &str,
        approval_id: &str,
        allow: bool,
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_tool_approval_for(Some(run_id), approval_id, allow, reason)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn resolve_mcp_client_request(
        &self,
        request_id: &str,
        decision: crate::mcp::client_capabilities::McpClientRequestDecision,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .resolve_mcp_client_request_for(None, request_id, decision)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn resolve_mcp_client_request_for(
        &self,
        run_id: &str,
        request_id: &str,
        decision: crate::mcp::client_capabilities::McpClientRequestDecision,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .resolve_mcp_client_request_for(Some(run_id), request_id, decision)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.runs.get_run_state().await
    }

    pub async fn get_run_state_for(&self, run_id: &str) -> Option<WorkflowRunState> {
        self.runs.get_run_state_for(run_id).await
    }

    pub async fn active_run_states(&self) -> Vec<WorkflowRunState> {
        self.runs.active_run_states().await
    }

    pub async fn current_run_id(&self) -> Option<String> {
        self.runs.current_run_id().await
    }

    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.runs
            .preview_file_edit_for(None, approval_id, tool_name, arguments)
            .await
    }

    pub async fn preview_file_edit_for(
        &self,
        run_id: &str,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.runs
            .preview_file_edit_for(Some(run_id), approval_id, tool_name, arguments)
            .await
    }

    pub async fn git_diff_file(&self, path: String) -> Result<String, BackendError> {
        self.runs.git_diff_file_for(None, path).await
    }

    pub async fn git_diff_file_for(
        &self,
        run_id: &str,
        path: String,
    ) -> Result<String, BackendError> {
        self.runs.git_diff_file_for(Some(run_id), path).await
    }

    pub async fn revert_edit_batch(
        &self,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs.revert_edit_batch_for(None, batch_id).await
    }

    pub async fn revert_edit_batch_for(
        &self,
        run_id: &str,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .revert_edit_batch_for(Some(run_id), batch_id)
            .await
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        self.runs.clear_run_trace_for(None).await
    }

    pub async fn clear_run_trace_for(
        &self,
        run_id: &str,
    ) -> Result<Option<WorkflowRunState>, BackendError> {
        self.runs.clear_run_trace_for(Some(run_id)).await
    }

    pub async fn start_scheduled_run(
        &self,
        workflow_id: String,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let workflow = self.load_workflow(&workflow_id)?;
        let workspace = self.workspace_for_workflow(&workflow_id, None)?;
        let settings = self.load_settings(None)?.settings;
        self.runs
            .start_run(RunStartParams {
                workflow,
                invoked_skill_ids: Vec::new(),
                entrypoint: None,
                execution_cwd: Some(workspace.execution_cwd),
                run_root: workspace.run_root,
                settings: &settings,
                transient_api_key: None,
                agent_store: self.agents.store(),
                skill_catalog: self.settings.skill_catalog(),
                settings_store: self.settings.store_arc(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| {
                self.schedule
                    .record_start_error(&workflow_id, error.to_string());
                self.backend_err(error)
            })
    }

    pub async fn start_due_scheduled_run(
        &self,
    ) -> Result<Option<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String)>, BackendError>
    {
        let Some(candidate) = self.claim_due_scheduled_run().await? else {
            return Ok(None);
        };
        let workflow_name = self.load_workflow(&candidate.workflow_id)?.name;
        let (state, event_rx) = self.start_scheduled_run(candidate.workflow_id).await?;
        Ok(Some((state, event_rx, workflow_name)))
    }

    pub async fn claim_due_scheduled_run(
        &self,
    ) -> Result<Option<ScheduledRunCandidate>, BackendError> {
        self.claim_due_scheduled_run_at(chrono::Utc::now()).await
    }

    pub async fn claim_due_scheduled_run_at(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ScheduledRunCandidate>, BackendError> {
        let active_workflow_ids = self
            .active_run_states()
            .await
            .into_iter()
            .filter_map(|state| state.workflow_id)
            .collect();
        Ok(self.schedule.claim_due_run(now, &active_workflow_ids))
    }
}
