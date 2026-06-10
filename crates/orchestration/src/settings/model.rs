use providers::{
    builtin_provider_specs, provider_spec, ProviderId, ProviderKind, ProviderSpec,
    ReasoningEffortOption, WireApi,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type ProviderTransport = WireApi;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub display_name: String,
    pub base_url: String,
    pub transport: ProviderTransport,
    #[serde(default = "default_responses_path")]
    pub responses_path: String,
    #[serde(default = "default_chat_completions_path")]
    pub chat_completions_path: String,
    pub known_models: Vec<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(default)]
    pub editable: bool,
    #[serde(skip)]
    pub new_model_input: String,
    #[serde(
        default,
        rename = "reasoningEffortOptions",
        alias = "reasoning_effort_options"
    )]
    pub reasoning_effort_options: Vec<ReasoningEffortOption>,
    #[serde(
        default,
        rename = "defaultReasoningBudgetTokens",
        alias = "default_reasoning_budget_tokens"
    )]
    pub default_reasoning_budget_tokens: BTreeMap<String, u32>,
    #[serde(
        default,
        rename = "defaultReasoningEffort",
        alias = "default_reasoning_effort"
    )]
    pub default_reasoning_effort: Option<String>,
}

fn default_responses_path() -> String {
    "v1/responses".to_string()
}

fn default_chat_completions_path() -> String {
    "v1/chat/completions".to_string()
}

impl ProviderProfile {
    #[must_use]
    pub fn from_spec(spec: &ProviderSpec) -> Self {
        let (transport, responses_path, chat_completions_path) = match spec.kind {
            ProviderKind::OpenAiCompatible(openai) => (
                openai.default_wire_api,
                openai.responses_path.to_string(),
                openai.chat_completions_path.to_string(),
            ),
            ProviderKind::Anthropic(_) => (
                ProviderTransport::ChatCompletions,
                default_responses_path(),
                default_chat_completions_path(),
            ),
        };
        Self {
            display_name: spec.display_name.to_string(),
            base_url: spec.default_base_url.to_string(),
            transport,
            responses_path,
            chat_completions_path,
            known_models: spec
                .default_models
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
            default_model: Some(spec.default_model.to_string()),
            api_key: String::new(),
            editable: spec.editable,
            new_model_input: String::new(),
            reasoning_effort_options: spec.default_reasoning_effort_options(),
            default_reasoning_budget_tokens: Self::default_budget_tokens_for_spec(spec),
            default_reasoning_effort: None,
        }
    }

    #[must_use]
    pub fn openai_default() -> Self {
        provider_spec(&ProviderId::from("openai"))
            .map(Self::from_spec)
            .unwrap_or_else(|| Self::fallback("openai", "OpenAI"))
    }

    #[must_use]
    pub fn compatible_default() -> Self {
        provider_spec(&ProviderId::from("custom_openai_compatible"))
            .map(Self::from_spec)
            .unwrap_or_else(|| {
                Self::fallback("custom_openai_compatible", "Custom OpenAI-compatible API")
            })
    }

    fn fallback(_id: &str, display_name: &str) -> Self {
        Self {
            display_name: display_name.to_string(),
            base_url: "https://api.openai.com".to_string(),
            transport: ProviderTransport::Responses,
            responses_path: default_responses_path(),
            chat_completions_path: default_chat_completions_path(),
            known_models: vec!["gpt-4o-mini".to_string()],
            default_model: Some("gpt-4o-mini".to_string()),
            api_key: String::new(),
            editable: false,
            new_model_input: String::new(),
            reasoning_effort_options: Vec::new(),
            default_reasoning_budget_tokens: BTreeMap::new(),
            default_reasoning_effort: None,
        }
    }

    fn normalize(&mut self, spec: Option<&ProviderSpec>) {
        if let Some(spec) = spec {
            if self.display_name.trim().is_empty() {
                self.display_name = spec.display_name.to_string();
            }
            if self.base_url.trim().is_empty() {
                self.base_url = spec.default_base_url.to_string();
            }
            if self.known_models.is_empty() {
                self.known_models = spec
                    .default_models
                    .iter()
                    .map(|model| (*model).to_string())
                    .collect();
            }
            if self.default_model.is_none() {
                self.default_model = Some(spec.default_model.to_string());
            }
            self.editable = spec.editable;
            if self.reasoning_effort_options.is_empty() {
                self.reasoning_effort_options = spec.default_reasoning_effort_options();
            }
            if self.default_reasoning_budget_tokens.is_empty() {
                self.default_reasoning_budget_tokens = Self::default_budget_tokens_for_spec(spec);
            }
        }
        self.new_model_input.clear();
    }

