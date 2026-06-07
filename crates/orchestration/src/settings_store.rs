#![allow(clippy::derive_partial_eq_without_eq, clippy::must_use_candidate)]

use crate::credential_store::{CredentialStore, CredentialStoreError};
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
    #[serde(default)]
    pub key_ref: String,
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

#[must_use]
pub fn key_ref_for_provider(provider_id: &ProviderId) -> String {
    format!("provider:{}:api-key", provider_id.as_str())
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
            key_ref: key_ref_for_provider(&ProviderId::from(spec.id)),
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

    fn fallback(id: &str, display_name: &str) -> Self {
        Self {
            display_name: display_name.to_string(),
            base_url: "https://api.openai.com".to_string(),
            transport: ProviderTransport::Responses,
            responses_path: default_responses_path(),
            chat_completions_path: default_chat_completions_path(),
            known_models: vec!["gpt-4o-mini".to_string()],
            default_model: Some("gpt-4o-mini".to_string()),
            key_ref: key_ref_for_provider(&ProviderId::from(id)),
            editable: false,
            new_model_input: String::new(),
        }
    }

    fn normalize(&mut self, provider_id: &ProviderId, spec: Option<&ProviderSpec>) {
        if self.key_ref.trim().is_empty() {
            self.key_ref = key_ref_for_provider(provider_id);
        }
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
                profile.normalize(&id, spec);
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

#[derive(Debug, Deserialize)]
struct LegacyKnownModelsSettings {
    known_models: Vec<String>,
}

impl From<LegacyKnownModelsSettings> for AppSettings {
    fn from(value: LegacyKnownModelsSettings) -> Self {
        let mut settings = Self::default();
        if !value.known_models.is_empty() {
            if let Some(profile) = settings.providers.get_mut(&ProviderId::from("openai")) {
                profile.known_models = value.known_models;
            }
        }
        settings
    }
}

#[derive(Debug, Deserialize)]
struct LegacyFixedSettings {
    active_provider: Option<LegacyProviderKind>,
    openai: Option<LegacyProviderProfile>,
    #[serde(rename = "openai_compatible")]
    compatible: Option<LegacyProviderProfile>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyProviderKind {
    OpenAi,
    OpenAiCompatible,
}

#[derive(Debug, Deserialize)]
struct LegacyProviderProfile {
    display_name: String,
    base_url: String,
    transport: ProviderTransport,
    #[serde(default = "default_responses_path")]
    responses_path: String,
    #[serde(default = "default_chat_completions_path")]
    chat_completions_path: String,
    known_models: Vec<String>,
    #[serde(default)]
    default_model: Option<String>,
    #[serde(default)]
    api_key: String,
}

struct MigratedSettings {
    settings: AppSettings,
    migrated_secret: bool,
}

impl LegacyProviderProfile {
    fn into_profile(self, provider_id: &ProviderId, fallback: ProviderProfile) -> ProviderProfile {
        ProviderProfile {
            display_name: if self.display_name.trim().is_empty() {
                fallback.display_name
            } else {
                self.display_name
            },
            base_url: if self.base_url.trim().is_empty() {
                fallback.base_url
            } else {
                self.base_url
            },
            transport: self.transport,
            responses_path: self.responses_path,
            chat_completions_path: self.chat_completions_path,
            known_models: if self.known_models.is_empty() {
                fallback.known_models
            } else {
                self.known_models
            },
            default_model: self.default_model.or(fallback.default_model),
            key_ref: key_ref_for_provider(provider_id),
            editable: fallback.editable,
            new_model_input: String::new(),
        }
    }
}

fn migrate_settings_json(
    text: &str,
    credential_store: &CredentialStore,
) -> Result<MigratedSettings, SettingsParseError> {
    let current_error = match serde_json::from_str::<AppSettings>(text) {
        Ok(settings) => {
            let mut value = serde_json::from_str::<serde_json::Value>(text)
                .map_err(SettingsParseError::JsonValue)?;
            let migrated_secret = migrate_provider_map_secrets(&mut value, credential_store)
                .map_err(SettingsParseError::Credential)?;
            return Ok(MigratedSettings {
                settings: settings.normalized(),
                migrated_secret,
            });
        }
        Err(error) => error,
    };

    let fixed_error = match serde_json::from_str::<LegacyFixedSettings>(text) {
        Ok(legacy) if legacy.openai.is_some() || legacy.compatible.is_some() => {
            return migrate_legacy_fixed_settings(legacy, credential_store)
                .map_err(SettingsParseError::Credential);
        }
        Ok(_) => serde_json::Error::io(io::Error::new(
            io::ErrorKind::InvalidData,
            "not legacy fixed provider settings",
        )),
        Err(error) => error,
    };

    serde_json::from_str::<LegacyKnownModelsSettings>(text)
        .map(|legacy| MigratedSettings {
            settings: AppSettings::from(legacy),
            migrated_secret: false,
        })
        .map_err(|known_error| SettingsParseError::Schemas {
            current: current_error,
            fixed: fixed_error,
            known: known_error,
        })
}

fn migrate_provider_map_secrets(
    value: &mut serde_json::Value,
    credential_store: &CredentialStore,
) -> Result<bool, CredentialStoreError> {
    let Some(providers) = value
        .get_mut("providers")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(false);
    };
    let mut migrated_secret = false;
    for (provider_id, profile) in providers {
        let Some(profile) = profile.as_object_mut() else {
            continue;
        };
        let api_key = profile
            .remove("api_key")
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        if !api_key.trim().is_empty() {
            let provider_id = ProviderId::from(provider_id.as_str());
            credential_store.set(&key_ref_for_provider(&provider_id), api_key.trim())?;
            migrated_secret = true;
        }
    }
    Ok(migrated_secret)
}

fn migrate_legacy_fixed_settings(
    legacy: LegacyFixedSettings,
    credential_store: &CredentialStore,
) -> Result<MigratedSettings, CredentialStoreError> {
    let mut settings = AppSettings::default();
    let mut migrated_secret = false;

    if let Some(openai) = legacy.openai {
        let provider_id = ProviderId::from("openai");
        let api_key = openai.api_key.trim().to_string();
        settings.providers.insert(
            provider_id.clone(),
            openai.into_profile(&provider_id, ProviderProfile::openai_default()),
        );
        if !api_key.is_empty() {
            credential_store.set(&key_ref_for_provider(&provider_id), &api_key)?;
            migrated_secret = true;
        }
    }

    if let Some(compatible) = legacy.compatible {
        let provider_id = ProviderId::from("custom_openai_compatible");
        let api_key = compatible.api_key.trim().to_string();
        settings.providers.insert(
            provider_id.clone(),
            compatible.into_profile(&provider_id, ProviderProfile::compatible_default()),
        );
        if !api_key.is_empty() {
            credential_store.set(&key_ref_for_provider(&provider_id), &api_key)?;
            migrated_secret = true;
        }
    }

    settings.active_provider = match legacy.active_provider.unwrap_or(LegacyProviderKind::OpenAi) {
        LegacyProviderKind::OpenAi => ProviderId::from("openai"),
        LegacyProviderKind::OpenAiCompatible => ProviderId::from("custom_openai_compatible"),
    };

    Ok(MigratedSettings {
        settings: settings.normalized(),
        migrated_secret,
    })
}

#[derive(Debug)]
enum SettingsParseError {
    JsonValue(serde_json::Error),
    Credential(CredentialStoreError),
    Schemas {
        current: serde_json::Error,
        fixed: serde_json::Error,
        known: serde_json::Error,
    },
}

impl std::fmt::Display for SettingsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonValue(error) => write!(f, "settings JSON invalid: {error}"),
            Self::Credential(error) => write!(f, "settings credential migration failed: {error}"),
            Self::Schemas {
                current,
                fixed,
                known,
            } => write!(
                f,
                "settings JSON invalid: current schema: {current}; legacy provider schema: {fixed}; legacy known-model schema: {known}"
            ),
        }
    }
}

