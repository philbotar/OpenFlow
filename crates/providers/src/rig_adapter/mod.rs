//! rig adapter: request/response/error translation and provider dispatch.

mod anthropic_http;
mod claude_thinking;
mod convert;
mod error;
mod model;
mod openai_http;
mod outcome;
mod reasoning_convert;
mod stream;

use crate::client::{AiClientConfig, ProviderAdapterConfig};
use crate::CodexOAuthCredentials;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiStreamSink};
use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, SystemTime};

pub(crate) fn build_codex_model(
    provider_label: &str,
    base_url: &str,
    model_name: &str,
    credentials: &CodexOAuthCredentials,
    http: reqwest::Client,
) -> Result<model::RigModel, AgentError> {
    model::build_codex(provider_label, base_url, model_name, credentials, http)
}

pub(crate) async fn invoke_codex_model(
    model: &model::RigModel,
    request: &AgentRequest,
    provider_label: &str,
    provider_id: &crate::ProviderId,
) -> Result<AgentTurnOutcome, AgentError> {
    model
        .invoke(request, provider_label, provider_id, None)
        .await
}

pub(crate) async fn invoke_codex_model_stream(
    model: &model::RigModel,
    request: &AgentRequest,
    sink: &dyn AiStreamSink,
    provider_label: &str,
    provider_id: &crate::ProviderId,
) -> Result<AgentTurnOutcome, AgentError> {
    model
        .invoke_stream(request, sink, provider_label, provider_id, None)
        .await
}

pub(crate) fn is_codex_unauthorized(error: &AgentError, provider_label: &str) -> bool {
    error::is_unauthorized(error, provider_label)
}

/// Rebuild a cached model this long before its credentials actually expire so
/// an in-flight request never straddles the expiry.
const CREDENTIAL_EXPIRY_MARGIN: Duration = Duration::from_mins(2);

/// Per-client cache of built provider models, keyed by model id.
///
/// Building a model constructs a fresh HTTP client (new connection pool, so a
/// new TLS handshake on first use) and, for Bedrock, re-resolves AWS
/// credentials — potentially via an `aws` CLI subprocess costing seconds.
/// Caching makes those costs once-per-run instead of once-per-turn.
#[derive(Default)]
pub struct ModelCache {
    entries: tokio::sync::Mutex<HashMap<String, model::BuiltModel>>,
}

impl std::fmt::Debug for ModelCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCache").finish_non_exhaustive()
    }
}

impl ModelCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    async fn get_or_build(
        &self,
        config: &AiClientConfig,
        model_name: &str,
    ) -> Result<model::RigModel, AgentError> {
        self.get_or_build_with(model_name, || model::build_model(config, model_name))
            .await
    }

    /// Cache hit returns immediately; on miss the lock is released before
    /// building so other models can proceed and we don't hold a mutex guard
    /// across `.await`.
    async fn get_or_build_with<F, Fut>(
        &self,
        key: &str,
        build: F,
    ) -> Result<model::RigModel, AgentError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<model::BuiltModel, AgentError>>,
    {
        if let Some(cached) = {
            let entries = self.entries.lock().await;
            entries.get(key).and_then(|entry| {
                let usable = entry
                    .expires_at
                    .is_none_or(|at| SystemTime::now() + CREDENTIAL_EXPIRY_MARGIN < at);
                usable.then(|| entry.model.clone())
            })
        } {
            return Ok(cached);
        }

        let assembled = build().await?;
        let model = assembled.model.clone();
        {
            let mut entries = self.entries.lock().await;
            if let Some(entry) = entries.get(key) {
                let usable = entry
                    .expires_at
                    .is_none_or(|at| SystemTime::now() + CREDENTIAL_EXPIRY_MARGIN < at);
                if usable {
                    return Ok(entry.model.clone());
                }
            }
            entries.insert(key.to_string(), assembled);
        }
        Ok(model)
    }
}