    /// Build the default budget token map for a provider spec.
    #[must_use]
    fn default_budget_tokens_for_spec(spec: &ProviderSpec) -> BTreeMap<String, u32> {
        match spec.kind {
            ProviderKind::Anthropic(_) => {
                let mut map = BTreeMap::new();
                map.insert("low".to_string(), 10_240);
                map.insert("medium".to_string(), 40_960);
                map.insert("high".to_string(), 128_000);
                map
            }
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSettings {
    #[serde(default = "default_lsp_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub format_on_write: bool,
    #[serde(default)]
    pub diagnostics_on_write: bool,
}

fn default_lsp_enabled() -> bool {
    true
}

impl Default for LspSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            format_on_write: false,
            diagnostics_on_write: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub active_provider: ProviderId,
    pub providers: BTreeMap<ProviderId, ProviderProfile>,
    #[serde(default)]
    pub skill_search_paths: Vec<String>,
    #[serde(default)]
    pub lsp: LspSettings,
}

impl AppSettings {
    #[must_use]
    pub fn active_profile(&self) -> &ProviderProfile {
        self.providers
            .get(&self.active_provider)
            .expect("active provider profile exists")
    }

    #[must_use]
    pub fn active_profile_mut(&mut self) -> &mut ProviderProfile {
        self.providers
            .get_mut(&self.active_provider)
            .expect("active provider profile exists")
    }

    #[must_use]
    pub fn active_models(&self) -> &[String] {
        &self.active_profile().known_models
    }

    #[must_use]
    pub fn provider_display_order(&self) -> Vec<ProviderId> {
        let mut ids = builtin_provider_specs()
            .iter()
            .map(|spec| ProviderId::from(spec.id))
            .filter(|id| self.providers.contains_key(id))
            .collect::<Vec<_>>();
        ids.extend(
            self.providers
                .keys()
                .filter(|id| provider_spec(id).is_none())
                .cloned(),
        );
        ids
    }

    #[must_use]
    pub fn redacted(&self) -> Self {
        let mut copy = self.clone();
        for profile in copy.providers.values_mut() {
            profile.api_key.clear();
        }
        copy
    }

    pub(crate) fn normalized(mut self) -> Self {
        for spec in builtin_provider_specs() {
            let id = ProviderId::from(spec.id);
            self.providers
                .entry(id)
                .or_insert_with(|| ProviderProfile::from_spec(spec));
        }
        let ids = self.providers.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let spec = provider_spec(&id);
            if let Some(profile) = self.providers.get_mut(&id) {
                profile.normalize(spec);
            }
        }
        if !self.providers.contains_key(&self.active_provider) {
            self.active_provider = ProviderId::from("openai");
        }
        self
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        let providers = builtin_provider_specs()
            .iter()
            .map(|spec| (ProviderId::from(spec.id), ProviderProfile::from_spec(spec)))
            .collect::<BTreeMap<_, _>>();
        Self {
            active_provider: ProviderId::from("openai"),
            providers,
            skill_search_paths: Vec::new(),
            lsp: LspSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub fn merge_preserved_api_keys(incoming: &mut AppSettings, existing: &AppSettings) {
    for (id, profile) in &mut incoming.providers {
        if profile.api_key.trim().is_empty() {
            if let Some(existing_profile) = existing.providers.get(id) {
                profile.api_key = existing_profile.api_key.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_profile_serde_roundtrip_with_reasoning_effort_options() {
        let profile = ProviderProfile {
            reasoning_effort_options: vec![ReasoningEffortOption {
                value: "adaptive".to_string(),
                label: "Adaptive".to_string(),
                uses_budget_tokens: false,
            }],
            default_reasoning_budget_tokens: {
                let mut m = BTreeMap::new();
                m.insert("low".to_string(), 10_240);
                m
            },
            default_reasoning_effort: Some("adaptive".to_string()),
            ..ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap())
        };
        let value = serde_json::to_value(&profile).unwrap();
        assert!(value["reasoningEffortOptions"].is_array());
        assert_eq!(value["reasoningEffortOptions"][0]["value"], "adaptive");
        assert_eq!(value["defaultReasoningBudgetTokens"]["low"], 10_240);
        assert_eq!(value["defaultReasoningEffort"], "adaptive");
        let back: ProviderProfile = serde_json::from_value(value).unwrap();
        assert_eq!(back.reasoning_effort_options.len(), 1);
        assert_eq!(back.default_reasoning_effort.as_deref(), Some("adaptive"));
        assert_eq!(
            back.default_reasoning_budget_tokens.get("low"),
            Some(&10_240)
        );
    }

    #[test]
    fn provider_profile_backfills_from_spec_when_empty() {
        // Simulate a profile saved before reasoning effort fields existed
        let value = serde_json::json!({
            "display_name": "Anthropic",
            "base_url": "https://api.anthropic.com",
            "transport": "chat_completions",
            "responses_path": "v1/responses",
            "chat_completions_path": "v1/chat/completions",
            "known_models": ["claude-3-5-sonnet-latest"],
            "default_model": "claude-3-5-sonnet-latest",
            "api_key": "",
            "editable": false
        });
        let mut profile: ProviderProfile = serde_json::from_value(value).unwrap();
        assert!(profile.reasoning_effort_options.is_empty());
        assert!(profile.default_reasoning_budget_tokens.is_empty());

        let spec = provider_spec(&ProviderId::from("anthropic")).unwrap();
        profile.normalize(Some(spec));
        assert!(!profile.reasoning_effort_options.is_empty());
        assert_eq!(profile.reasoning_effort_options.len(), 5);
        assert_eq!(
            profile.default_reasoning_budget_tokens.get("high"),
            Some(&128_000)
        );
    }

    #[test]
    fn provider_profile_preserves_user_added_options() {
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        // Add a custom user option
        profile
            .reasoning_effort_options
            .push(ReasoningEffortOption {
                value: "custom".to_string(),
                label: "Custom".to_string(),
                uses_budget_tokens: true,
            });
        let original_len = profile.reasoning_effort_options.len();

        // Normalize should NOT overwrite user options
        let spec = provider_spec(&ProviderId::from("anthropic")).unwrap();
        profile.normalize(Some(spec));
        assert_eq!(profile.reasoning_effort_options.len(), original_len);
    }
}
