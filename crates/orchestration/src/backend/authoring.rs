use crate::api::{
    WorkflowAuthoringDraftEvent, WorkflowAuthoringRuntimeConfig, WorkflowAuthoringStartResult,
    WorkflowAuthoringThinkingEvent,
};
use crate::settings::model::{merge_preserved_secrets, AppSettings};
use crate::workflow::authoring::{WorkflowAuthoringEventHandlers, WorkflowAuthoringProjectContext};
use engine::Workflow;

use super::{AppBackend, BackendError, WorkflowAuthoringTurnResult};

impl AppBackend {
    pub fn start_workflow_authoring(
        &self,
        base_workflow: Option<Workflow>,
        target_project_id: Option<&str>,
    ) -> Result<WorkflowAuthoringStartResult, BackendError> {
        let Some(target_project_id) = target_project_id else {
            return Ok(self.workflow_authoring.start_session(base_workflow));
        };
        let project = self
            .projects
            .load()?
            .into_iter()
            .find(|project| project.id == target_project_id)
            .ok_or_else(|| BackendError::ProjectNotFound(target_project_id.to_string()))?;
        Ok(self.workflow_authoring.start_project_session(
            base_workflow,
            WorkflowAuthoringProjectContext {
                id: project.id,
                name: project.name,
                path: project.path,
                default_execution_cwd: Some(project.default_execution_cwd),
            },
        ))
    }

    pub fn end_workflow_authoring(&self, session_id: &str) -> bool {
        self.workflow_authoring.end_session(session_id)
    }

    pub async fn workflow_authoring_turn<F, G>(
        &self,
        session_id: String,
        message: String,
        settings: &AppSettings,
        runtime_config: &WorkflowAuthoringRuntimeConfig,
        transient_api_key: Option<&str>,
        event_handlers: WorkflowAuthoringEventHandlers<F, G>,
    ) -> Result<WorkflowAuthoringTurnResult, BackendError>
    where
        F: Fn(WorkflowAuthoringThinkingEvent) + Send + Sync,
        G: Fn(WorkflowAuthoringDraftEvent) + Send + Sync,
    {
        let mut merged = settings.clone();
        merge_preserved_secrets(&mut merged, &self.settings.store().load()?);
        let mut provider_config = crate::settings::provider::resolve_provider_config(
            &merged,
            transient_api_key,
            self.settings.env(),
        )?;
        crate::settings::provider::attach_codex_credential_sink(
            &mut provider_config,
            self.settings.store_arc(),
        );
        let ai = providers::create_provider(provider_config);
        self.workflow_authoring
            .send_turn_with_runtime_config(
                &session_id,
                message,
                &merged,
                runtime_config,
                &ai,
                event_handlers,
            )
            .await
            .map_err(Into::into)
    }
}