async fn dispatch_invoke(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let model = cache.get_or_build(config, &request.model).await?;
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
    cache: &ModelCache,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    let model = cache.get_or_build(config, &request.model).await?;
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
    cache: &ModelCache,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, cache, request).await
}

pub async fn invoke_anthropic_stream(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, cache, request, sink).await
}

pub async fn invoke_openai_compatible(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, cache, request).await
}

pub async fn invoke_openai_compatible_stream(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, cache, request, sink).await
}

#[cfg(feature = "bedrock")]
pub async fn invoke_bedrock(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke(config, cache, request).await
}

#[cfg(feature = "bedrock")]
pub async fn invoke_bedrock_stream(
    config: &AiClientConfig,
    cache: &ModelCache,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    dispatch_invoke_stream(config, cache, request, sink).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthConfig;
    use crate::client::OpenAiCompatibleConfig;
    use crate::spec::ProviderId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn openai_test_config() -> AiClientConfig {
        AiClientConfig {
            provider_id: ProviderId::from("custom_openai_compatible"),
            provider_label: "test".to_string(),
            auth: AuthConfig::Bearer {
                api_key: Some("test-key".to_string()),
                required: true,
            },
            adapter: ProviderAdapterConfig::OpenAiCompatible(
                OpenAiCompatibleConfig::openai_default(),
            ),
        }
    }

    async fn built_model(expires_at: Option<SystemTime>) -> Result<model::BuiltModel, AgentError> {
        let mut built = model::build_model(&openai_test_config(), "test-model").await?;
        built.expires_at = expires_at;
        Ok(built)
    }

    #[tokio::test]
    async fn cache_builds_model_once_for_same_key() {
        let cache = ModelCache::new();
        let builds = AtomicUsize::new(0);
        for _ in 0..3 {
            let result = cache
                .get_or_build_with("m1", || {
                    builds.fetch_add(1, Ordering::SeqCst);
                    built_model(None)
                })
                .await;
            assert!(result.is_ok());
        }
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cache_builds_per_model_key() {
        let cache = ModelCache::new();
        let builds = AtomicUsize::new(0);
        for key in ["m1", "m2", "m1"] {
            let result = cache
                .get_or_build_with(key, || {
                    builds.fetch_add(1, Ordering::SeqCst);
                    built_model(None)
                })
                .await;
            assert!(result.is_ok());
        }
        assert_eq!(builds.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cache_rebuilds_when_credentials_near_expiry() {
        let cache = ModelCache::new();
        let builds = AtomicUsize::new(0);
        // Expires inside the safety margin -> both calls must build.
        let soon = SystemTime::now() + Duration::from_secs(30);
        for _ in 0..2 {
            let result = cache
                .get_or_build_with("m1", || {
                    builds.fetch_add(1, Ordering::SeqCst);
                    built_model(Some(soon))
                })
                .await;
            assert!(result.is_ok());
        }
        assert_eq!(builds.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cache_keeps_model_with_distant_expiry() {
        let cache = ModelCache::new();
        let builds = AtomicUsize::new(0);
        let distant = SystemTime::now() + Duration::from_hours(1);
        for _ in 0..2 {
            let result = cache
                .get_or_build_with("m1", || {
                    builds.fetch_add(1, Ordering::SeqCst);
                    built_model(Some(distant))
                })
                .await;
            assert!(result.is_ok());
        }
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cache_does_not_store_failed_builds() {
        let cache = ModelCache::new();
        let result = cache
            .get_or_build_with("m1", || async {
                Err::<model::BuiltModel, _>(AgentError::Transient("boom".to_string()))
            })
            .await;
        assert!(result.is_err());
        let builds = AtomicUsize::new(0);
        let retry = cache
            .get_or_build_with("m1", || {
                builds.fetch_add(1, Ordering::SeqCst);
                built_model(None)
            })
            .await;
        assert!(retry.is_ok());
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }
}
