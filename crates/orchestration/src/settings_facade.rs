use crate::api::{ProviderReadiness, WorkflowValidationSummary};
use crate::error::BackendError;
use crate::provider_config::{
    active_provider_env_var, active_provider_label, resolve_provider_config, ProviderConfigError,
    ProviderEnv,
};
use crate::settings_store::{key_ref_for_provider, AppSettings, FileSettingsStore};
use crate::skill_store::{self, SkillSummary};
use domain::{execution_layers, validate_workflow, Workflow};
use providers::ProviderId;

#[derive(Debug)]
pub struct SettingsFacade {
    store: FileSettingsStore,
    env: ProviderEnv,
}

impl SettingsFacade {
    #[must_use]
    pub fn new(store: FileSettingsStore, env: ProviderEnv) -> Self {
        Self { store, env }
    }

    /// # Errors
    /// Returns an error if skill discovery fails.
    pub fn list_skills(&self) -> Result<Vec<SkillSummary>, BackendError> {
        let settings = self.store.load()?;
        skill_store::discover(&settings.skill_search_paths).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read.
    pub fn load(&self) -> Result<AppSettings, BackendError> {
        self.store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the settings file cannot be written.
    pub fn save(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.store.save(settings).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the credential store cannot be read.
    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        let provider_id = ProviderId::from(provider_id);
        Ok(self
            .store
            .credential_store()
            .get(&key_ref_for_provider(&provider_id))?)
    }

    /// # Errors
    /// Returns an error if the credential store cannot be written.
    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        let provider_id = ProviderId::from(provider_id);
        let key_ref = key_ref_for_provider(&provider_id);
        let api_key = api_key.trim();
        if api_key.is_empty() {
            self.store.credential_store().delete(&key_ref)?;
        } else {
            self.store.credential_store().set(&key_ref, api_key)?;
        }
        Ok(())
    }

    /// # Errors
    /// Returns an error if the credential store cannot delete the key.
    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        let provider_id = ProviderId::from(provider_id);
        self.store
            .credential_store()
            .delete(&key_ref_for_provider(&provider_id))?;
        Ok(())
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        match resolve_provider_config(
            settings,
            transient_api_key,
            &self.env,
            self.store.credential_store(),
        ) {
            Ok(_) => ProviderReadiness {
                ready: true,
                provider: active_provider_label(settings),
                message: "Ready".to_string(),
                env_var: active_provider_env_var(settings)
                    .unwrap_or_default()
                    .to_string(),
            },
            Err(ProviderConfigError::MissingApiKey { provider, env_var }) => ProviderReadiness {
                ready: false,
                provider,
                message: format!("API key missing (set it in Settings or {env_var})"),
                env_var,
            },
            Err(error) => ProviderReadiness {
                ready: false,
                provider: active_provider_label(settings),
                message: error.to_string(),
                env_var: active_provider_env_var(settings)
                    .unwrap_or_default()
                    .to_string(),
            },
        }
    }

    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn validate_workflow(
        &self,
        workflow: &Workflow,
    ) -> Result<WorkflowValidationSummary, BackendError> {
        validate_workflow(workflow)?;
        let layers = execution_layers(workflow)?;
        Ok(WorkflowValidationSummary {
            layer_count: layers.len(),
            layers: layers
                .into_iter()
                .map(|layer| layer.into_iter().map(|id| id.to_string()).collect())
                .collect(),
        })
    }

    pub(crate) fn store(&self) -> &FileSettingsStore {
        &self.store
    }

    pub(crate) fn env(&self) -> &ProviderEnv {
        &self.env
    }
}
