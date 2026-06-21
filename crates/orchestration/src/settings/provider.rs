use crate::settings::model::AppSettings;
use providers::{
    provider_spec, AiClientConfig, AnthropicConfig, AuthConfig, AuthSpec, OpenAiCompatibleConfig,
    ProviderAdapterConfig, ProviderKind,
};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderEnv {
    values: BTreeMap<String, String>,
}

impl ProviderEnv {
    #[must_use]
    pub fn from_system() -> Self {
        let values = providers::builtin_provider_specs()
            .iter()
            .filter_map(|spec| spec.auth.env_var())
            .filter_map(|env_var| {
                std::env::var(env_var)
                    .ok()
                    .map(|value| (env_var.to_string(), value))
            })
            .collect();
        Self { values }
    }

    #[must_use]
    pub fn from_pairs(
        pairs: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            values: pairs
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    #[must_use]
    pub fn get(&self, env_var: &str) -> Option<&str> {
        self.values.get(env_var).map(String::as_str)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderConfigError {
    #[error("provider {provider} is not implemented")]
    UnsupportedProvider { provider: String },
    #[error("{provider} API key missing")]
    MissingApiKey { provider: String, env_var: String },
}

/// # Errors
/// Returns an error if the active provider is unsupported or its required key cannot be resolved.
pub fn resolve_provider_config(
    settings: &AppSettings,
    transient_api_key: Option<&str>,
    env: &ProviderEnv,
) -> Result<AiClientConfig, ProviderConfigError> {
    let provider_id = settings.active_provider.clone();
    let spec =
        provider_spec(&provider_id).ok_or_else(|| ProviderConfigError::UnsupportedProvider {
            provider: provider_id.to_string(),
        })?;
    let profile = settings.active_profile();
    let api_key = resolve_api_key(
        transient_api_key,
        profile.api_key.as_str(),
        spec.display_name,
        spec.auth,
        env,
    )?;
    let auth = auth_config(spec.auth, api_key.clone());
    let adapter = match spec.kind {
        ProviderKind::OpenAiCompatible(_) => {
            ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
                base_url: profile.base_url.trim().to_string(),
                wire_api: profile.transport,
                responses_path: profile.responses_path.trim().to_string(),
                chat_completions_path: profile.chat_completions_path.trim().to_string(),
            })
        }
        ProviderKind::Anthropic(anthropic) => ProviderAdapterConfig::Anthropic(AnthropicConfig {
            base_url: profile.base_url.trim().to_string(),
            messages_path: anthropic.messages_path.to_string(),
            anthropic_version: anthropic.anthropic_version.to_string(),
        }),
    };

    Ok(AiClientConfig {
        provider_id,
        provider_label: spec.display_name.to_string(),
        auth,
        adapter,
    })
}

#[must_use]
pub fn active_provider_env_var(settings: &AppSettings) -> Option<&'static str> {
    provider_spec(&settings.active_provider).and_then(|spec| spec.auth.env_var())
}

#[must_use]
pub fn active_provider_label(settings: &AppSettings) -> String {
    provider_spec(&settings.active_provider)
        .map(|spec| spec.display_name.to_string())
        .unwrap_or_else(|| settings.active_provider.to_string())
}

fn resolve_api_key(
    transient_api_key: Option<&str>,
    stored_api_key: &str,
    provider_label: &str,
    auth: AuthSpec,
    env: &ProviderEnv,
) -> Result<Option<String>, ProviderConfigError> {
    let api_key = trimmed(transient_api_key)
        .map(str::to_string)
        .or_else(|| trimmed(Some(stored_api_key)).map(str::to_string))
        .or_else(|| {
            auth.env_var()
                .and_then(|env_var| env.get(env_var))
                .and_then(|value| trimmed(Some(value)).map(str::to_string))
        });

    if api_key.is_none() && auth.requires_key() {
        return Err(ProviderConfigError::MissingApiKey {
            provider: provider_label.to_string(),
            env_var: auth.env_var().unwrap_or("API key").to_string(),
        });
    }

    Ok(api_key)
}

