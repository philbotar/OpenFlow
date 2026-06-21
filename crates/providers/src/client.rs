use crate::anthropic;
use crate::auth::AuthConfig;
#[cfg(feature = "bedrock")]
use crate::bedrock;
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
pub struct BedrockConfig {
    pub region: String,
    pub aws_profile: Option<String>,
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

/// Time allowed to establish a connection to the provider.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Time allowed between reads on a response. Converts a stalled SSE stream
/// (provider stops sending, dead TCP path after sleep/wake) into a transient
/// error the retry policy can handle, instead of hanging the node forever.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(2);

#[derive(Debug, Clone)]
pub struct AiClient {
    http: Client,
    config: AiClientConfig,
}

impl AiClient {
    #[must_use]
    pub fn with_config(config: AiClientConfig) -> Self {
        let http = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { http, config }
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
                openai_compat::invoke(&self.http, config, &self.config.auth, &self.config, request)
                    .await
            }
            ProviderAdapterConfig::Anthropic(config) => {
                anthropic::invoke(&self.http, config, &self.config.auth, request).await
            }
            ProviderAdapterConfig::Bedrock(config) => {
                bedrock_invoke(config, &self.config.auth, request).await
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
                openai_compat::invoke_stream(
                    &self.http,
                    config,
                    &self.config.auth,
                    &self.config,
                    request,
                    sink,
                )
                .await
            }
            ProviderAdapterConfig::Anthropic(config) => {
                anthropic::invoke_stream(&self.http, config, &self.config.auth, request, sink).await
            }
            ProviderAdapterConfig::Bedrock(config) => {
                bedrock_invoke_stream(config, &self.config.auth, request, sink).await
            }
        }
    }
}

#[cfg(feature = "bedrock")]
async fn bedrock_invoke(
    config: &BedrockConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    bedrock::invoke(config, auth, request).await
}

#[cfg(not(feature = "bedrock"))]
fn bedrock_invoke(
    _config: &BedrockConfig,
    _auth: &AuthConfig,
    _request: AgentRequest,
) -> std::future::Ready<Result<AgentTurnOutcome, AgentError>> {
    std::future::ready(Err(AgentError::Failed(
        "Bedrock provider requires the providers `bedrock` feature".into(),
    )))
}

#[cfg(feature = "bedrock")]
async fn bedrock_invoke_stream(
    config: &BedrockConfig,
    auth: &AuthConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    bedrock::invoke_stream(config, auth, request, sink).await
}

#[cfg(not(feature = "bedrock"))]
fn bedrock_invoke_stream(
    _config: &BedrockConfig,
    _auth: &AuthConfig,
    _request: AgentRequest,
    _sink: &dyn AiStreamSink,
) -> std::future::Ready<Result<AgentTurnOutcome, AgentError>> {
    std::future::ready(Err(AgentError::Failed(
        "Bedrock provider requires the providers `bedrock` feature".into(),
    )))
}
