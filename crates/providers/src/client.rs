use crate::anthropic;
use crate::auth::AuthConfig;
use crate::openai_compat;
use crate::openai_compat::OpenAiCompatibleConfig;
use crate::spec::ProviderId;
use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink};
use reqwest::Client;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub messages_path: String,
    pub anthropic_version: String,
}

impl AnthropicConfig {
    #[must_use]
    pub fn default_base() -> Self {
        Self {
            base_url: "https://api.anthropic.com".to_string(),
            messages_path: "v1/messages".to_string(),
            anthropic_version: "2023-06-01".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAdapterConfig {
    OpenAiCompatible(OpenAiCompatibleConfig),
    Anthropic(AnthropicConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiClientConfig {
    pub provider_id: ProviderId,
    pub provider_label: String,
    pub auth: AuthConfig,
    pub adapter: ProviderAdapterConfig,
}

impl AiClientConfig {
    #[must_use]
    pub fn openai(api_key: impl Into<String>) -> Self {
        Self {
            provider_id: ProviderId::from("openai"),
            provider_label: "OpenAI".to_string(),
            auth: AuthConfig::Bearer {
                api_key: Some(api_key.into()),
                required: true,
            },
            adapter: ProviderAdapterConfig::OpenAiCompatible(
                OpenAiCompatibleConfig::openai_default(),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiClient {
    http: Client,
    config: AiClientConfig,
}

impl AiClient {
    #[must_use]
    pub fn with_config(config: AiClientConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_config(AiClientConfig::openai(api_key))
    }
}

#[async_trait]
impl AiPort for AiClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        match &self.config.adapter {
            ProviderAdapterConfig::OpenAiCompatible(config) => {
                openai_compat::invoke(&self.http, config, &self.config.auth, request).await
            }
            ProviderAdapterConfig::Anthropic(config) => {
                anthropic::invoke(&self.http, config, &self.config.auth, request).await
            }
        }
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        match &self.config.adapter {
            ProviderAdapterConfig::OpenAiCompatible(config) => {
                openai_compat::invoke_stream(&self.http, config, &self.config.auth, request, sink)
                    .await
            }
            ProviderAdapterConfig::Anthropic(config) => {
                anthropic::invoke_stream(&self.http, config, &self.config.auth, request, sink).await
            }
        }
    }
}
