#![allow(clippy::multiple_crate_versions)]

pub(crate) mod anthropic;
pub(crate) mod auth;
mod client;
pub(crate) mod mapping;
pub(crate) mod openai_compat;
mod sse;
mod spec;

pub use auth::AuthConfig;
pub use client::{AiClient, AiClientConfig, AnthropicConfig, ProviderAdapterConfig};
pub use engine::AiPort;
pub use openai_compat::OpenAiCompatibleConfig;
pub use spec::{
    builtin_provider_specs, provider_spec, AnthropicSpec, AuthSpec, OpenAiCompatibleSpec,
    ProviderId, ProviderKind, ProviderSpec, WireApi,
};

pub type OpenAiClient = AiClient;
pub type OpenAiClientConfig = AiClientConfig;
pub type OpenAiWireApi = WireApi;

#[must_use]
pub fn create_provider(config: AiClientConfig) -> Box<dyn AiPort> {
    Box::new(AiClient::with_config(config))
}
