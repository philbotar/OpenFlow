use crate::run::prep::provider_reasoning_for_profile;
use crate::settings::model::{merge_preserved_secrets, AppSettings};
use engine::CallableAgent;

use super::{AgentDefinitionSummary, AppBackend, BackendError};

impl AppBackend {
    pub fn load_agents(&self) -> Result<Vec<CallableAgent>, BackendError> {
        self.agents.load()
    }

    pub fn save_agents(&self, agents: &[CallableAgent]) -> Result<(), BackendError> {
        self.agents.save(agents)
    }

    pub fn create_agent_definition(&self, name: String) -> Result<CallableAgent, BackendError> {
        self.agents.create(name)
    }

    pub async fn create_agent_definition_with_ai(
        &self,
        description: String,
        settings: &AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<CallableAgent, BackendError> {
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
        let profile = merged.active_profile();
        let model = profile
            .default_model
            .clone()
            .unwrap_or_else(|| "gpt-5.5".to_string());
        let (reasoning_effort, reasoning_budget_tokens) = provider_reasoning_for_profile(profile);
        self.agents
            .create_with_ai(
                description,
                model,
                reasoning_effort,
                reasoning_budget_tokens,
                &*ai,
            )
            .await
    }

    pub fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        self.agents.list()
    }
}
