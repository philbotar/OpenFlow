use crate::http_errors::classify_http_status;
use crate::{
    AiClientConfig, AnthropicConfig, AuthConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
};
use engine::AgentError;
use reqwest::RequestBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelList {
    data: Vec<ModelListItem>,
}

#[derive(Debug, Deserialize)]
struct ModelListItem {
    id: String,
}

/// Lists models exposed by the configured provider HTTP API.
///
/// # Errors
///
/// Returns an error when the provider has no model-list endpoint, auth is
/// invalid, the request fails, or the response is not a model list.
pub async fn list_remote_models(config: &AiClientConfig) -> Result<Vec<String>, AgentError> {
    let (url, anthropic_version, request_timeout) = match &config.adapter {
        ProviderAdapterConfig::OpenAiCompatible(openai) => {
            (openai_models_url(openai), None, openai.request_timeout)
        }
        ProviderAdapterConfig::Anthropic(anthropic) => (
            anthropic_models_url(anthropic),
            Some(anthropic.anthropic_version.as_str()),
            anthropic.request_timeout,
        ),
        ProviderAdapterConfig::OpenAiCodex(_) => {
            return Err(AgentError::Permanent(
                "ChatGPT model discovery is not available through the Codex endpoint".to_string(),
            ));
        }
        ProviderAdapterConfig::Bedrock(_) => {
            return Err(AgentError::Permanent(
                "Use Bedrock foundation model discovery for Amazon Bedrock".to_string(),
            ));
        }
    };

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(request_timeout.min(std::time::Duration::from_secs(30)))
        .build()
        .map_err(|error| {
            AgentError::Failed(format!("failed to build model list client: {error}"))
        })?;
    let mut request = apply_auth(client.get(url), &config.auth, &config.provider_label)?;
    if let Some(version) = anthropic_version {
        request = request.header("anthropic-version", version);
    }
    let response = request.send().await.map_err(|error| {
        AgentError::Transient(format!(
            "{} model discovery request failed: {error}",
            config.provider_label
        ))
    })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        AgentError::Transient(format!(
            "{} model discovery response failed: {error}",
            config.provider_label
        ))
    })?;
    if !status.is_success() {
        return Err(classify_http_status(
            status.as_u16(),
            &body,
            &format!("{} model discovery", config.provider_label),
        ));
    }

    let payload: ModelList = serde_json::from_str(&body).map_err(|error| {
        AgentError::Failed(format!(
            "{} returned an invalid model list: {error}",
            config.provider_label
        ))
    })?;
    let mut models = payload
        .data
        .into_iter()
        .map(|item| item.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    models.sort_unstable();
    models.dedup();
    Ok(models)
}

fn openai_models_url(config: &OpenAiCompatibleConfig) -> String {
    format!(
        "{}/models",
        crate::rig_adapter::openai_api_base_url(config).trim_end_matches('/')
    )
}

fn anthropic_models_url(config: &AnthropicConfig) -> String {
    let base = config.base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

fn apply_auth(
    request: RequestBuilder,
    auth: &AuthConfig,
    provider_label: &str,
) -> Result<RequestBuilder, AgentError> {
    if matches!(auth, AuthConfig::AwsCredentials { .. }) {
        return Err(AgentError::Failed(
            "AWS credentials cannot authenticate an HTTP model list".to_string(),
        ));
    }
    let (key, headers) = auth.http_auth(provider_label)?;
    let request = request.headers(headers);
    Ok(if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    })
}
