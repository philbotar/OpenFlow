use crate::settings::model::{AppSettings, ProviderProfile};
use crate::settings::ports::SettingsStore;
use providers::{
    provider_spec, AiClientConfig, AnthropicConfig, AuthConfig, AuthSpec, BedrockConfig,
    CodexCredentialSink, CodexOAuthCredentials, OpenAiCodexConfig, OpenAiCompatibleConfig,
    ProviderAdapterConfig, ProviderId, ProviderKind,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderEnv {
    values: BTreeMap<String, String>,
}

impl ProviderEnv {
    #[must_use]
    pub fn from_system() -> Self {
        #[cfg(feature = "bedrock")]
        providers::ensure_process_home_env();
        let values = provider_env_var_names()
            .into_iter()
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

fn provider_env_var_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for spec in providers::builtin_provider_specs() {
        if let Some(env_var) = spec.auth.env_var() {
            names.push(env_var);
        }
        if let AuthSpec::AwsCredentials { region_env_var, .. } = spec.auth {
            names.push(region_env_var);
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderConfigError {
    #[error("provider {provider} is not implemented")]
    UnsupportedProvider { provider: String },
    #[error("{provider} API key missing")]
    MissingApiKey { provider: String, env_var: String },
    #[error("{provider} is not signed in")]
    MissingOAuth { provider: String },
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
    let api_key = if matches!(spec.auth, AuthSpec::ChatGptOAuth) {
        None
    } else {
        resolve_api_key(
            transient_api_key,
            profile.api_key.as_str(),
            spec.display_name,
            spec.auth,
            env,
        )?
    };
    let region = resolve_bedrock_region(profile, spec.auth, env);
    let bedrock_aws_profile = if matches!(spec.kind, ProviderKind::Bedrock(_)) {
        resolve_bedrock_profile(profile, spec.auth, env)
    } else {
        None
    };
    let auth = if matches!(spec.kind, ProviderKind::Bedrock(_)) {
        AuthConfig::AwsCredentials {
            profile: bedrock_aws_profile.clone(),
            region: region.clone().unwrap_or_default(),
        }
    } else {
        auth_config(spec.auth, api_key.clone(), region.clone())
    };
    let adapter = match spec.kind {
        ProviderKind::OpenAiCompatible(_) => {
            ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
                base_url: profile.base_url.trim().to_string(),
                wire_api: profile.transport,
                responses_path: profile.responses_path.trim().to_string(),
                chat_completions_path: profile.chat_completions_path.trim().to_string(),
                request_timeout: Duration::from_secs(profile.request_timeout_secs.max(1)),
            })
        }
        ProviderKind::OpenAiCodex => {
            let credentials =
                profile
                    .codex_oauth
                    .clone()
                    .ok_or_else(|| ProviderConfigError::MissingOAuth {
                        provider: spec.display_name.to_string(),
                    })?;
            ProviderAdapterConfig::OpenAiCodex(OpenAiCodexConfig {
                base_url: profile.base_url.trim().to_string(),
                request_timeout: Duration::from_secs(profile.request_timeout_secs.max(1)),
                credentials,
                credential_sink: None,
            })
        }
        ProviderKind::Anthropic(anthropic) => ProviderAdapterConfig::Anthropic(AnthropicConfig {
            base_url: profile.base_url.trim().to_string(),
            messages_path: anthropic.messages_path.to_string(),
            anthropic_version: anthropic.anthropic_version.to_string(),
            request_timeout: Duration::from_secs(profile.request_timeout_secs.max(1)),
        }),
        ProviderKind::Bedrock(_) => {
            let region = region.ok_or_else(|| ProviderConfigError::MissingApiKey {
                provider: spec.display_name.to_string(),
                env_var: bedrock_region_env_var(spec.auth)
                    .unwrap_or("AWS_REGION")
                    .to_string(),
            })?;
            ProviderAdapterConfig::Bedrock(BedrockConfig {
                region,
                aws_profile: bedrock_aws_profile,
                aws_credential_command: first_trimmed_string([Some(
                    profile.aws_credential_command.as_str(),
                )]),
            })
        }
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

fn first_trimmed_string<'a>(sources: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    sources
        .into_iter()
        .find_map(|source| trimmed(source).map(str::to_string))
}

fn env_trimmed_string(env: &ProviderEnv, env_var: &str) -> Option<String> {
    first_trimmed_string([env.get(env_var)]).or_else(|| {
        std::env::var(env_var)
            .ok()
            .and_then(|value| trimmed(Some(&value)).map(str::to_string))
    })
}

fn resolve_api_key(
    transient_api_key: Option<&str>,
    stored_api_key: &str,
    provider_label: &str,
    auth: AuthSpec,
    env: &ProviderEnv,
) -> Result<Option<String>, ProviderConfigError> {
    let api_key = first_trimmed_string([
        transient_api_key,
        Some(stored_api_key),
        auth.env_var().and_then(|env_var| env.get(env_var)),
    ]);

    if api_key.is_none() && auth.requires_key() {
        return Err(ProviderConfigError::MissingApiKey {
            provider: provider_label.to_string(),
            env_var: auth.env_var().unwrap_or("API key").to_string(),
        });
    }

    Ok(api_key)
}

fn auth_config(auth: AuthSpec, api_key: Option<String>, region: Option<String>) -> AuthConfig {
    match auth {
        AuthSpec::Bearer { required, .. } => AuthConfig::Bearer { api_key, required },
        AuthSpec::Header { name, required, .. } => AuthConfig::Header {
            name: name.to_string(),
            api_key,
            required,
        },
        AuthSpec::NoneAllowed { .. } => AuthConfig::NoneAllowed,
        AuthSpec::ChatGptOAuth => AuthConfig::NoneAllowed,
        AuthSpec::AwsCredentials {
            profile_env_var, ..
        } => AuthConfig::AwsCredentials {
            profile: api_key.or_else(|| {
                std::env::var(profile_env_var)
                    .ok()
                    .and_then(|value| trimmed(Some(&value)).map(str::to_string))
            }),
            region: region.unwrap_or_default(),
        },
    }
}

#[derive(Clone)]
struct SettingsCodexCredentialSink {
    store: Arc<dyn SettingsStore>,
}

impl CodexCredentialSink for SettingsCodexCredentialSink {
    fn save(&self, credentials: &CodexOAuthCredentials) -> Result<(), String> {
        let mut settings = self.store.load().map_err(|error| error.to_string())?;
        let profile = settings
            .providers
            .get_mut(&ProviderId::from("openai-codex"))
            .ok_or_else(|| "OpenAI Codex provider profile is missing".to_string())?;
        profile.codex_oauth = Some(credentials.clone());
        self.store
            .save_raw(&settings)
            .map_err(|error| error.to_string())
    }
}

/// Attaches durable token-rotation persistence to a resolved Codex config.
pub fn attach_codex_credential_sink(config: &mut AiClientConfig, store: Arc<dyn SettingsStore>) {
    if let ProviderAdapterConfig::OpenAiCodex(codex) = &mut config.adapter {
        codex.credential_sink = Some(Arc::new(SettingsCodexCredentialSink { store }));
    }
}

fn resolve_bedrock_region(
    profile: &ProviderProfile,
    auth: AuthSpec,
    env: &ProviderEnv,
) -> Option<String> {
    first_trimmed_string([
        Some(profile.aws_region.as_str()),
        Some(profile.base_url.as_str()),
    ])
    .or_else(|| bedrock_region_env_var(auth).and_then(|env_var| env_trimmed_string(env, env_var)))
}

fn resolve_bedrock_profile(
    profile: &ProviderProfile,
    auth: AuthSpec,
    env: &ProviderEnv,
) -> Option<String> {
    first_trimmed_string([Some(profile.aws_profile.as_str())])
        .or_else(|| first_trimmed_string([auth.env_var().and_then(|env_var| env.get(env_var))]))
        .or_else(|| match auth {
            AuthSpec::AwsCredentials {
                profile_env_var, ..
            } => env_trimmed_string(env, profile_env_var),
            _ => None,
        })
}

fn bedrock_region_env_var(auth: AuthSpec) -> Option<&'static str> {
    match auth {
        AuthSpec::AwsCredentials { region_env_var, .. } => Some(region_env_var),
        _ => None,
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
                request_timeout_secs: 45,
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
        assert_eq!(config.request_timeout, std::time::Duration::from_secs(45));
    }

    #[test]
    fn bedrock_provider_uses_region_and_optional_profile() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_region = "eu-west-1".to_string();
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_profile = "work".to_string();

        let resolved = resolve_provider_config(&settings, None, &ProviderEnv::default()).unwrap();

        assert_eq!(resolved.provider_id, ProviderId::from("bedrock"));
        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(config.aws_profile.as_deref(), Some("work"));
        assert!(matches!(resolved.auth, AuthConfig::AwsCredentials { .. }));
    }

    #[test]
    fn bedrock_ignores_api_key_and_transient_key_for_profile() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .api_key = "legacy-key".to_string();

        let resolved =
            resolve_provider_config(&settings, Some("transient-key"), &ProviderEnv::default())
                .unwrap();

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.aws_profile, None);
        assert!(matches!(
            resolved.auth,
            AuthConfig::AwsCredentials { profile: None, .. }
        ));
    }

    #[test]
    fn bedrock_profile_falls_back_to_aws_profile_env() {
        let settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("AWS_PROFILE", "bedrock-sso")]),
        )
        .unwrap();

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.aws_profile.as_deref(), Some("bedrock-sso"));
        assert!(matches!(
            resolved.auth,
            AuthConfig::AwsCredentials {
                profile: Some(ref name),
                ..
            } if name == "bedrock-sso"
        ));
    }

    #[test]
    fn bedrock_region_falls_back_to_aws_region_env() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_region
            .clear();
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .base_url
            .clear();

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("AWS_REGION", "ap-southeast-2")]),
        )
        .unwrap();

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.region, "ap-southeast-2");
        assert!(matches!(
            resolved.auth,
            AuthConfig::AwsCredentials {
                region: ref resolved_region,
                ..
            } if resolved_region == "ap-southeast-2"
        ));
    }

    #[test]
    fn provider_env_var_names_include_bedrock_profile_and_region() {
        let names = provider_env_var_names();

        assert!(names.contains(&"AWS_PROFILE"));
        assert!(names.contains(&"AWS_REGION"));
    }

    #[test]
    fn bedrock_region_reads_legacy_base_url_when_aws_region_missing() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        let profile = settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile");
        profile.aws_region.clear();
        profile.base_url = "ap-southeast-2".to_string();

        let resolved = resolve_provider_config(&settings, None, &ProviderEnv::default()).unwrap();

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.region, "ap-southeast-2");
    }

    #[test]
    fn bedrock_credential_command_flows_to_adapter_config() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        let profile = settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile");
        profile.aws_credential_command =
            "  aws configure export-credentials --profile bedrock  ".to_string();

        let config = resolve_provider_config(&settings, None, &ProviderEnv::default())
            .expect("provider config");

        let ProviderAdapterConfig::Bedrock(bedrock) = config.adapter else {
            panic!("expected bedrock adapter");
        };
        assert_eq!(
            bedrock.aws_credential_command.as_deref(),
            Some("aws configure export-credentials --profile bedrock")
        );
    }

    #[test]
    fn bedrock_stored_aws_profile_beats_env() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_profile = "from-settings".to_string();

        let resolved = resolve_provider_config(
            &settings,
            None,
            &ProviderEnv::from_pairs([("AWS_PROFILE", "from-env")]),
        )
        .unwrap();

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.aws_profile.as_deref(), Some("from-settings"));
    }

    #[test]
    fn bedrock_region_falls_back_to_live_aws_region_env() {
        use std::sync::{Mutex, OnceLock};

        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let previous = std::env::var_os("AWS_REGION");
        std::env::set_var("AWS_REGION", "eu-central-1");

        let mut settings = AppSettings {
            active_provider: ProviderId::from("bedrock"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .aws_region
            .clear();
        settings
            .providers
            .get_mut(&ProviderId::from("bedrock"))
            .expect("bedrock profile")
            .base_url
            .clear();

        let resolved = resolve_provider_config(&settings, None, &ProviderEnv::default()).unwrap();

        match previous {
            Some(value) => std::env::set_var("AWS_REGION", value),
            None => std::env::remove_var("AWS_REGION"),
        }

        let ProviderAdapterConfig::Bedrock(config) = resolved.adapter else {
            panic!("expected Bedrock adapter");
        };
        assert_eq!(config.region, "eu-central-1");
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