impl std::error::Error for SettingsParseError {}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    path: PathBuf,
    credential_store: CredentialStore,
}

impl FileSettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_credential_store(path, CredentialStore::system())
    }

    #[must_use]
    pub fn with_credential_store(
        path: impl Into<PathBuf>,
        credential_store: CredentialStore,
    ) -> Self {
        Self {
            path: path.into(),
            credential_store,
        }
    }

    #[must_use]
    pub fn credential_store(&self) -> &CredentialStore {
        &self.credential_store
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("step-through-agentic-workflow")
            .join("settings.json")
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read, parsed, migrated, or sanitized.
    pub fn load(&self) -> io::Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)?;
        let migrated = migrate_settings_json(&text, &self.credential_store)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        if migrated.migrated_secret {
            self.save(&migrated.settings)?;
        }
        Ok(migrated.settings)
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
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

    fn memory_store(path: impl Into<PathBuf>) -> FileSettingsStore {
        FileSettingsStore::with_credential_store(path, CredentialStore::in_memory())
    }

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
        assert_eq!(openai.key_ref, "provider:openai:api-key");
        assert!(!openai.editable);
        let custom = settings
            .providers
            .get(&ProviderId::from("custom_openai_compatible"))
            .expect("custom profile");
        assert!(custom.editable);
        assert!(custom.known_models.contains(&"model-name".to_string()));
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
    fn loads_legacy_known_models_into_openai_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "known_models": ["legacy-a", "legacy-b"]
}"#,
        )
        .unwrap();
        let store = memory_store(path);

        let settings = store.load().unwrap();

        assert_eq!(settings.active_provider, ProviderId::from("openai"));
        assert_eq!(
            settings
                .providers
                .get(&ProviderId::from("openai"))
                .expect("openai profile")
                .known_models,
            vec!["legacy-a".to_string(), "legacy-b".to_string()]
        );
    }

    #[test]
    fn migrates_legacy_fixed_profiles_and_removes_plaintext_keys() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "active_provider": "open_ai_compatible",
  "openai": {
    "display_name": "ChatGPT / OpenAI",
    "base_url": "https://api.openai.com",
    "transport": "responses",
    "responses_path": "v1/responses",
    "chat_completions_path": "v1/chat/completions",
    "known_models": ["legacy-openai"],
    "default_model": "legacy-openai",
    "api_key": " sk-openai "
  },
  "openai_compatible": {
    "display_name": "Compatible",
    "base_url": "https://vendor.example.test/v1-root",
    "transport": "chat_completions",
    "responses_path": "custom/responses",
    "chat_completions_path": "chat/completions",
    "known_models": ["vendor-model"],
    "default_model": "vendor-model",
    "api_key": " vendor-key "
  }
}"#,
        )
        .unwrap();
        let store = memory_store(path);

        let settings = store.load().unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();

        assert_eq!(
            settings.active_provider,
            ProviderId::from("custom_openai_compatible")
        );
        let custom = settings
            .providers
            .get(&ProviderId::from("custom_openai_compatible"))
            .expect("custom profile");
        assert_eq!(custom.base_url, "https://vendor.example.test/v1-root");
        assert_eq!(custom.transport, ProviderTransport::ChatCompletions);
        assert_eq!(custom.responses_path, "custom/responses");
        assert_eq!(custom.chat_completions_path, "chat/completions");
        assert_eq!(custom.known_models, vec!["vendor-model".to_string()]);
        assert_eq!(custom.default_model.as_deref(), Some("vendor-model"));
        assert!(!raw.contains("api_key"));
        assert_eq!(
            store
                .credential_store()
                .get("provider:openai:api-key")
                .unwrap()
                .as_deref(),
            Some("sk-openai")
        );
        assert_eq!(
            store
                .credential_store()
                .get("provider:custom_openai_compatible:api-key")
                .unwrap()
                .as_deref(),
            Some("vendor-key")
        );
    }

    #[test]
    fn saves_provider_settings_without_transient_or_secret_fields() {
        let dir = tempdir().unwrap();
        let store = memory_store(dir.path().join("settings.json"));
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
        assert!(!raw.contains("api_key"));
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
        let store = memory_store(dir.path().join("settings.json"));

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn invalid_settings_json_returns_invalid_data_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let store = memory_store(path);

        let error = store.load().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("settings JSON invalid"));
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = memory_store(&path);
        let settings = AppSettings::default();

        store.save(&settings).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn settings_roundtrip_restores_identical_state() {
        let dir = tempdir().unwrap();
        let store = memory_store(dir.path().join("settings.json"));
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
