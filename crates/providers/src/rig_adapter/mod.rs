//! rig adapter: request/response/error translation and provider dispatch.

mod convert;
mod error;
mod model;
mod outcome;
mod stream;

use crate::client::{AiClientConfig, ProviderAdapterConfig};
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiStreamSink};

async fn dispatch_invoke(
    config: &AiClientConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let model = model::build_model(config, &request.model).await?;
    let openai_config = match &config.adapter {
        ProviderAdapterConfig::OpenAiCompatible(cfg) => Some(cfg),
        _ => None,
    };
    model
        .invoke(
            &request,
            &config.provider_label,
            &config.provider_id,
            openai_config,
        )
        .await
}

async fn dispatch_invoke_stream(
    config: &AiClientConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    let model = model::build_model(config, &request.model).await?;
    let openai_config = match &config.adapter {
        ProviderAdapterConfig::OpenAiCompatible(cfg) => Some(cfg),
        _ => None,
    };
    model
        .invoke_stream(
            &request,
            sink,
            &config.provider_label,
            &config.provider_id,
            openai_config,
        )
        .await
}

pub async fn invoke_anthropic(
    config: &AiClientConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, request).await
}

pub async fn invoke_anthropic_stream(
    config: &AiClientConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, request, sink).await
}

pub async fn invoke_openai_compatible(
    config: &AiClientConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, request).await
}

pub async fn invoke_openai_compatible_stream(
    config: &AiClientConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, request, sink).await
}

#[cfg(feature = "bedrock")]
pub async fn invoke_bedrock(
    config: &AiClientConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, request).await
}

#[cfg(feature = "bedrock")]
pub async fn invoke_bedrock_stream(
    config: &AiClientConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, request, sink).await
}
