use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ProviderId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireApi {
    Responses,
    ChatCompletions,
}

impl WireApi {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Responses => "Responses API",
            Self::ChatCompletions => "Chat Completions API",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSpec {
    Bearer {
        env_var: &'static str,
        required: bool,
    },
    Header {
        name: &'static str,
        env_var: &'static str,
        required: bool,
    },
    NoneAllowed {
        env_var: Option<&'static str>,
    },
    AwsCredentials {
        profile_env_var: &'static str,
        region_env_var: &'static str,
    },
}

impl AuthSpec {
    #[must_use]
    pub const fn env_var(self) -> Option<&'static str> {
        match self {
            Self::Bearer { env_var, .. } | Self::Header { env_var, .. } => Some(env_var),
            Self::NoneAllowed { env_var } => env_var,
            Self::AwsCredentials {
                profile_env_var, ..
            } => Some(profile_env_var),
        }
    }

    #[must_use]
    pub const fn requires_key(self) -> bool {
        match self {
            Self::Bearer { required, .. } | Self::Header { required, .. } => required,
            Self::NoneAllowed { .. } | Self::AwsCredentials { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiCompatibleSpec {
    pub default_wire_api: WireApi,
    pub responses_path: &'static str,
    pub chat_completions_path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicSpec {
    pub messages_path: &'static str,
    pub anthropic_version: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BedrockSpec {
    pub default_region: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAiCompatible(OpenAiCompatibleSpec),
    Anthropic(AnthropicSpec),
    Bedrock(BedrockSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub default_base_url: &'static str,
    pub kind: ProviderKind,
    pub auth: AuthSpec,
    pub default_models: &'static [&'static str],
    pub default_model: &'static str,
    pub editable: bool,
}

/// A single reasoning effort option that a provider supports, with metadata for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningEffortOption {
    pub value: String,
    pub label: String,
    pub uses_budget_tokens: bool,
}

impl ProviderSpec {
    /// Returns the built-in reasoning effort options for this provider kind.
    #[must_use]
    pub fn default_reasoning_effort_options(&self) -> Vec<ReasoningEffortOption> {
        match self.kind {
            ProviderKind::Anthropic(_) | ProviderKind::Bedrock(_) => vec![
                ReasoningEffortOption {
                    value: "none".to_string(),
                    label: "None".to_string(),
                    uses_budget_tokens: false,
                },
                ReasoningEffortOption {
                    value: "adaptive".to_string(),
                    label: "Adaptive".to_string(),
                    uses_budget_tokens: false,
                },
                ReasoningEffortOption {
                    value: "low".to_string(),
                    label: "Low".to_string(),
                    uses_budget_tokens: true,
                },
                ReasoningEffortOption {
                    value: "medium".to_string(),
                    label: "Medium".to_string(),
                    uses_budget_tokens: true,
                },
                ReasoningEffortOption {
                    value: "high".to_string(),
                    label: "High".to_string(),
                    uses_budget_tokens: true,
                },
            ],
            ProviderKind::OpenAiCompatible(_) => vec![
                ReasoningEffortOption {
                    value: "low".to_string(),
                    label: "Low".to_string(),
                    uses_budget_tokens: false,
                },
                ReasoningEffortOption {
                    value: "medium".to_string(),
                    label: "Medium".to_string(),
                    uses_budget_tokens: false,
                },
                ReasoningEffortOption {
                    value: "high".to_string(),
                    label: "High".to_string(),
                    uses_budget_tokens: false,
                },
            ],
        }
    }
}

const RESPONSES_PATH: &str = "v1/responses";
const CHAT_COMPLETIONS_PATH: &str = "v1/chat/completions";
const ANTHROPIC_MESSAGES_PATH: &str = "v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

const OPENAI_MODELS: &[&str] = &["gpt-4o", "gpt-4o-mini", "gpt-4.5", "o3"];
const OPENROUTER_MODELS: &[&str] = &[
    "openai/gpt-4o-mini",
    "anthropic/claude-3.5-sonnet",
    "meta-llama/llama-3.1-70b-instruct",
];
const GROQ_MODELS: &[&str] = &[
    "llama-3.3-70b-versatile",
    "llama-3.1-8b-instant",
    "mixtral-8x7b-32768",
];
const TOGETHER_MODELS: &[&str] = &[
    "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
    "Qwen/Qwen2.5-72B-Instruct-Turbo",
];
const FIREWORKS_MODELS: &[&str] = &[
    "accounts/fireworks/models/llama-v3p1-70b-instruct",
    "accounts/fireworks/models/qwen2p5-72b-instruct",
];
const DEEPSEEK_MODELS: &[&str] = &["deepseek-chat", "deepseek-reasoner"];
const XAI_MODELS: &[&str] = &["grok-2-latest", "grok-2-vision-latest"];
const MISTRAL_MODELS: &[&str] = &[
    "mistral-large-latest",
    "mistral-small-latest",
    "codestral-latest",
];
const PERPLEXITY_MODELS: &[&str] = &["sonar", "sonar-pro", "sonar-reasoning"];
const GEMINI_MODELS: &[&str] = &["gemini-2.0-flash", "gemini-1.5-pro", "gemini-1.5-flash"];
const OLLAMA_MODELS: &[&str] = &["llama3.1", "qwen2.5", "mistral"];
const LMSTUDIO_MODELS: &[&str] = &["local-model"];
const CUSTOM_MODELS: &[&str] = &["model-name"];
const ANTHROPIC_MODELS: &[&str] = &[
    "claude-3-5-sonnet-latest",
    "claude-3-5-haiku-latest",
    "claude-3-opus-latest",
];
const BEDROCK_MODELS: &[&str] = &["anthropic.claude-sonnet-4-20250514-v1:0"];

const OPENAI_COMPAT_RESPONSES: OpenAiCompatibleSpec = OpenAiCompatibleSpec {
    default_wire_api: WireApi::Responses,
    responses_path: RESPONSES_PATH,
    chat_completions_path: CHAT_COMPLETIONS_PATH,
};

const OPENAI_COMPAT_CHAT: OpenAiCompatibleSpec = OpenAiCompatibleSpec {
    default_wire_api: WireApi::ChatCompletions,
    responses_path: RESPONSES_PATH,
    chat_completions_path: CHAT_COMPLETIONS_PATH,
};

const ANTHROPIC: AnthropicSpec = AnthropicSpec {
    messages_path: ANTHROPIC_MESSAGES_PATH,
    anthropic_version: ANTHROPIC_VERSION,
};

const BEDROCK: BedrockSpec = BedrockSpec {
    default_region: "us-east-1",
};

const BUILTIN_PROVIDER_SPECS: &[ProviderSpec] = &[
    ProviderSpec {
        id: "openai",
        display_name: "OpenAI",
        default_base_url: "https://api.openai.com",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_RESPONSES),
        auth: AuthSpec::Bearer {
            env_var: "OPENAI_API_KEY",
            required: true,
        },
        default_models: OPENAI_MODELS,
        default_model: "gpt-4o-mini",
        editable: false,
    },
    ProviderSpec {
        id: "openrouter",
        display_name: "OpenRouter",
        default_base_url: "https://openrouter.ai/api/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "OPENROUTER_API_KEY",
            required: true,
        },
        default_models: OPENROUTER_MODELS,
        default_model: "openai/gpt-4o-mini",
        editable: false,
    },
    ProviderSpec {
        id: "groq",
        display_name: "Groq",
        default_base_url: "https://api.groq.com/openai/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "GROQ_API_KEY",
            required: true,
        },
        default_models: GROQ_MODELS,
        default_model: "llama-3.1-8b-instant",
        editable: false,
    },
    ProviderSpec {
        id: "together",
        display_name: "Together AI",
        default_base_url: "https://api.together.xyz/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "TOGETHER_API_KEY",
            required: true,
        },
        default_models: TOGETHER_MODELS,
        default_model: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
        editable: false,
    },
    ProviderSpec {
        id: "fireworks",
        display_name: "Fireworks AI",
        default_base_url: "https://api.fireworks.ai/inference/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "FIREWORKS_API_KEY",
            required: true,
        },
        default_models: FIREWORKS_MODELS,
        default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct",
        editable: false,
    },
    ProviderSpec {
        id: "deepseek",
        display_name: "DeepSeek",
        default_base_url: "https://api.deepseek.com/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "DEEPSEEK_API_KEY",
            required: true,
        },
        default_models: DEEPSEEK_MODELS,
        default_model: "deepseek-chat",
        editable: false,
    },
    ProviderSpec {
        id: "xai",
        display_name: "xAI / Grok",
        default_base_url: "https://api.x.ai/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "XAI_API_KEY",
            required: true,
        },
        default_models: XAI_MODELS,
        default_model: "grok-2-latest",
        editable: false,
    },
    ProviderSpec {
        id: "mistral",
        display_name: "Mistral AI",
        default_base_url: "https://api.mistral.ai/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "MISTRAL_API_KEY",
            required: true,
        },
        default_models: MISTRAL_MODELS,
        default_model: "mistral-small-latest",
        editable: false,
    },
    ProviderSpec {
        id: "perplexity",
        display_name: "Perplexity",
        default_base_url: "https://api.perplexity.ai",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "PERPLEXITY_API_KEY",
            required: true,
        },
        default_models: PERPLEXITY_MODELS,
        default_model: "sonar",
        editable: false,
    },
    ProviderSpec {
        id: "gemini",
        display_name: "Gemini OpenAI compatibility",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "GEMINI_API_KEY",
            required: true,
        },
        default_models: GEMINI_MODELS,
        default_model: "gemini-2.0-flash",
        editable: false,
    },
    ProviderSpec {
        id: "ollama",
        display_name: "Ollama local",
        default_base_url: "http://localhost:11434/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::NoneAllowed { env_var: None },
        default_models: OLLAMA_MODELS,
        default_model: "llama3.1",
        editable: false,
    },
    ProviderSpec {
        id: "lmstudio",
        display_name: "LM Studio local",
        default_base_url: "http://localhost:1234/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::NoneAllowed { env_var: None },
        default_models: LMSTUDIO_MODELS,
        default_model: "local-model",
        editable: false,
    },
    ProviderSpec {
        id: "custom_openai_compatible",
        display_name: "Custom OpenAI-compatible API",
        default_base_url: "http://localhost:11434/v1",
        kind: ProviderKind::OpenAiCompatible(OPENAI_COMPAT_CHAT),
        auth: AuthSpec::Bearer {
            env_var: "OPENAI_COMPATIBLE_API_KEY",
            required: true,
        },
        default_models: CUSTOM_MODELS,
        default_model: "model-name",
        editable: true,
    },
    ProviderSpec {
        id: "bedrock",
        display_name: "Amazon Bedrock",
        default_base_url: "",
        kind: ProviderKind::Bedrock(BEDROCK),
        auth: AuthSpec::AwsCredentials {
            profile_env_var: "AWS_PROFILE",
            region_env_var: "AWS_REGION",
        },
        default_models: BEDROCK_MODELS,
        default_model: "anthropic.claude-sonnet-4-20250514-v1:0",
        editable: false,
    },
    ProviderSpec {
        id: "anthropic",
        display_name: "Anthropic",
        default_base_url: "https://api.anthropic.com",
        kind: ProviderKind::Anthropic(ANTHROPIC),
        auth: AuthSpec::Header {
            name: "x-api-key",
            env_var: "ANTHROPIC_API_KEY",
            required: true,
        },
        default_models: ANTHROPIC_MODELS,
        default_model: "claude-3-5-sonnet-latest",
        editable: false,
    },
];

