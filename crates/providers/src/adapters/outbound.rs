//! Outbound adapters for provider wire protocols.

use crate::auth::AuthConfig;
use crate::client::AnthropicConfig;
use crate::openai_compat::OpenAiCompatibleConfig;
use crate::ports::outbound::ProviderInvokeResult;
use domain::AgentRequest;
use reqwest::Client;

/// # Errors
/// Returns an error when auth, transport, provider status, or response mapping fails.
pub async fn invoke_openai_compatible(
    http: &Client,
    config: &OpenAiCompatibleConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> ProviderInvokeResult {
    crate::openai_compat::invoke(http, config, auth, request).await
}

/// # Errors
/// Returns an error when auth, transport, provider status, or response mapping fails.
pub async fn invoke_anthropic(
    http: &Client,
    config: &AnthropicConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> ProviderInvokeResult {
    crate::anthropic::invoke(http, config, auth, request).await
}
