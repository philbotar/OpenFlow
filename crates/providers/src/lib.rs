#![allow(
    clippy::multiple_crate_versions,
    reason = "transitive dependency version duplicates are not selected by this crate"
)]

pub(crate) mod anthropic;
pub(crate) mod auth;
mod client;
pub(crate) mod mapping;
pub(crate) mod openai_compat;
pub(crate) mod prompt_cache;
mod spec;
mod sse;

pub use auth::AuthConfig;
pub use client::{AiClient, AiClientConfig, AnthropicConfig, ProviderAdapterConfig};
pub use engine::AiPort;
pub use openai_compat::OpenAiCompatibleConfig;
pub use spec::{
    builtin_provider_specs, provider_spec, AnthropicSpec, AuthSpec, OpenAiCompatibleSpec,
    ProviderId, ProviderKind, ProviderSpec, ReasoningEffortOption, WireApi,
};

pub type OpenAiClient = AiClient;
pub type OpenAiClientConfig = AiClientConfig;
pub type OpenAiWireApi = WireApi;

#[must_use]
pub fn create_provider(config: AiClientConfig) -> Box<dyn AiPort> {
    Box::new(AiClient::with_config(config))
}
