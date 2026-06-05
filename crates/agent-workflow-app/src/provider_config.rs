use crate::settings_store::{AiProviderKind, AppSettings, ProviderTransport};
use openai_client::OpenAiWireApi;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEnv {
    pub openai_api_key: Option<String>,
    pub compatible_api_key: Option<String>,
}

impl ProviderEnv {
    #[must_use]
    pub fn from_system() -> Self {
        Self {
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            compatible_api_key: std::env::var("OPENAI_COMPATIBLE_API_KEY").ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProviderConfig {
    pub provider: AiProviderKind,
    pub api_key: String,
    pub base_url: String,
    pub wire_api: OpenAiWireApi,
    pub responses_path: String,
    pub chat_completions_path: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderConfigError {
    #[error("{provider} API key missing (set it in Settings or {env_var})")]
    MissingApiKey {
        provider: &'static str,
        env_var: &'static str,
    },
}

/// # Errors
/// Returns an error if the API key cannot be resolved for the active provider.
pub fn resolve_provider_config(
    settings: &AppSettings,
    transient_api_key: Option<&str>,
    env: &ProviderEnv,
) -> Result<ResolvedProviderConfig, ProviderConfigError> {
    let profile = settings.active_profile();
    let env_key_name = settings.active_provider.env_key();
    let env_key_value = match settings.active_provider {
        AiProviderKind::OpenAi => env.openai_api_key.as_deref(),
        AiProviderKind::OpenAiCompatible => env.compatible_api_key.as_deref(),
    };
    let stored_key = profile.api_key.trim();
    let api_key = transient_api_key
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            if stored_key.is_empty() {
                None
            } else {
                Some(stored_key.to_string())
            }
        })
        .or_else(|| {
            env_key_value
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
        })
        .ok_or_else(|| ProviderConfigError::MissingApiKey {
            provider: settings.active_provider.label(),
            env_var: env_key_name,
        })?;
    let wire_api = match profile.transport {
        ProviderTransport::Responses => OpenAiWireApi::Responses,
        ProviderTransport::ChatCompletions => OpenAiWireApi::ChatCompletions,
    };

    Ok(ResolvedProviderConfig {
        provider: settings.active_provider,
        api_key,
        base_url: profile.base_url.trim().to_string(),
        wire_api,
        responses_path: profile.responses_path.trim().to_string(),
        chat_completions_path: profile.chat_completions_path.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_store::ProviderProfile;

    #[test]
    fn openai_provider_uses_ui_key_before_env_key() {
        let settings = AppSettings::default();

        let resolved = resolve_provider_config(
            &settings,
            Some(" sk-ui "),
            &ProviderEnv {
                openai_api_key: Some("sk-env".to_string()),
                compatible_api_key: None,
            },
        )
        .unwrap();

        assert_eq!(resolved.provider, AiProviderKind::OpenAi);
        assert_eq!(resolved.api_key, "sk-ui");
        assert_eq!(resolved.base_url, "https://api.openai.com");
        assert_eq!(resolved.wire_api, OpenAiWireApi::Responses);
        assert_eq!(resolved.responses_path, "v1/responses");
        assert_eq!(resolved.chat_completions_path, "v1/chat/completions");
    }

    #[test]
    fn stored_key_is_used_when_no_transient_or_env() {
        let settings = AppSettings {
            openai: ProviderProfile {
                api_key: " sk-stored ".to_string(),
                ..ProviderProfile::openai_default()
            },
            ..Default::default()
        };

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv {
                openai_api_key: None,
                compatible_api_key: None,
            },
        )
        .unwrap();

        assert_eq!(resolved.api_key, "sk-stored");
    }

    #[test]
    fn stored_key_takes_priority_over_env() {
        let settings = AppSettings {
            openai: ProviderProfile {
                api_key: "sk-stored".to_string(),
                ..ProviderProfile::openai_default()
            },
            ..Default::default()
        };

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv {
                openai_api_key: Some("sk-env".to_string()),
                compatible_api_key: None,
            },
        )
        .unwrap();

        assert_eq!(resolved.api_key, "sk-stored");
    }

    #[test]
    fn compatible_provider_uses_compatible_env_key_and_transport() {
        let settings = AppSettings {
            active_provider: AiProviderKind::OpenAiCompatible,
            compatible: ProviderProfile {
                base_url: " https://vendor.example.test/v1-root ".to_string(),
                transport: ProviderTransport::ChatCompletions,
                responses_path: " custom/responses ".to_string(),
                chat_completions_path: " chat/completions ".to_string(),
                ..ProviderProfile::compatible_default()
            },
            ..Default::default()
        };

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv {
                openai_api_key: Some("sk-openai".to_string()),
                compatible_api_key: Some(" vendor-key ".to_string()),
            },
        )
        .unwrap();

        assert_eq!(resolved.provider, AiProviderKind::OpenAiCompatible);
        assert_eq!(resolved.api_key, "vendor-key");
        assert_eq!(resolved.base_url, "https://vendor.example.test/v1-root");
        assert_eq!(resolved.wire_api, OpenAiWireApi::ChatCompletions);
        assert_eq!(resolved.responses_path, "custom/responses");
        assert_eq!(resolved.chat_completions_path, "chat/completions");
    }

    #[test]
    fn missing_key_reports_selected_provider_and_env_var() {
        let settings = AppSettings {
            active_provider: AiProviderKind::OpenAiCompatible,
            ..Default::default()
        };

        let error = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv {
                openai_api_key: Some("sk-openai".to_string()),
                compatible_api_key: None,
            },
        )
        .unwrap_err();

        assert_eq!(
            error,
            ProviderConfigError::MissingApiKey {
                provider: "OpenAI-compatible API",
                env_var: "OPENAI_COMPATIBLE_API_KEY"
            }
        );
    }
}
