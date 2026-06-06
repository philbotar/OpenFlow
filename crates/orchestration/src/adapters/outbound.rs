//! Outbound adapters for orchestration dependencies.

use crate::agent_store::{AgentDefinition, FileAgentStore};
use crate::credential_store::{CredentialStore, CredentialStoreError};
use crate::ports::outbound::{
    AgentStoragePort, CredentialLookupPort, ProviderConfigResolverPort, SettingsStoragePort,
    WorkflowStoragePort,
};
use crate::provider_config::{resolve_provider_config, ProviderConfigError, ProviderEnv};
use crate::settings_store::{AppSettings, FileSettingsStore};
use crate::storage::FileWorkflowStore;
use domain::Workflow;
use providers::AiClientConfig;
use std::io;

impl WorkflowStoragePort for FileWorkflowStore {
    fn load_workflows(&self) -> Result<Vec<Workflow>, io::Error> {
        FileWorkflowStore::load(self)
    }

    fn save_workflows(&self, workflows: &[Workflow]) -> Result<(), io::Error> {
        FileWorkflowStore::save(self, workflows)
    }
}

impl AgentStoragePort for FileAgentStore {
    fn load_agents(&self) -> Result<Vec<AgentDefinition>, io::Error> {
        FileAgentStore::load(self)
    }

    fn save_agents(&self, agents: &[AgentDefinition]) -> Result<(), io::Error> {
        FileAgentStore::save(self, agents)
    }
}

impl SettingsStoragePort for FileSettingsStore {
    fn load_settings(&self) -> Result<AppSettings, io::Error> {
        FileSettingsStore::load(self)
    }

    fn save_settings(&self, settings: &AppSettings) -> Result<(), io::Error> {
        FileSettingsStore::save(self, settings)
    }

    fn credential_store(&self) -> &CredentialStore {
        FileSettingsStore::credential_store(self)
    }
}

impl CredentialLookupPort for CredentialStore {
    fn get(&self, key_ref: &str) -> Result<Option<String>, CredentialStoreError> {
        CredentialStore::get(self, key_ref)
    }

    fn set(&self, key_ref: &str, value: &str) -> Result<(), CredentialStoreError> {
        CredentialStore::set(self, key_ref, value)
    }

    fn delete(&self, key_ref: &str) -> Result<(), CredentialStoreError> {
        CredentialStore::delete(self, key_ref)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultProviderConfigResolver;

impl ProviderConfigResolverPort for DefaultProviderConfigResolver {
    fn resolve_provider_config(
        &self,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
        env: &ProviderEnv,
        credential_store: &CredentialStore,
    ) -> Result<AiClientConfig, ProviderConfigError> {
        resolve_provider_config(settings, transient_api_key, env, credential_store)
    }
}
