#![allow(clippy::derive_partial_eq_without_eq, clippy::must_use_candidate)]

use providers::{
    builtin_provider_specs, provider_spec, ProviderId, ProviderKind, ProviderSpec, WireApi,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub active_provider: ProviderId,
    pub providers: BTreeMap<ProviderId, ProviderProfile>,
    #[serde(default)]
    pub skill_search_paths: Vec<String>,
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

    fn normalized(mut self) -> Self {
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
        }
    }
}

pub(crate) fn merge_preserved_api_keys(incoming: &mut AppSettings, existing: &AppSettings) {
    for (id, profile) in &mut incoming.providers {
        if profile.api_key.trim().is_empty() {
            if let Some(existing_profile) = existing.providers.get(id) {
                profile.api_key = existing_profile.api_key.clone();
            }
        }
    }
}

fn parse_settings_json(text: &str) -> Result<AppSettings, serde_json::Error> {
    serde_json::from_str::<AppSettings>(text).map(AppSettings::normalized)
}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    path: PathBuf,
}

impl FileSettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("step-through-agentic-workflow")
            .join("settings.json")
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read or parsed.
    pub fn load(&self) -> io::Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)?;
        parse_settings_json(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("settings JSON invalid: {error}"),
            )
        })
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        let mut to_save = settings.clone();
        if self.path.exists() {
            let text = fs::read_to_string(&self.path)?;
            if let Ok(existing) = parse_settings_json(&text) {
                merge_preserved_api_keys(&mut to_save, &existing);
            }
        }
        self.write(&to_save)
    }

    fn write(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(settings).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("settings serialization failed: {e}"),
            )
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_settings_include_builtin_provider_profiles() {
        let settings = AppSettings::default();

        assert_eq!(settings.active_provider, ProviderId::from("openai"));
        assert!(settings.providers.contains_key(&ProviderId::from("openai")));
        assert!(settings
            .providers
            .contains_key(&ProviderId::from("anthropic")));
        assert!(!settings
            .providers
            .contains_key(&ProviderId::from("bedrock")));
        assert!(!settings
            .providers
            .contains_key(&ProviderId::from("azure_native")));
        let openai = settings
            .providers
            .get(&ProviderId::from("openai"))
            .expect("openai profile");
        assert_eq!(openai.display_name, "OpenAI");
        assert_eq!(openai.base_url, "https://api.openai.com");
        assert_eq!(openai.transport, ProviderTransport::Responses);
        assert_eq!(openai.responses_path, "v1/responses");
        assert_eq!(openai.chat_completions_path, "v1/chat/completions");
        assert_eq!(openai.default_model.as_deref(), Some("gpt-4o-mini"));
        assert!(openai.api_key.is_empty());
        assert!(!openai.editable);
        let custom = settings
            .providers
            .get(&ProviderId::from("custom_openai_compatible"))
            .expect("custom profile");
        assert!(custom.editable);
        assert!(custom.known_models.contains(&"model-name".to_string()));
    }

    #[test]
    fn redacted_settings_clear_api_keys() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-secret".to_string();

        let redacted = settings.redacted();

        assert!(settings.active_profile().api_key == "sk-secret");
        assert!(redacted.active_profile().api_key.is_empty());
    }

    #[test]
    fn active_profile_tracks_active_provider() {
        let mut settings = AppSettings {
            active_provider: ProviderId::from("openai"),
            ..Default::default()
        };

        assert_eq!(settings.active_profile().display_name, "OpenAI");

        settings.active_provider = ProviderId::from("custom_openai_compatible");
        assert_eq!(
            settings.active_profile().display_name,
            "Custom OpenAI-compatible API"
        );
        settings
            .active_profile_mut()
            .known_models
            .push("custom-model".to_string());

        assert!(settings
            .providers
            .get(&ProviderId::from("custom_openai_compatible"))
            .expect("custom profile")
            .known_models
            .contains(&"custom-model".to_string()));
        assert!(!settings
            .providers
            .get(&ProviderId::from("openai"))
            .expect("openai profile")
            .known_models
            .contains(&"custom-model".to_string()));
    }

    #[test]
    fn save_from_redacted_snapshot_preserves_existing_api_keys() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-persisted".to_string();
        store.save(&settings).unwrap();

        let mut redacted = store.load().unwrap().redacted();
        redacted
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .known_models
            .push("new-model".to_string());
        store.save(&redacted).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(
            loaded
                .providers
                .get(&ProviderId::from("openai"))
                .expect("openai profile")
                .api_key,
            "sk-persisted"
        );
        assert!(loaded
            .providers
            .get(&ProviderId::from("openai"))
            .expect("openai profile")
            .known_models
            .contains(&"new-model".to_string()));
    }

    #[test]
    fn saves_provider_settings_without_transient_fields() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings {
            active_provider: ProviderId::from("custom_openai_compatible"),
            ..Default::default()
        };
        let profile = settings.active_profile_mut();
        profile.base_url = "https://models.example.test".to_string();
        profile.transport = ProviderTransport::Responses;
        profile.responses_path = "custom/responses".to_string();
        profile.chat_completions_path = "custom/chat/completions".to_string();
        profile.known_models = vec!["vendor-model".to_string()];
        profile.default_model = Some("vendor-model".to_string());
        profile.new_model_input = "draft-model".to_string();

        store.save(&settings).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        let loaded = store.load().unwrap();

        assert!(raw.contains("\"active_provider\""));
        assert!(raw.contains("\"providers\""));
        assert!(!raw.contains("draft-model"));
        assert_eq!(
            loaded
                .providers
                .get(&ProviderId::from("custom_openai_compatible"))
                .expect("custom profile")
                .base_url,
            "https://models.example.test"
        );
        assert!(loaded.active_profile().new_model_input.is_empty());
    }

    #[test]
    fn missing_settings_file_loads_default_settings() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn invalid_settings_json_returns_invalid_data_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let store = FileSettingsStore::new(path);

        let error = store.load().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("settings JSON invalid"));
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = FileSettingsStore::new(&path);
        let settings = AppSettings::default();

        store.save(&settings).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn settings_roundtrip_restores_identical_state() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings {
            active_provider: ProviderId::from("anthropic"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .transport = ProviderTransport::ChatCompletions;
        settings
            .providers
            .get_mut(&ProviderId::from("anthropic"))
            .expect("anthropic profile")
            .known_models = vec!["claude-custom".to_string()];

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, settings);
    }
}