fn auth_config(auth: AuthSpec, api_key: Option<String>) -> AuthConfig {
    match auth {
        AuthSpec::Bearer { required, .. } => AuthConfig::Bearer { api_key, required },
        AuthSpec::Header { name, required, .. } => AuthConfig::Header {
            name: name.to_string(),
            api_key,
            required,
        },
        AuthSpec::NoneAllowed { .. } => AuthConfig::NoneAllowed,
    }
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::{ProviderProfile, ProviderTransport};
    use providers::{ProviderAdapterConfig, ProviderId, WireApi};

    #[test]
    fn openai_provider_uses_ui_key_before_env_key() {
        let settings = AppSettings::default();

        let resolved = resolve_provider_config(
            &settings,
            Some(" sk-ui "),
            &ProviderEnv::from_pairs([("OPENAI_API_KEY", "sk-env")]),
        )
        .unwrap();

        assert_eq!(resolved.provider_id, ProviderId::from("openai"));
        assert_eq!(resolved.provider_label, "OpenAI");
        assert_eq!(
            resolved.auth,
            AuthConfig::Bearer {
                api_key: Some("sk-ui".to_string()),
                required: true,
            }
        );
        let ProviderAdapterConfig::OpenAiCompatible(config) = resolved.adapter else {
            panic!("expected OpenAI-compatible adapter");
        };
        assert_eq!(config.base_url, "https://api.openai.com");
        assert_eq!(config.wire_api, WireApi::Responses);
        assert_eq!(config.responses_path, "v1/responses");
        assert_eq!(config.chat_completions_path, "v1/chat/completions");
    }

    #[test]
    fn stored_settings_key_is_used_when_no_transient_or_env() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = " sk-stored ".to_string();

        let resolved = resolve_provider_config(&settings, None, &ProviderEnv::default()).unwrap();

        assert_eq!(
            resolved.auth,
            AuthConfig::Bearer {
                api_key: Some("sk-stored".to_string()),
                required: true,
            }
        );
    }

    #[test]
    fn stored_settings_key_takes_priority_over_env() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-stored".to_string();

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("OPENAI_API_KEY", "sk-env")]),
        )
        .unwrap();

        assert_eq!(
            resolved.auth,
            AuthConfig::Bearer {
                api_key: Some("sk-stored".to_string()),
                required: true,
            }
        );
    }

    #[test]
    fn custom_compatible_provider_uses_compatible_env_key_and_transport() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("custom_openai_compatible"),
            ..Default::default()
        };
        settings.providers.insert(
            ProviderId::from("custom_openai_compatible"),
            ProviderProfile {
                base_url: " https://vendor.example.test/v1-root ".to_string(),
                transport: ProviderTransport::ChatCompletions,
                responses_path: " custom/responses ".to_string(),
                chat_completions_path: " chat/completions ".to_string(),
                ..ProviderProfile::compatible_default()
            },
        );

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([
                ("OPENAI_API_KEY", "sk-openai"),
                ("OPENAI_COMPATIBLE_API_KEY", " vendor-key "),
            ]),
        )
        .unwrap();

        assert_eq!(
            resolved.provider_id,
            ProviderId::from("custom_openai_compatible")
        );
        assert_eq!(
            resolved.auth,
            AuthConfig::Bearer {
                api_key: Some("vendor-key".to_string()),
                required: true,
            }
        );
        let ProviderAdapterConfig::OpenAiCompatible(config) = resolved.adapter else {
            panic!("expected OpenAI-compatible adapter");
        };
        assert_eq!(config.base_url, "https://vendor.example.test/v1-root");
        assert_eq!(config.wire_api, WireApi::ChatCompletions);
        assert_eq!(config.responses_path, "custom/responses");
        assert_eq!(config.chat_completions_path, "chat/completions");
    }

    #[test]
    fn anthropic_provider_uses_header_auth_and_direct_adapter() {
        let settings = AppSettings {
            active_provider: ProviderId::from("anthropic"),
            ..Default::default()
        };

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("ANTHROPIC_API_KEY", " anthropic-key ")]),
        )
        .unwrap();

        assert_eq!(resolved.provider_id, ProviderId::from("anthropic"));
        assert_eq!(
            resolved.auth,
            AuthConfig::Header {
                name: "x-api-key".to_string(),
                api_key: Some("anthropic-key".to_string()),
                required: true,
            }
        );
        let ProviderAdapterConfig::Anthropic(config) = resolved.adapter else {
            panic!("expected Anthropic adapter");
        };
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.messages_path, "v1/messages");
        assert_eq!(config.anthropic_version, "2023-06-01");
    }

    #[test]
    fn local_provider_does_not_require_key() {
        let settings = AppSettings {
            active_provider: ProviderId::from("ollama"),
            ..Default::default()
        };

        let resolved = resolve_provider_config(&settings, None, &ProviderEnv::default())
            .expect("ollama config without key");

        assert_eq!(resolved.auth, AuthConfig::NoneAllowed);
        let ProviderAdapterConfig::OpenAiCompatible(config) = resolved.adapter else {
            panic!("expected OpenAI-compatible adapter");
        };
        assert_eq!(config.base_url, "http://localhost:11434/v1");
        assert_eq!(config.wire_api, WireApi::ChatCompletions);
    }

    #[test]
    fn missing_key_reports_selected_provider_and_env_var() {
        let settings = AppSettings {
            active_provider: ProviderId::from("custom_openai_compatible"),
            ..Default::default()
        };

        let error = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("OPENAI_API_KEY", "sk-openai")]),
        )
        .unwrap_err();

        assert_eq!(
            error,
            ProviderConfigError::MissingApiKey {
                provider: "Custom OpenAI-compatible API".to_string(),
                env_var: "OPENAI_COMPATIBLE_API_KEY".to_string(),
            }
        );
    }
}