#[must_use]
pub const fn builtin_provider_specs() -> &'static [ProviderSpec] {
    BUILTIN_PROVIDER_SPECS
}

#[must_use]
pub fn provider_spec(id: &ProviderId) -> Option<&'static ProviderSpec> {
    BUILTIN_PROVIDER_SPECS
        .iter()
        .find(|spec| spec.id == id.as_str())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "provider spec tests use expect for brevity"
)]
mod tests {
    use super::*;

    #[test]
    fn builtin_specs_include_bedrock() {
        assert!(builtin_provider_specs()
            .iter()
            .any(|spec| spec.id == "bedrock"));
    }

    #[test]
    fn builtin_specs_include_openai_and_anthropic_exclude_deferred_special_auth() {
        let ids = builtin_provider_specs()
            .iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"anthropic"));
        assert!(!ids.contains(&"azure_native"));
    }

    #[test]
    fn local_providers_do_not_require_api_keys() {
        let ollama = provider_spec(&ProviderId::from("ollama"));
        let lmstudio = provider_spec(&ProviderId::from("lmstudio"));
        assert!(ollama.is_some() && lmstudio.is_some());
        let Some(ollama) = ollama else {
            return;
        };
        let Some(lmstudio) = lmstudio else {
            return;
        };

        assert!(!ollama.auth.requires_key());
        assert!(!lmstudio.auth.requires_key());
    }
}

