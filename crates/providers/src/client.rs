use crate::auth::{AuthConfig, CodexOAuthCredentials};
use crate::spec::{ProviderId, WireApi};
use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub wire_api: WireApi,
    pub responses_path: String,
    pub chat_completions_path: String,
    pub request_timeout: Duration,
}

impl OpenAiCompatibleConfig {
    #[must_use]
    pub fn openai_default() -> Self {
        Self {
            base_url: "https://api.openai.com".to_string(),
            wire_api: WireApi::Responses,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
            request_timeout: Duration::from_mins(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub messages_path: String,
    pub anthropic_version: String,
    pub request_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockConfig {
    pub region: String,
    pub aws_profile: Option<String>,
    pub aws_credential_command: Option<String>,
}

/// Persistence boundary used after an in-run OAuth token rotation.
pub trait CodexCredentialSink: Send + Sync {
    /// # Errors
    /// Returns an error when the refreshed credentials cannot be persisted.
    fn save(&self, credentials: &CodexOAuthCredentials) -> Result<(), String>;
}

#[derive(Clone)]
pub struct OpenAiCodexConfig {
    pub base_url: String,
    pub request_timeout: Duration,
    pub credentials: CodexOAuthCredentials,
    pub credential_sink: Option<Arc<dyn CodexCredentialSink>>,
}

impl fmt::Debug for OpenAiCodexConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCodexConfig")
            .field("base_url", &self.base_url)
            .field("request_timeout", &self.request_timeout)
            .field("credentials", &self.credentials)
            .field("credential_sink_present", &self.credential_sink.is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum ProviderAdapterConfig {
    OpenAiCompatible(OpenAiCompatibleConfig),
    OpenAiCodex(OpenAiCodexConfig),
    Anthropic(AnthropicConfig),
    Bedrock(BedrockConfig),
}

#[derive(Debug, Clone)]
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
            ProviderAdapterConfig::OpenAiCodex(_) => Err(codex_not_wired_error()),
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
            ProviderAdapterConfig::OpenAiCodex(_) => Err(codex_not_wired_error()),
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

fn codex_not_wired_error() -> AgentError {
    AgentError::Failed("OpenAI Codex inference is not wired yet".into())
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

#[cfg(test)]
mod codex_contract_tests {
    use super::{OpenAiCodexConfig, ProviderAdapterConfig};
    use crate::auth::CodexOAuthCredentials;
    use crate::CodexCredentialSink;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingSink {
        saved: Mutex<Vec<CodexOAuthCredentials>>,
    }

    impl CodexCredentialSink for RecordingSink {
        fn save(&self, credentials: &CodexOAuthCredentials) -> Result<(), String> {
            self.saved
                .lock()
                .map_err(|_| "credential sink lock poisoned".to_string())?
                .push(credentials.clone());
            Ok(())
        }
    }

    #[test]
    fn codex_adapter_config_carries_a_redacted_shared_sink() {
        let credentials = CodexOAuthCredentials {
            access_token: "access-sentinel".to_string(),
            refresh_token: "refresh-sentinel".to_string(),
            id_token: Some("id-sentinel".to_string()),
            expires_at: 1_800_000_000,
            account_id: "account-sentinel".to_string(),
            email: Some("person@example.com".to_string()),
        };
        let sink = Arc::new(RecordingSink::default());
        let adapter = ProviderAdapterConfig::OpenAiCodex(OpenAiCodexConfig {
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            request_timeout: std::time::Duration::from_mins(5),
            credentials: credentials.clone(),
            credential_sink: Some(sink.clone()),
        });

        assert!(matches!(adapter, ProviderAdapterConfig::OpenAiCodex(_)));
        let ProviderAdapterConfig::OpenAiCodex(config) = &adapter else {
            return;
        };
        assert!(config.credential_sink.is_some());
        let Some(config_sink) = config.credential_sink.as_ref() else {
            return;
        };
        assert!(config_sink.save(&credentials).is_ok());
        assert!(matches!(
            sink.saved.lock().as_deref(),
            Ok(saved) if saved.as_slice() == [credentials]
        ));

        let debug = format!("{adapter:?}");
        for secret in [
            "access-sentinel",
            "refresh-sentinel",
            "id-sentinel",
            "account-sentinel",
            "person@example.com",
        ] {
            assert!(!debug.contains(secret), "debug output leaked {secret}");
        }
        assert!(debug.contains("credential_sink_present"));
    }
}
