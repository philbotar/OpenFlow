use crate::auth::AuthConfig;
use crate::spec::{ProviderId, WireApi};
use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub wire_api: WireApi,
    pub responses_path: String,
    pub chat_completions_path: String,
}

impl OpenAiCompatibleConfig {
    #[must_use]
    pub fn openai_default() -> Self {
        Self {
            base_url: "https://api.openai.com".to_string(),
            wire_api: WireApi::Responses,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub messages_path: String,
    pub anthropic_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockConfig {
    pub region: String,
    pub aws_profile: Option<String>,
    pub aws_credential_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAdapterConfig {
    OpenAiCompatible(OpenAiCompatibleConfig),
    Anthropic(AnthropicConfig),
    Bedrock(BedrockConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiClientConfig {
    pub provider_id: ProviderId,
    pub provider_label: String,
    pub auth: AuthConfig,
    pub adapter: ProviderAdapterConfig,
}

#[derive(Debug, Clone)]
pub struct AiClient {
    config: AiClientConfig,
    /// Shared across clones so every turn of a run reuses the same provider
    /// model (HTTP connection pool, resolved AWS credentials).
    models: std::sync::Arc<crate::rig_adapter::ModelCache>,
}

impl AiClient {
    #[must_use]
    pub fn with_config(config: AiClientConfig) -> Self {
        Self {
            config,
            models: std::sync::Arc::new(crate::rig_adapter::ModelCache::new()),
        }
    }
}

#[async_trait]
impl AiPort for AiClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        match &self.config.adapter {
            ProviderAdapterConfig::OpenAiCompatible(_) => {
                crate::rig_adapter::invoke_openai_compatible(&self.config, &self.models, request)
                    .await
            }
            ProviderAdapterConfig::Anthropic(_) => {
                crate::rig_adapter::invoke_anthropic(&self.config, &self.models, request).await
            }
            ProviderAdapterConfig::Bedrock(_) => {
                bedrock_invoke(&self.config, &self.models, request).await
            }
        }
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        match &self.config.adapter {
            ProviderAdapterConfig::OpenAiCompatible(_) => {
                crate::rig_adapter::invoke_openai_compatible_stream(
                    &self.config,
                    &self.models,
                    request,
                    sink,
                )
                .await
            }
            ProviderAdapterConfig::Anthropic(_) => {
                crate::rig_adapter::invoke_anthropic_stream(
                    &self.config,
                    &self.models,
                    request,
                    sink,
                )
                .await
            }
            ProviderAdapterConfig::Bedrock(_) => {
                bedrock_invoke_stream(&self.config, &self.models, request, sink).await
            }
        }
    }
}

#[cfg(feature = "bedrock")]
async fn bedrock_invoke(
    config: &AiClientConfig,
    models: &crate::rig_adapter::ModelCache,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    crate::rig_adapter::invoke_bedrock(config, models, request).await
}

#[cfg(not(feature = "bedrock"))]
async fn bedrock_invoke(
    _config: &AiClientConfig,
    _models: &crate::rig_adapter::ModelCache,
    _request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    Err(AgentError::Failed(
        "Bedrock support is disabled (enable the `bedrock` feature)".into(),
    ))
}

#[cfg(feature = "bedrock")]
async fn bedrock_invoke_stream(
    config: &AiClientConfig,
    models: &crate::rig_adapter::ModelCache,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    crate::rig_adapter::invoke_bedrock_stream(config, models, request, sink).await
}

#[cfg(not(feature = "bedrock"))]
async fn bedrock_invoke_stream(
    _config: &AiClientConfig,
    _models: &crate::rig_adapter::ModelCache,
    _request: AgentRequest,
    _sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    Err(AgentError::Failed(
        "Bedrock support is disabled (enable the `bedrock` feature)".into(),
    ))
}
