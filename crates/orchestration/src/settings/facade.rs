use crate::api::{ProviderReadiness, WorkflowValidationSummary};
use crate::error::BackendError;
use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::ports::{SettingsStore, SkillCatalog, SkillSummary};
use crate::settings::provider::{
    active_provider_env_var, active_provider_label, resolve_provider_config, ProviderConfigError,
    ProviderEnv,
};
#[cfg(feature = "bedrock")]
use engine::AgentError;
use engine::{execution_layers, validate_workflow, Workflow};
#[cfg(feature = "bedrock")]
use providers::{list_bedrock_foundation_models, verify_bedrock_credentials};
use providers::{ProviderAdapterConfig, ProviderId};

pub struct SettingsFacade {
    store: Box<dyn SettingsStore>,
    skills: Box<dyn SkillCatalog>,
    env: ProviderEnv,
}

impl SettingsFacade {
    #[must_use]
    pub fn new(
        store: Box<dyn SettingsStore>,
        skills: Box<dyn SkillCatalog>,
        env: ProviderEnv,
    ) -> Self {
        Self { store, skills, env }
    }

    /// # Errors
    /// Returns an error if skill discovery fails.
    pub fn list_skills(&self) -> Result<Vec<SkillSummary>, BackendError> {
        let settings = self.store.load()?;
        self.skills
            .discover(&settings.skill_search_paths)
            .map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read.
    pub fn load(&self) -> Result<AppSettings, BackendError> {
        Ok(self.store.load()?.redacted())
    }

    /// # Errors
    /// Returns an error if the settings file cannot be written.
    pub fn save(&self, settings: &AppSettings) -> Result<(), BackendError> {
        self.store.save(settings).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read.
    pub fn load_provider_api_key(&self, provider_id: &str) -> Result<Option<String>, BackendError> {
        let settings = self.store.load()?;
        let provider_id = ProviderId::from(provider_id);
        Ok(settings.providers.get(&provider_id).and_then(|profile| {
            let key = profile.api_key.trim();
            if key.is_empty() {
                None
            } else {
                Some(key.to_string())
            }
        }))
    }

    /// # Errors
    /// Returns an error if the settings file cannot be written.
    pub fn save_provider_api_key(
        &self,
        provider_id: &str,
        api_key: &str,
    ) -> Result<(), BackendError> {
        let mut settings = self.store.load()?;
        let provider_id = ProviderId::from(provider_id);
        let profile = settings.providers.get_mut(&provider_id).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("provider {provider_id} not found"),
            )
        })?;
        profile.api_key = api_key.trim().to_string();
        self.store.save_raw(&settings)?;
        Ok(())
    }

    /// # Errors
    /// Returns an error if the settings file cannot be written.
    pub fn delete_provider_api_key(&self, provider_id: &str) -> Result<(), BackendError> {
        self.save_provider_api_key(provider_id, "")
    }

    #[must_use]
    pub fn resolve_provider_readiness(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> ProviderReadiness {
        let Ok(persisted) = self.store.load() else {
            return ProviderReadiness {
                ready: false,
                provider: active_provider_label(settings),
                message: "Failed to load settings".to_string(),
                env_var: active_provider_env_var(settings)
                    .unwrap_or_default()
                    .to_string(),
            };
        };
        let mut merged = settings.clone();
        merge_preserved_api_keys(&mut merged, &persisted);

        match resolve_provider_config(&merged, transient_api_key, &self.env) {
            Ok(config) if matches!(config.adapter, ProviderAdapterConfig::Bedrock(_)) => {
                ProviderReadiness {
                    ready: true,
                    provider: active_provider_label(settings),
                    message: "Region configured — use Test AWS connection to verify credentials"
                        .to_string(),
                    env_var: active_provider_env_var(settings)
                        .unwrap_or_default()
                        .to_string(),
                }
            }
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
                message: "API key missing".to_string(),
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

    /// # Errors
    /// Returns an error if Bedrock model discovery fails or the Bedrock profile is missing.
    pub async fn refresh_bedrock_models(
        &self,
        settings: &AppSettings,
    ) -> Result<Vec<String>, BackendError> {
        let mut merged = settings.clone();
        merge_preserved_api_keys(&mut merged, &self.store.load()?);
        #[cfg(feature = "bedrock")]
        {
            let config = resolve_provider_config(&merged, None, &self.env).map_err(|error| {
                BackendError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    error.to_string(),
                ))
            })?;
            let ProviderAdapterConfig::Bedrock(bedrock) = config.adapter else {
                return Err(BackendError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "active provider is not Amazon Bedrock",
                )));
            };
            return list_bedrock_foundation_models(&bedrock.region, bedrock.aws_profile.as_deref())
                .await
                .map_err(map_agent_error_to_backend);
        }
        #[cfg(not(feature = "bedrock"))]
        {
            let _ = merged;
            Err(BackendError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Bedrock model refresh requires the orchestration `bedrock` feature",
            )))
        }
    }

    /// # Errors
    /// Returns an error if Bedrock credentials cannot be loaded for the active profile.
    pub async fn verify_bedrock_credentials(
        &self,
        settings: &AppSettings,
    ) -> Result<String, BackendError> {
        let mut merged = settings.clone();
        merge_preserved_api_keys(&mut merged, &self.store.load()?);
        #[cfg(feature = "bedrock")]
        {
            let config = resolve_provider_config(&merged, None, &self.env).map_err(|error| {
                BackendError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    error.to_string(),
                ))
            })?;
            let ProviderAdapterConfig::Bedrock(bedrock) = config.adapter else {
                return Err(BackendError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "active provider is not Amazon Bedrock",
                )));
            };
            return verify_bedrock_credentials(&bedrock.region, bedrock.aws_profile.as_deref())
                .await
                .map_err(map_agent_error_to_backend);
        }
        #[cfg(not(feature = "bedrock"))]
        {
            let _ = merged;
            Err(BackendError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Bedrock credential verification requires the orchestration `bedrock` feature",
            )))
        }
    }

    pub(crate) fn store(&self) -> &dyn SettingsStore {
        &*self.store
    }

    pub(crate) fn env(&self) -> &ProviderEnv {
        &self.env
    }
}

#[cfg(feature = "bedrock")]
fn map_agent_error_to_backend(error: AgentError) -> BackendError {
    BackendError::from(std::io::Error::other(error.to_string()))
}