#[cfg(test)]
mod reasoning_effort_tests {
    use super::*;

    #[test]
    fn default_reasoning_effort_options_anthropic() {
        let spec = provider_spec(&ProviderId::from("anthropic"));
        assert!(spec.is_some());
        let Some(spec) = spec else {
            return;
        };
        let options = spec.default_reasoning_effort_options();
        assert_eq!(options.len(), 5);
        assert_eq!(options[0].value, "none");
        assert!(!options[0].uses_budget_tokens);
        assert_eq!(options[1].value, "adaptive");
        assert!(!options[1].uses_budget_tokens);
        assert_eq!(options[2].value, "low");
        assert!(options[2].uses_budget_tokens);
        assert_eq!(options[3].value, "medium");
        assert!(options[3].uses_budget_tokens);
        assert_eq!(options[4].value, "high");
        assert!(options[4].uses_budget_tokens);
    }

    #[test]
    fn default_reasoning_effort_options_openai_compat() {
        let spec = provider_spec(&ProviderId::from("openai"));
        assert!(spec.is_some());
        let Some(spec) = spec else {
            return;
        };
        let options = spec.default_reasoning_effort_options();
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].value, "low");
        assert!(!options[0].uses_budget_tokens);
        assert_eq!(options[1].value, "medium");
        assert!(!options[1].uses_budget_tokens);
        assert_eq!(options[2].value, "high");
        assert!(!options[2].uses_budget_tokens);
    }
}
