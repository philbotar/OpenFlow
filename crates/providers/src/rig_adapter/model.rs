//! Enum dispatch over rig provider models (`CompletionModel` is not dyn-safe).

use std::time::Duration;

use crate::auth::{AuthConfig, CodexOAuthCredentials};
#[cfg(feature = "bedrock")]
use crate::client::BedrockConfig;
use crate::client::OpenAiCompatibleConfig;
use crate::client::{AiClientConfig, AnthropicConfig, ProviderAdapterConfig};
use crate::prompt_cache::{cache_session_key, openai_compat_cache_key_enabled};
use crate::rig_adapter::{
    anthropic_http::AnthropicHttpClient, claude_thinking, convert, error,
    openai_http::OpenAiHttpClient, outcome, stream,
};
use crate::spec::{ModelTransport, ProviderId, WireApi};
use engine::{
    emit_assistant_deltas_from_outcome, AgentError, AgentRequest, AgentTurnOutcome, AiStreamSink,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client as ReqwestClient;
use rig_core::client::CompletionClient;
use rig_core::completion::{CompletionModel, CompletionRequest};
use rig_core::message::{AssistantContent, ToolChoice};
use rig_core::providers::anthropic::{
    self, completion::CompletionModel as AnthropicCompletionModel,
};
use rig_core::providers::chatgpt::ResponsesCompletionModel as ChatGPTResponsesCompletionModel;
use rig_core::providers::chatgpt::{self, ChatGPTAuth};
use rig_core::providers::openai;
use rig_core::providers::openai::completion::CompletionModel as OpenAiChatModel;
use rig_core::providers::openai::responses_api::ResponsesCompletionModel;
use serde_json::{json, Value};

#[cfg(feature = "bedrock")]
use rig_bedrock::completion::CompletionModel as BedrockCompletionModel;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ANTHROPIC_MAX_TOKENS: u64 = 4096;

#[derive(Clone)]
pub enum RigModel {
    Anthropic(AnthropicCompletionModel<AnthropicHttpClient>),
    OpenAiChat(OpenAiChatModel<OpenAiHttpClient>),
    OpenAiResponses(ResponsesCompletionModel<OpenAiHttpClient>),
    ChatGPT(ChatGPTResponsesCompletionModel),
    #[cfg(feature = "bedrock")]
    Bedrock(BedrockCompletionModel),
}

/// A built provider model plus the moment its credentials stop being valid
/// (Bedrock session tokens only; `None` = usable for the process lifetime).
pub(super) struct BuiltModel {
    pub(super) model: RigModel,
    pub(super) expires_at: Option<std::time::SystemTime>,
}

impl BuiltModel {
    const fn without_expiry(model: RigModel) -> Self {
        Self {
            model,
            expires_at: None,
        }
    }
}

pub(super) async fn build_model(
    config: &AiClientConfig,
    model: &str,
) -> Result<BuiltModel, AgentError> {
    match &config.adapter {
        ProviderAdapterConfig::Anthropic(anthropic_config) => {
            build_anthropic(config, anthropic_config, model).map(BuiltModel::without_expiry)
        }
        ProviderAdapterConfig::OpenAiCompatible(openai_config) => {
            match openai_config.transport_for_model(model) {
                ModelTransport::Responses => {
                    build_openai(config, openai_config, model, WireApi::Responses)
                }
                ModelTransport::ChatCompletions => {
                    build_openai(config, openai_config, model, WireApi::ChatCompletions)
                }
                ModelTransport::AnthropicMessages => {
                    build_compatible_anthropic(config, openai_config, model)
                }
            }
            .map(BuiltModel::without_expiry)
        }
        ProviderAdapterConfig::OpenAiCodex(_) => Err(AgentError::Failed(
            "OpenAI Codex inference is not wired yet".into(),
        )),
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

fn build_compatible_anthropic(
    config: &AiClientConfig,
    openai_config: &OpenAiCompatibleConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let anthropic_config = AnthropicConfig {
        base_url: openai_config.base_url.clone(),
        messages_path: "v1/messages".to_string(),
        anthropic_version: "2023-06-01".to_string(),
        request_timeout: openai_config.request_timeout,
    };
    build_anthropic(config, &anthropic_config, model)
}

fn build_anthropic(
    config: &AiClientConfig,
    anthropic_config: &AnthropicConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let (api_key, headers) = rig_auth(config)?;
    let http = AnthropicHttpClient::new(
        rig_http_client(anthropic_config.request_timeout),
        config.debug_output,
        config.provider_label.clone(),
    );
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
    wire_api: WireApi,
) -> Result<RigModel, AgentError> {
    let (api_key, headers) = rig_auth(config)?;
    let http = OpenAiHttpClient::new(
        rig_http_client(openai_config.request_timeout),
        config.debug_output,
        config.provider_label.clone(),
    );
    let base_url = rig_openai_base_url(openai_config, wire_api);
    match wire_api {
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

pub(super) fn build_codex(
    provider_label: &str,
    base_url: &str,
    model: &str,
    credentials: &CodexOAuthCredentials,
    http: ReqwestClient,
) -> Result<RigModel, AgentError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("openai-beta"),
        HeaderValue::from_static("responses=experimental"),
    );
    let client = chatgpt::Client::builder()
        .api_key(ChatGPTAuth::AccessToken {
            access_token: credentials.access_token.clone(),
            account_id: Some(credentials.account_id.clone()),
        })
        .base_url(base_url)
        .default_instructions("")
        .originator("openflow")
        .http_headers(headers)
        .http_client(http)
        .build()
        .map_err(|error| {
            AgentError::Failed(format!(
                "failed to build {provider_label} ChatGPT client: {error}"
            ))
        })?;
    Ok(RigModel::ChatGPT(client.completion_model(model)))
}

#[cfg(feature = "bedrock")]
async fn build_bedrock(
    _config: &AiClientConfig,
    bedrock_config: &BedrockConfig,
    model: &str,
) -> Result<BuiltModel, AgentError> {
    let sdk_config = crate::aws_runtime::load_aws_sdk_config(
        &bedrock_config.region,
        bedrock_config.aws_profile.as_deref(),
        bedrock_config.aws_credential_command.as_deref(),
    )
    .await;
    let expires_at = bedrock_credentials_expiry(&sdk_config).await;
    let sdk_client = aws_sdk_bedrockruntime::Client::new(&sdk_config);
    let client: rig_bedrock::client::Client = sdk_client.into();
    let mut completion_model = client.completion_model(model);
    if bedrock_prompt_caching_supported(model) {
        completion_model = completion_model.with_prompt_caching();
    }
    Ok(BuiltModel {
        model: RigModel::Bedrock(completion_model),
        expires_at,
    })
}

/// Expiry of the resolved credentials, if they carry one. Providers from the
/// default chain cache internally, so this probe is cheap after
/// `load_aws_sdk_config` already resolved credentials once.
#[cfg(feature = "bedrock")]
async fn bedrock_credentials_expiry(
    sdk_config: &aws_config::SdkConfig,
) -> Option<std::time::SystemTime> {
    use aws_sdk_bedrockruntime::config::ProvideCredentials;
    let provider = sdk_config.credentials_provider()?;
    provider.provide_credentials().await.ok()?.expiry()
}

/// Whether a Bedrock model id supports Converse prompt caching. Bedrock
/// rejects `cachePoint` blocks on unsupported models with a
/// `ValidationException`, so this must stay an explicit allowlist.
/// Matches with or without inference-profile prefixes (`us.`, `apac.`, …).
#[cfg(feature = "bedrock")]
fn bedrock_prompt_caching_supported(model_id: &str) -> bool {
    const SUPPORTED: [&str; 10] = [
        "anthropic.claude-3-5-haiku-",
        "anthropic.claude-3-5-sonnet-20241022-v2",
        "anthropic.claude-3-7-sonnet-",
        "anthropic.claude-sonnet-4-",
        "anthropic.claude-opus-4-",
        "anthropic.claude-haiku-4-",
        "amazon.nova-micro-",
        "amazon.nova-lite-",
        "amazon.nova-pro-",
        "amazon.nova-premier-",
    ];
    SUPPORTED.iter().any(|prefix| {
        model_id
            .find(prefix)
            .is_some_and(|at| at == 0 || model_id.as_bytes().get(at - 1) == Some(&b'.'))
    })
}

impl RigModel {
    const fn openai_wire_api(&self) -> Option<WireApi> {
        match self {
            Self::OpenAiChat(_) => Some(WireApi::ChatCompletions),
            Self::OpenAiResponses(_) => Some(WireApi::Responses),
            Self::Anthropic(_) | Self::ChatGPT(_) => None,
            #[cfg(feature = "bedrock")]
            Self::Bedrock(_) => None,
        }
    }

    pub async fn invoke(
        &self,
        request: &AgentRequest,
        provider_label: &str,
        provider_id: &ProviderId,
        openai_config: Option<&OpenAiCompatibleConfig>,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let no_tool_calls = outcome::no_tool_calls_policy(request, self.openai_wire_api());
        let model_name = request.model.clone();
        let mut completion_request =
            completion_request_for(self, request, provider_id, openai_config)?;
        let result = match self {
            Self::Anthropic(model) => {
                if completion_request.max_tokens.is_none() {
                    completion_request.max_tokens = Some(ANTHROPIC_MAX_TOKENS);
                }
                match model.completion(completion_request).await {
                    Err(e) => Err(error::to_agent_error(e, provider_label)),
                    Ok(response) => {
                        let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                        outcome::resolve_outcome(
                            choice,
                            response.usage,
                            provider_label,
                            Some(&request.output_schema),
                            no_tool_calls,
                        )
                    }
                }
            }
            Self::OpenAiChat(model) => match model.completion(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(response) => {
                    let finish_reason = response
                        .raw_response
                        .choices
                        .first()
                        .map(|choice| choice.finish_reason.clone());
                    let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                    let diagnostics =
                        outcome::response_diagnostics(&choice, &response.usage, finish_reason);
                    outcome::resolve_outcome(
                        choice,
                        response.usage,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                    .map_err(|error| {
                        outcome::enrich_empty_turn_error_with_response(
                            error,
                            provider_label,
                            &model_name,
                            Some(&diagnostics),
                        )
                    })
                }
            },
            Self::OpenAiResponses(model) => match model.completion(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(response) => {
                    let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                    outcome::resolve_outcome(
                        choice,
                        response.usage,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                }
            },
            Self::ChatGPT(model) => match model.completion(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(response) => {
                    let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                    outcome::resolve_outcome(
                        choice,
                        response.usage,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                }
            },
            #[cfg(feature = "bedrock")]
            Self::Bedrock(model) => match model.completion(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(response) => {
                    let choice: Vec<AssistantContent> = response.choice.into_iter().collect();
                    outcome::resolve_outcome(
                        choice,
                        response.usage,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                }
            },
        };
        result.map_err(|error| outcome::enrich_empty_turn_error(error, provider_label, &model_name))
    }

    pub async fn invoke_stream(
        &self,
        request: &AgentRequest,
        sink: &dyn AiStreamSink,
        provider_label: &str,
        provider_id: &ProviderId,
        openai_config: Option<&OpenAiCompatibleConfig>,
    ) -> Result<AgentTurnOutcome, AgentError> {
        // Rig's chat-completions stream summary drops the provider finish reason.
        // Keep custom-compatible turns non-streaming so a failed structured-output
        // turn retains the metadata needed to distinguish truncation from an empty
        // or reasoning-only response. The fallback emitter publishes displayable
        // reasoning followed by final assistant text through the normal sink.
        if provider_id.as_str() == "custom_openai_compatible" {
            let outcome = self
                .invoke(request, provider_label, provider_id, openai_config)
                .await?;
            emit_assistant_deltas_from_outcome(sink, &outcome);
            return Ok(outcome);
        }
        let no_tool_calls = outcome::no_tool_calls_policy(request, self.openai_wire_api());
        let model_name = request.model.clone();
        let mut completion_request =
            completion_request_for(self, request, provider_id, openai_config)?;
        let result = match self {
            Self::Anthropic(model) => {
                if completion_request.max_tokens.is_none() {
                    completion_request.max_tokens = Some(ANTHROPIC_MAX_TOKENS);
                }
                match model.stream(completion_request).await {
                    Err(e) => Err(error::to_agent_error(e, provider_label)),
                    Ok(rig_stream) => {
                        stream::drain(
                            rig_stream,
                            sink,
                            provider_label,
                            Some(&request.output_schema),
                            no_tool_calls,
                        )
                        .await
                    }
                }
            }
            Self::OpenAiChat(model) => match model.stream(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(rig_stream) => {
                    stream::drain(
                        rig_stream,
                        sink,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                    .await
                }
            },
            Self::OpenAiResponses(model) => match model.stream(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(rig_stream) => {
                    stream::drain(
                        rig_stream,
                        sink,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                    .await
                }
            },
            Self::ChatGPT(model) => match model.stream(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(rig_stream) => {
                    stream::drain(
                        rig_stream,
                        sink,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                    .await
                }
            },
            #[cfg(feature = "bedrock")]
            Self::Bedrock(model) => match model.stream(completion_request).await {
                Err(e) => Err(error::to_agent_error(e, provider_label)),
                Ok(rig_stream) => {
                    stream::drain(
                        rig_stream,
                        sink,
                        provider_label,
                        Some(&request.output_schema),
                        no_tool_calls,
                    )
                    .await
                }
            },
        };
        result.map_err(|error| outcome::enrich_empty_turn_error(error, provider_label, &model_name))
    }
}

fn completion_request_for(
    model: &RigModel,
    request: &AgentRequest,
    provider_id: &ProviderId,
    openai_config: Option<&OpenAiCompatibleConfig>,
) -> Result<CompletionRequest, AgentError> {
    let mut completion_request = convert::to_completion_request(request);
    apply_tool_choice_policy(&mut completion_request, provider_id);
    match model {
        RigModel::Anthropic(_) => {
            claude_thinking::apply(
                claude_thinking::ClaudePlatform::Anthropic,
                &mut completion_request,
                request,
            )?;
            apply_anthropic_cache_params(&mut completion_request);
        }
        RigModel::OpenAiChat(_) | RigModel::OpenAiResponses(_) => {
            if let Some(config) = openai_config {
                merge_openai_params(
                    &mut completion_request,
                    request,
                    provider_id,
                    model.openai_wire_api().unwrap_or(config.wire_api),
                );
            }
        }
        RigModel::ChatGPT(_) => merge_codex_params(&mut completion_request, request),
        #[cfg(feature = "bedrock")]
        RigModel::Bedrock(_) => {
            claude_thinking::apply(
                claude_thinking::ClaudePlatform::Bedrock,
                &mut completion_request,
                request,
            )?;
        }
    }
    Ok(completion_request)
}

fn merge_codex_params(request: &mut CompletionRequest, agent_request: &AgentRequest) {
    let Some(effort) = agent_request.reasoning_effort.as_deref() else {
        return;
    };
    let reasoning = json!({"effort": effort, "summary": "auto"});
    request.additional_params = Some(match request.additional_params.take() {
        Some(Value::Object(mut existing)) => {
            existing.insert("reasoning".into(), reasoning);
            Value::Object(existing)
        }
        Some(other) => json!({"reasoning": reasoning, "_rig_merge": other}),
        None => json!({"reasoning": reasoning}),
    });
}

fn apply_tool_choice_policy(request: &mut CompletionRequest, provider_id: &ProviderId) {
    let _ = provider_id;
    request.tool_choice = Some(ToolChoice::Required);
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

fn rig_http_client(request_timeout: Duration) -> ReqwestClient {
    ReqwestClient::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(request_timeout)
        .build()
        .unwrap_or_else(|_| ReqwestClient::new())
}

fn rig_openai_base_url(config: &OpenAiCompatibleConfig, wire_api: WireApi) -> String {
    let path = match wire_api {
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

    let base = base_url.trim_end_matches('/');
    let mut path = path.trim_start_matches('/');
    // Specs often set both base `.../v1` and path `v1/chat/completions`. After
    // stripping the endpoint suffix that becomes `.../v1` + `v1` → double v1.
    if base.ends_with("/v1") {
        if path == "v1" {
            return base.to_string();
        }
        if let Some(rest) = path.strip_prefix("v1/") {
            path = rest;
        }
    }
    if path.is_empty() {
        return base.to_string();
    }
    format!("{base}/{path}")
}

fn openai_build_error(label: &str, error: impl std::fmt::Display) -> AgentError {
    AgentError::Failed(format!("failed to build {label} OpenAI client: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthConfig;
    use crate::rig_adapter::convert;
    use engine::{AgentRequest, NodeId, NodeToolConfig, WorkflowId};
    use rig_core::message::ToolChoice;
    use std::collections::BTreeMap;

    fn minimal_request() -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId::from("wf"),
            node_id: NodeId::from("n1"),
            node_label: "Idea".into(),
            model: "mimo-v2.5".into(),
            system_messages: vec!["sys".into()],
            task_prompt: "task".into(),
            input: serde_json::json!({}),
            output_schema: serde_json::json!({"type":"object"}),
            tool_config: NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: false,
        }
    }

    fn custom_compatible_config() -> AiClientConfig {
        custom_compatible_config_with_transports(BTreeMap::new())
    }

    fn custom_compatible_config_with_transports(
        model_transports: BTreeMap<String, ModelTransport>,
    ) -> AiClientConfig {
        AiClientConfig {
            provider_id: ProviderId::from("custom_openai_compatible"),
            provider_label: "Custom OpenAI-compatible API".into(),
            auth: AuthConfig::Bearer {
                api_key: Some("test-key".into()),
                required: true,
            },
            adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
                base_url: "https://api.example.test/v1".into(),
                wire_api: WireApi::ChatCompletions,
                responses_path: "v1/responses".into(),
                chat_completions_path: "v1/chat/completions".into(),
                model_transports,
                request_timeout: Duration::from_mins(5),
            }),
            debug_output: false,
        }
    }

    #[tokio::test]
    async fn model_transport_override_uses_anthropic_messages() {
        let config = custom_compatible_config_with_transports(BTreeMap::from([(
            "vendor-model".to_string(),
            ModelTransport::AnthropicMessages,
        )]));
        assert!(matches!(
            build_model(&config, "vendor-model").await,
            Ok(BuiltModel {
                model: RigModel::Anthropic(_),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn model_without_override_uses_provider_default() {
        assert!(matches!(
            build_model(&custom_compatible_config(), "vendor-model").await,
            Ok(BuiltModel {
                model: RigModel::OpenAiChat(_),
                ..
            })
        ));
    }

    #[cfg(feature = "bedrock")]
    #[test]
    fn bedrock_prompt_caching_allowlist_matches_supported_models() {
        let supported = [
            "anthropic.claude-3-5-haiku-20241022-v1:0",
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
            "anthropic.claude-3-7-sonnet-20250219-v1:0",
            "anthropic.claude-sonnet-4-20250514-v1:0",
            "us.anthropic.claude-opus-4-20250514-v1:0",
            "apac.anthropic.claude-sonnet-4-20250514-v1:0",
            "amazon.nova-pro-v1:0",
            "us.amazon.nova-premier-v1:0",
        ];
        for model in supported {
            assert!(bedrock_prompt_caching_supported(model), "{model}");
        }
    }

    #[cfg(feature = "bedrock")]
    #[test]
    fn bedrock_prompt_caching_rejects_unsupported_models() {
        // Bedrock throws ValidationException on cachePoint for these.
        let unsupported = [
            "anthropic.claude-3-5-sonnet-20240620-v1:0",
            "anthropic.claude-3-haiku-20240307-v1:0",
            "anthropic.claude-3-opus-20240229-v1:0",
            "anthropic.claude-v2:0",
            "deepseek.v3-v1:0",
            "amazon.titan-text-express-v1",
            "meta.llama3-70b-instruct-v1:0",
            "notanthropic.claude-sonnet-4-20250514-v1:0",
        ];
        for model in unsupported {
            assert!(!bedrock_prompt_caching_supported(model), "{model}");
        }
    }

    #[cfg(feature = "bedrock")]
    #[test]
    fn bedrock_claude_thinking_uses_automatic_tool_choice() {
        let model_id = "anthropic.claude-sonnet-4-20250514-v1:0";
        let client = rig_bedrock::client::Client::with_profile_name("unused-test-profile");
        let model = RigModel::Bedrock(client.completion_model(model_id));
        let mut agent_request = minimal_request();
        agent_request.model = model_id.into();
        agent_request.reasoning_effort = Some("low".into());

        let result =
            completion_request_for(&model, &agent_request, &ProviderId::from("bedrock"), None);

        assert!(result.is_ok_and(|request| {
            request.tool_choice == Some(ToolChoice::Auto)
                && request.additional_params.as_ref().is_some_and(|params| {
                    params["thinking"]["type"] == "enabled"
                        && params["thinking"]["budget_tokens"] == 10_240
                })
        }));
    }

    #[cfg(feature = "bedrock")]
    #[test]
    fn bedrock_claude_without_thinking_requires_tool_use() {
        let model_id = "anthropic.claude-sonnet-4-20250514-v1:0";
        let client = rig_bedrock::client::Client::with_profile_name("unused-test-profile");
        let model = RigModel::Bedrock(client.completion_model(model_id));
        let mut agent_request = minimal_request();
        agent_request.model = model_id.into();

        let result =
            completion_request_for(&model, &agent_request, &ProviderId::from("bedrock"), None);

        assert!(result.is_ok_and(|request| {
            request.tool_choice == Some(ToolChoice::Required)
                && request
                    .additional_params
                    .as_ref()
                    .is_none_or(|params| params.get("thinking").is_none())
        }));
    }

    #[test]
    fn custom_openai_compatible_requires_a_tool_call() {
        let mut request = convert::to_completion_request(&minimal_request());
        apply_tool_choice_policy(&mut request, &ProviderId::from("custom_openai_compatible"));
        assert_eq!(request.tool_choice, Some(ToolChoice::Required));
    }

    #[test]
    fn rig_openai_base_url_joins_relative_api_prefix() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.example.test".into(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
            model_transports: BTreeMap::new(),
            request_timeout: Duration::from_mins(5),
        };
        assert_eq!(
            rig_openai_base_url(&config, WireApi::ChatCompletions),
            "https://api.example.test/v1"
        );
    }

    #[test]
    fn rig_openai_base_url_keeps_absolute_path_host() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.example.test".into(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "https://other.example.test/v1/chat/completions".into(),
            model_transports: BTreeMap::new(),
            request_timeout: Duration::from_mins(5),
        };
        assert_eq!(
            rig_openai_base_url(&config, WireApi::ChatCompletions),
            "https://other.example.test/v1"
        );
    }

    #[test]
    fn rig_openai_base_url_dedupes_v1_when_base_already_has_it() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.x.ai/v1".into(),
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".into(),
            chat_completions_path: "v1/chat/completions".into(),
            model_transports: BTreeMap::new(),
            request_timeout: Duration::from_mins(5),
        };
        assert_eq!(
            rig_openai_base_url(&config, WireApi::ChatCompletions),
            "https://api.x.ai/v1"
        );
    }

    #[test]
    fn builtin_openai_compat_specs_never_double_v1() {
        use crate::spec::{builtin_provider_specs, ProviderKind};

        for spec in builtin_provider_specs() {
            let ProviderKind::OpenAiCompatible(oa) = spec.kind else {
                continue;
            };
            if spec.default_base_url.is_empty() {
                continue;
            }
            let config = OpenAiCompatibleConfig {
                base_url: spec.default_base_url.into(),
                wire_api: oa.default_wire_api,
                responses_path: oa.responses_path.into(),
                chat_completions_path: oa.chat_completions_path.into(),
                model_transports: BTreeMap::new(),
                request_timeout: Duration::from_mins(5),
            };
            let url = rig_openai_base_url(&config, oa.default_wire_api);
            assert!(
                !url.contains("/v1/v1"),
                "{}: {url} (base={}, wire={:?})",
                spec.id,
                spec.default_base_url,
                oa.default_wire_api
            );
        }
    }
}
