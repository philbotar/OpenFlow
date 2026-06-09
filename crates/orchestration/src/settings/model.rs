use providers::{
    builtin_provider_specs, provider_spec, ProviderId, ProviderKind, ProviderSpec, WireApi,
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
        }
        self.new_model_input.clear();
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
