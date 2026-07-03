//! Enum dispatch over rig provider models (`CompletionModel` is not dyn-safe).

use std::time::Duration;

use crate::auth::AuthConfig;
use crate::client::OpenAiCompatibleConfig;
use crate::client::{AiClientConfig, AnthropicConfig, BedrockConfig, ProviderAdapterConfig};
use crate::prompt_cache::{cache_session_key, openai_compat_cache_key_enabled};
use crate::rig_adapter::{convert, error, outcome, stream};
use crate::spec::{ProviderId, WireApi};
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiStreamSink};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client as ReqwestClient;
use rig_core::client::CompletionClient;
use rig_core::completion::{CompletionModel, CompletionRequest};
use rig_core::message::AssistantContent;
use rig_core::providers::anthropic::{
    self, completion::CompletionModel as AnthropicCompletionModel,
};
use rig_core::providers::openai;
use rig_core::providers::openai::completion::CompletionModel as OpenAiChatModel;
use rig_core::providers::openai::responses_api::ResponsesCompletionModel;
use serde_json::{json, Value};

#[cfg(feature = "bedrock")]
use rig_bedrock::completion::CompletionModel as BedrockCompletionModel;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_mins(2);
const ANTHROPIC_MAX_TOKENS: u64 = 4096;

pub enum RigModel {
    Anthropic(AnthropicCompletionModel<ReqwestClient>),
    OpenAiChat(OpenAiChatModel<ReqwestClient>),
    OpenAiResponses(ResponsesCompletionModel<ReqwestClient>),
    #[cfg(feature = "bedrock")]
    Bedrock(BedrockCompletionModel),
}

pub async fn build_model(config: &AiClientConfig, model: &str) -> Result<RigModel, AgentError> {
    match &config.adapter {
        ProviderAdapterConfig::Anthropic(anthropic_config) => {
            build_anthropic(config, anthropic_config, model)
        }
        ProviderAdapterConfig::OpenAiCompatible(openai_config) => {
            build_openai(config, openai_config, model)
        }
        #[cfg(feature = "bedrock")]
        ProviderAdapterConfig::Bedrock(bedrock_config) => {
            build_bedrock(config, bedrock_config, model).await
        }
        #[cfg(not(feature = "bedrock"))]
        ProviderAdapterConfig::Bedrock(_) => Err(AgentError::Failed(
            "Bedrock support is disabled (enable the `bedrock` feature)".into(),
        )),
    }
}

fn build_anthropic(
    config: &AiClientConfig,
    anthropic_config: &AnthropicConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let (api_key, headers) = rig_auth(config)?;
    let http = rig_http_client();
    let client = anthropic::Client::builder()
        .api_key(api_key.as_str())
        .base_url(&anthropic_config.base_url)
        .anthropic_version(&anthropic_config.anthropic_version)
        .http_client(http)
        .http_headers(headers)
        .build()
        .map_err(|e| {
            AgentError::Failed(format!(
                "failed to build {} Anthropic client: {e}",
                config.provider_label
            ))
        })?;
    Ok(RigModel::Anthropic(client.completion_model(model)))
}

fn build_openai(
    config: &AiClientConfig,
    openai_config: &OpenAiCompatibleConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let (api_key, headers) = rig_auth(config)?;
    let http = rig_http_client();
    let base_url = rig_openai_base_url(openai_config);
    match openai_config.wire_api {
        WireApi::ChatCompletions => {
            let client = openai::CompletionsClient::builder()
                .api_key(api_key)
                .base_url(base_url)
                .http_client(http)
                .http_headers(headers)
                .build()
                .map_err(|e| openai_build_error(&config.provider_label, e))?;
            Ok(RigModel::OpenAiChat(client.completion_model(model)))
        }
        WireApi::Responses => {
            let client = openai::Client::builder()
                .api_key(api_key)
                .base_url(base_url)
                .http_client(http)
                .http_headers(headers)
                .build()
                .map_err(|e| openai_build_error(&config.provider_label, e))?;
            Ok(RigModel::OpenAiResponses(client.completion_model(model)))
        }
    }
}

