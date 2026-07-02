use crate::adapters::storage::run_checkpoint_store::FileRunCheckpointStore;
use crate::run::coordinator::{DurableResumeParams, RunStartParams};
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::RunStoreRoot;
use crate::run::state::WorkflowRunState;
use engine::Workflow;
use tokio::sync::mpsc::UnboundedReceiver;

use super::{AppBackend, BackendError, FileEditPreview, ScheduledRunCandidate};

impl AppBackend {
    fn run_roots(&self) -> Result<Vec<RunStoreRoot>, BackendError> {
        let mut roots = vec![RunStoreRoot {
            project_id: None,
            root: FileRunCheckpointStore::app_runs_root(),
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

    fn run_root_for_workflow(&self, workflow_id: &str) -> Result<RunStoreRoot, BackendError> {
        for project in self.projects.load()? {
            if project.workflow_ids.iter().any(|id| id == workflow_id) {
                return Ok(RunStoreRoot {
                    project_id: Some(project.id),
                    root: std::path::Path::new(&project.path)
                        .join(".flow")
                        .join("runs"),
                });
            }
        }
        Ok(RunStoreRoot {
            project_id: None,
            root: FileRunCheckpointStore::app_runs_root(),
        })
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
        self.runs
            .replay_run(self.run_store.as_ref(), &roots, run_id)
    }

    pub async fn resume_durable_run(
        &self,
        run_id: &str,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>, String), BackendError> {
        let roots = self.run_roots()?;
        let (root, record) = self
            .run_store
            .load_record(&roots, run_id)?
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))?;
        let workflow_name = record.workflow_name.clone();
        let checkpoint = self
            .run_store
            .load_latest_checkpoint(&root, run_id)?
            .ok_or_else(|| BackendError::RunHasNoCheckpoints(run_id.to_string()))?;
        let workflow = self.load_workflow(&record.workflow_id)?;
        let (state, event_rx) = self
            .runs
            .resume_durable_run(DurableResumeParams {
                run_id,
                workflow,
                root,
                record,
                checkpoint,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))?;
        Ok((state, event_rx, workflow_name))
    }

    pub async fn start_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        execution_cwd: Option<String>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let run_root = self.run_root_for_workflow(&workflow.id)?;
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd,
                run_root,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn stop_run(&self) -> Result<WorkflowRunState, BackendError> {
        self.runs.stop_run().await
    }

    pub async fn continue_run(
        &self,
        workflow: Workflow,
        entrypoint: Option<String>,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let run_root = self.run_root_for_workflow(&workflow.id)?;
        self.runs
            .continue_run(RunStartParams {
                workflow,
                entrypoint,
                execution_cwd: None,
                run_root,
                settings,
                transient_api_key,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
                run_store: self.run_store.as_ref(),
                env: self.settings.env(),
            })
            .await
            .map_err(|error| self.backend_err(error))
    }

    #[must_use]
    pub async fn is_run_continuable(&self) -> bool {
        self.runs.is_run_continuable().await
    }

    pub async fn interrupt_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.runs.interrupt_node(node_id).await
    }

    pub async fn retry_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.runs.retry_node(node_id).await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        self.runs.is_run_active().await
    }

    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .apply_execution_event(event, self.run_store.as_ref())
            .await
    }

    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.runs
            .submit_user_input(node_id, text)
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
            .submit_tool_approval(approval_id, allow, reason)
            .await
            .map_err(|error| self.backend_err(error))
    }

    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.runs.get_run_state().await
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

    fn scheduled_execution_cwd(&self, workflow_id: &str) -> Result<Option<String>, BackendError> {
        let projects = self.projects.load()?;
        let cwd = projects
            .iter()
            .find(|project| project.workflow_ids.iter().any(|id| id == workflow_id))
            .map(|project| {
                let candidate = project.default_execution_cwd.trim();
                if candidate.is_empty() {
                    project.path.clone()
                } else {
                    candidate.to_string()
                }
            });
        Ok(cwd)
    }

    pub async fn start_scheduled_run(
        &self,
        workflow_id: String,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        if self.is_run_active().await {
            return Err(BackendError::Schedule(
                "Skipped because another workflow run was active".to_string(),
            ));
        }

        let workflow = self.load_workflow(&workflow_id)?;
        let execution_cwd = self.scheduled_execution_cwd(&workflow_id)?;
        let settings = self.load_settings(None)?.settings;
        let run_root = self.run_root_for_workflow(&workflow_id)?;
        self.runs
            .start_run(RunStartParams {
                workflow,
                entrypoint: None,
                execution_cwd,
                run_root,
                settings: &settings,
                transient_api_key: None,
                agent_store: self.agents.store(),
                settings_store: self.settings.store(),
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
        let active = self.is_run_active().await;
        Ok(self.schedule.claim_due_run(now, active))
    }
}
