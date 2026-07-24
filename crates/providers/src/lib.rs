#![allow(
    clippy::multiple_crate_versions,
    reason = "transitive dependency version duplicates are not selected by this crate"
)]

pub(crate) mod auth;
#[cfg(feature = "bedrock")]
pub(crate) mod aws_runtime;
#[cfg(feature = "bedrock")]
pub(crate) mod bedrock_errors;
#[cfg(feature = "bedrock")]
pub(crate) mod bedrock_models;
mod client;
mod codex;
pub mod codex_oauth;
pub(crate) mod http_errors;
pub(crate) mod mapping;
mod model_debug;
pub(crate) mod prompt_cache;
pub(crate) mod rig_adapter;
mod spec;

pub use auth::{AuthConfig, CodexOAuthCredentials};
#[cfg(feature = "bedrock")]
pub use aws_runtime::ensure_process_home_env;
#[cfg(feature = "bedrock")]
pub use bedrock_models::{list_bedrock_foundation_models, verify_bedrock_credentials};
pub use client::{
    AiClient, AiClientConfig, AnthropicConfig, BedrockConfig, CodexCredentialSink,
    OpenAiCodexConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
};
pub use codex_oauth::{login_codex, CodexLoginCancellation, CodexLoginPrompt};
pub use engine::AiPort;
pub use spec::{
    builtin_provider_specs, provider_spec, AnthropicSpec, AuthSpec, BedrockSpec, ModelTransport,
    OpenAiCompatibleSpec, ProviderId, ProviderKind, ProviderSpec, ReasoningEffortOption, WireApi,
};

#[must_use]
pub fn create_provider(config: AiClientConfig) -> Box<dyn AiPort> {
    match config {
        AiClientConfig {
            provider_id,
            provider_label,
            adapter: ProviderAdapterConfig::OpenAiCodex(codex_config),
            ..
        } => Box::new(codex::CodexClient::new(
            provider_id,
            provider_label,
            codex_config,
        )),
        config => Box::new(AiClient::with_config(config)),
    }
}