#[cfg(feature = "bedrock")]
async fn build_bedrock(
    _config: &AiClientConfig,
    bedrock_config: &BedrockConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let sdk_config = crate::aws_runtime::load_aws_sdk_config(
        &bedrock_config.region,
        bedrock_config.aws_profile.as_deref(),
    )
    .await;
    let sdk_client = aws_sdk_bedrockruntime::Client::new(&sdk_config);
    let client: rig_bedrock::client::Client = sdk_client.into();
    Ok(RigModel::Bedrock(client.completion_model(model)))
}

impl RigModel {
    pub async fn invoke(
        &self,
        request: &AgentRequest,
        provider_label: &str,
        provider_id: &ProviderId,
        openai_config: Option<&OpenAiCompatibleConfig>,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let mut completion_request =
            completion_request_for(self, request, provider_id, openai_config);
        match self {
            Self::Anthropic(model) => {
                completion_request.max_tokens = Some(ANTHROPIC_MAX_TOKENS);
                let response = model
                    .completion(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                outcome::resolve_outcome(
                    choice,
                    response.usage,
                    provider_label,
                    Some(&request.output_schema),
                )
            }
            Self::OpenAiChat(model) => {
                let response = model
                    .completion(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                outcome::resolve_outcome(
                    choice,
                    response.usage,
                    provider_label,
                    Some(&request.output_schema),
                )
            }
            Self::OpenAiResponses(model) => {
                let response = model
                    .completion(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                outcome::resolve_outcome(
                    choice,
                    response.usage,
                    provider_label,
                    Some(&request.output_schema),
                )
            }
            #[cfg(feature = "bedrock")]
            Self::Bedrock(model) => {
                let response = model
                    .completion(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                outcome::resolve_outcome(
                    choice,
                    response.usage,
                    provider_label,
                    Some(&request.output_schema),
                )
            }
        }
    }

    pub async fn invoke_stream(
        &self,
        request: &AgentRequest,
        sink: &dyn AiStreamSink,
        provider_label: &str,
        provider_id: &ProviderId,
        openai_config: Option<&OpenAiCompatibleConfig>,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let mut completion_request =
            completion_request_for(self, request, provider_id, openai_config);
        match self {
            Self::Anthropic(model) => {
                completion_request.max_tokens = Some(ANTHROPIC_MAX_TOKENS);
                let rig_stream = model
                    .stream(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                stream::drain(
                    rig_stream,
                    sink,
                    provider_label,
                    Some(&request.output_schema),
                )
                .await
            }
            Self::OpenAiChat(model) => {
                let rig_stream = model
                    .stream(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                stream::drain(
                    rig_stream,
                    sink,
                    provider_label,
                    Some(&request.output_schema),
                )
                .await
            }
            Self::OpenAiResponses(model) => {
                let rig_stream = model
                    .stream(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                stream::drain(
                    rig_stream,
                    sink,
                    provider_label,
                    Some(&request.output_schema),
                )
                .await
            }
            #[cfg(feature = "bedrock")]
            Self::Bedrock(model) => {
                let rig_stream = model
                    .stream(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                stream::drain(
                    rig_stream,
                    sink,
                    provider_label,
                    Some(&request.output_schema),
                )
                .await
            }
        }
    }
}

fn completion_request_for(
    model: &RigModel,
    request: &AgentRequest,
    provider_id: &ProviderId,
    openai_config: Option<&OpenAiCompatibleConfig>,
) -> CompletionRequest {
    let mut completion_request = convert::to_completion_request(request);
    match model {
        RigModel::Anthropic(_) => apply_anthropic_cache_params(&mut completion_request),
        RigModel::OpenAiChat(_) | RigModel::OpenAiResponses(_) => {
            if let Some(config) = openai_config {
                merge_openai_params(
                    &mut completion_request,
                    request,
                    provider_id,
                    config.wire_api,
                );
            }
        }
        #[cfg(feature = "bedrock")]
        RigModel::Bedrock(_) => {}
    }
    completion_request
}

fn apply_anthropic_cache_params(request: &mut CompletionRequest) {
    let cache = json!({ "type": "ephemeral" });
    request.additional_params = Some(match request.additional_params.take() {
        Some(Value::Object(mut map)) => {
            map.entry("cache_control".to_string()).or_insert(cache);
            Value::Object(map)
        }
        Some(other) => json!({ "cache_control": cache, "_rig_merge": other }),
        None => json!({ "cache_control": cache }),
    });
}

fn merge_openai_params(
    request: &mut CompletionRequest,
    agent_request: &AgentRequest,
    provider_id: &ProviderId,
    _wire_api: WireApi,
) {
    let mut params = serde_json::Map::new();
    if openai_compat_cache_key_enabled(provider_id) {
        params.insert(
            "prompt_cache_key".into(),
            json!(cache_session_key(agent_request)),
        );
    }
    if let Some(effort) = &agent_request.reasoning_effort {
        params.insert("reasoning_effort".into(), json!(effort));
    }
    if let Some(budget) = agent_request.reasoning_budget_tokens {
        params.insert("reasoning_budget_tokens".into(), json!(budget));
        params.insert("reasoning".into(), json!({ "max_tokens": budget }));
    }
    if params.is_empty() {
        return;
    }
    request.additional_params = Some(match request.additional_params.take() {
        Some(Value::Object(mut existing)) => {
            existing.extend(params);
            Value::Object(existing)
        }
        Some(other) => {
            params.insert("_rig_merge".into(), other);
            Value::Object(params)
        }
        None => Value::Object(params),
    });
}

fn rig_auth(config: &AiClientConfig) -> Result<(String, HeaderMap), AgentError> {
    let label = config.provider_label.as_str();
    match &config.auth {
        AuthConfig::Bearer { api_key, required } => {
            let key = extract_api_key(api_key.as_ref(), *required, label)?;
            Ok((key.unwrap_or_default(), HeaderMap::new()))
        }
        AuthConfig::Header {
            name,
            api_key,
            required,
        } => {
            let key = extract_api_key(api_key.as_ref(), *required, label)?;
            let mut headers = HeaderMap::new();
            if let Some(key) = key {
                let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                    AgentError::Failed(format!("invalid auth header name `{name}`: {e}"))
                })?;
                let header_value = HeaderValue::from_str(&key).map_err(|e| {
                    AgentError::Failed(format!("invalid auth header value for `{name}`: {e}"))
                })?;
                headers.insert(header_name, header_value);
            }
            Ok((String::new(), headers))
        }
        AuthConfig::NoneAllowed => Ok((String::new(), HeaderMap::new())),
        AuthConfig::AwsCredentials { .. } => Err(AgentError::Failed(format!(
            "{label} HTTP auth is not configured for rig adapter"
        ))),
    }
}

fn extract_api_key(
    api_key: Option<&String>,
    required: bool,
    label: &str,
) -> Result<Option<String>, AgentError> {
    let key = api_key.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    if key.is_none() && required {
        return Err(AgentError::Permanent(format!("{label} API key missing")));
    }
    Ok(key)
}

fn rig_http_client() -> ReqwestClient {
    ReqwestClient::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .unwrap_or_else(|_| ReqwestClient::new())
}

fn rig_openai_base_url(config: &OpenAiCompatibleConfig) -> String {
    let path = match config.wire_api {
        WireApi::ChatCompletions => config.chat_completions_path.as_str(),
        WireApi::Responses => config.responses_path.as_str(),
    };
    if path.starts_with("http://") || path.starts_with("https://") {
        return strip_openai_api_suffix(path).map_or_else(|| path.to_string(), str::to_string);
    }
    join_base_url(
        &config.base_url,
        strip_openai_api_suffix(path).unwrap_or(path),
    )
}

fn strip_openai_api_suffix(path: &str) -> Option<&str> {
    path.strip_suffix("/chat/completions")
        .or_else(|| path.strip_suffix("/responses"))
        .or_else(|| path.strip_suffix("chat/completions"))
        .or_else(|| path.strip_suffix("responses"))
}

fn join_base_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        }
    )
}

fn openai_build_error(label: &str, error: impl std::fmt::Display) -> AgentError {
    AgentError::Failed(format!("failed to build {label} OpenAI client: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_openai_base_url_joins_relative_api_prefix() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.example.test".into(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
        };
        assert_eq!(rig_openai_base_url(&config), "https://api.example.test/v1");
    }

    #[test]
    fn rig_openai_base_url_keeps_absolute_path_host() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.example.test".into(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "https://other.example.test/v1/chat/completions".into(),
        };
        assert_eq!(
            rig_openai_base_url(&config),
            "https://other.example.test/v1"
        );
    }
}
