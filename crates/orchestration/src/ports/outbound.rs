//! Outbound ports for orchestration dependencies.

use crate::agent_store::AgentDefinition;
use crate::credential_store::{CredentialStore, CredentialStoreError};
use crate::provider_config::{ProviderConfigError, ProviderEnv};
use crate::settings_store::AppSettings;
use ai::AiClientConfig;
use std::io;
use workflow_core::Workflow;

pub trait WorkflowStoragePort {
    fn load_workflows(&self) -> Result<Vec<Workflow>, io::Error>;
    fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), io::Error>;
}

pub trait AgentStoragePort {
    fn load_agents(&self) -> Result<Vec<AgentDefinition>, io::Error>;
    fn save_agents(&self, agents: &[AgentDefinition]) -> Result<(), io::Error>;
}

pub trait SettingsStoragePort {
    fn load_settings(&self) -> Result<AppSettings, io::Error>;
    fn save_settings(&self, settings: &AppSettings) -> Result<(), io::Error>;
    fn credential_store(&self) -> &CredentialStore;
}

pub trait CredentialLookupPort {
    fn get(&self, key_ref: &str) -> Result<Option<String>, CredentialStoreError>;
    fn set(&self, key_ref: &str, value: &str) -> Result<(), CredentialStoreError>;
    fn delete(&self, key_ref: &str) -> Result<(), CredentialStoreError>;
}

pub trait ProviderConfigResolverPort {
    fn resolve_provider_config(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
        env: &ProviderEnv,
        credential_store: &CredentialStore,
    ) -> Result<AiClientConfig, ProviderConfigError>;
}
