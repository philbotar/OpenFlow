use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use engine::Workflow;

use super::{AppBackend, BackendError, WorkflowAuthoringTurnResult};

impl AppBackend {
    pub fn start_workflow_authoring(&self, base_workflow: Option<Workflow>) -> String {
        self.workflow_authoring.start_session(base_workflow)
    }

    pub fn end_workflow_authoring(&self, session_id: &str) -> bool {
        self.workflow_authoring.end_session(session_id)
    }

    pub async fn workflow_authoring_turn(
        &self,
        session_id: String,
        message: String,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<WorkflowAuthoringTurnResult, BackendError> {
        let mut merged = settings.clone();
        merge_preserved_api_keys(&mut merged, &self.settings.store().load()?);
        let provider_config = crate::settings::provider::resolve_provider_config(
            &merged,
            transient_api_key,
            self.settings.env(),
        )?;
        let ai = providers::create_provider(provider_config);
        self.workflow_authoring
            .send_turn(&session_id, message, &merged, &ai)
            .await
            .map_err(Into::into)
    }
}
